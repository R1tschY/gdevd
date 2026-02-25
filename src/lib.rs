#[macro_use]
extern crate log;
#[macro_use]
extern crate quick_error;

use std::convert::TryFrom;
use std::fmt;
use std::fmt::Display;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::sync::{mpsc, Arc, Mutex, MutexGuard};

use hex::FromHexError;
use quick_error::ResultExt;
use rusb::{Context, Device, Hotplug, HotplugBuilder, Registration, UsbContext};

use crate::config::Config;
use crate::drivers::g203_lightsync::G203LightsyncDriver;
use crate::drivers::g213::G213Driver;

pub mod config;
pub mod drivers;
pub mod usb_ext;

const LOGITECH_USB_VENDOR_ID: u16 = 0x046d;

/// RGB color
#[derive(Clone, Debug)]
pub struct RgbColor(pub u8, pub u8, pub u8);

impl RgbColor {
    #[inline]
    pub fn red(&self) -> u8 {
        self.0
    }

    #[inline]
    pub fn green(&self) -> u8 {
        self.1
    }

    #[inline]
    pub fn blue(&self) -> u8 {
        self.2
    }

    pub fn from_hex(rgb_hex: &str) -> Result<Self, FromHexError> {
        let mut bytes = [0u8; 3];
        hex::decode_to_slice(rgb_hex, &mut bytes as &mut [u8])?;
        Ok(RgbColor(bytes[0], bytes[1], bytes[2]))
    }

    pub fn to_hex(&self) -> String {
        hex::encode([self.0, self.1, self.2])
    }

    #[inline]
    pub fn to_int(&self) -> u32 {
        ((self.0 as u32) << 16) | ((self.1 as u32) << 8) | (self.2 as u32)
    }
}

#[derive(PartialEq, Eq, Debug, Copy, Clone)]
pub enum Direction {
    LeftToRight = 1,
    RightToLeft = 6,
    CenterToEdge = 3,
    EdgeToCenter = 8,
}

impl TryFrom<&str> for Direction {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "left-to-right" => Ok(Direction::LeftToRight),
            "right-to-left" => Ok(Direction::RightToLeft),
            "center-to-edge" => Ok(Direction::CenterToEdge),
            "edge-to-center" => Ok(Direction::EdgeToCenter),
            _ => Err(()),
        }
    }
}

/// speed of effect
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Eq)]
pub struct Speed(u16);

impl From<u16> for Speed {
    #[inline]
    fn from(input: u16) -> Self {
        Speed(input)
    }
}

/// DPI
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Eq)]
pub struct Dpi(u16);

impl From<u16> for Dpi {
    #[inline]
    fn from(input: u16) -> Self {
        Dpi(input)
    }
}

/// Brightness
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq, Eq)]
pub struct Brightness(u8);

impl Default for Brightness {
    #[inline]
    fn default() -> Self {
        Brightness(100)
    }
}

impl TryFrom<u8> for Brightness {
    type Error = CommandError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value <= 100 {
            Ok(Brightness(value))
        } else {
            Err(CommandError::InvalidArgument(
                "brightness",
                format!("{} < {}", value, 100),
            ))
        }
    }
}

/// command to send to device to change color
#[derive(Clone, Debug)]
pub enum Command {
    ColorSector(RgbColor, Option<u8>),
    Breathe(RgbColor, Option<Speed>, Option<Brightness>),
    Cycle(Option<Speed>, Option<Brightness>),
    Wave(Direction, Option<Speed>, Option<Brightness>),
    Blend(Option<Speed>, Option<Brightness>),
    StartEffect(bool),
    Dpi(Dpi),
}

pub type UsbDevice = Device<Context>;

pub enum GDeviceManagerEvent {
    DevicePluggedIn(UsbDevice),
    DevicePluggedOut(UsbDevice),
    Shutdown,
}

#[derive(Debug)]
pub enum DeviceType {
    Keyboard,
    Mouse,
}

/// Driver for Logitech G devices
pub trait GDeviceDriver: Send {
    fn get_model(&self) -> GDeviceModelRef;
    fn open_device(&self, device: &UsbDevice) -> Option<Box<dyn GDevice>>;
}

pub type GDeviceDriverRef = Box<dyn GDeviceDriver>;

/// Logitech G device model series
///
/// Implementation is provided by a driver.
pub trait GDeviceModel: Send + Sync {
    fn get_sectors(&self) -> u8;

    fn get_default_color(&self) -> RgbColor;

    fn get_name(&self) -> &'static str;

    fn get_type(&self) -> DeviceType;

    fn usb_product_id(&self) -> u16;
}

pub type GDeviceModelRef = Arc<dyn GDeviceModel>;

/// Logitech G device
///
/// Implementation is provided by a driver.
pub trait GDevice: Display + Send {
    /// Return USB device reference.
    fn dev(&self) -> &UsbDevice;
    /// Return serial number
    fn serial_number(&self) -> &str;
    /// Return device model information
    fn get_model(&self) -> GDeviceModelRef;
    /// Send command to device
    fn send_command(&mut self, cmd: Command) -> CommandResult<()>;
}

pub type GDeviceRef = Box<dyn GDevice>;

pub struct GDeviceInfo {
    pub model: &'static str,
    pub serial: String,
}

quick_error! {
    #[derive(Debug)]
    pub enum CommandError {
        Usb(context: String, err: rusb::Error) {
            display("USB error: {}: {}", context, err)
            context(message: &'a str, err: rusb::Error)
                -> (message.to_string(), err)
        }
        InvalidArgument(arg: &'static str, msg: String) {
            display("Invalid argument {}: {}", arg, msg)
        }
        InvalidCommand {
            display("Invalid command")
        }
    }
}

type CommandResult<T> = Result<T, CommandError>;

impl PartialEq for Box<dyn GDeviceModel> {
    fn eq(&self, other: &Self) -> bool {
        self.get_name() == other.get_name()
    }
}

impl Eq for Box<dyn GDeviceModel> {}

impl Hash for Box<dyn GDeviceModel> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.get_name().as_bytes())
    }
}

struct GDeviceManagerState {
    pub context: Context,
    #[allow(dead_code)]
    hotplug: Registration<Context>,
    config: Config,
    devices: Vec<GDeviceRef>,
    drivers: Vec<GDeviceDriverRef>,
}

impl GDeviceManagerState {
    pub fn new(tx: mpsc::SyncSender<GDeviceManagerEvent>) -> CommandResult<Self> {
        let context = Context::new().context("creating USB context")?;
        let config = Config::load();
        Ok(Self {
            devices: vec![],
            config,
            drivers: vec![
                Box::<G213Driver>::default(),
                Box::<G203LightsyncDriver>::default(),
            ],
            hotplug: HotplugBuilder::new()
                .vendor_id(LOGITECH_USB_VENDOR_ID)
                .register(&context, Box::new(HotPlugHandler { channel: tx }))
                .context("registering hotplug callback")?,
            context,
        })
    }

    pub fn get_devices(&mut self) -> Vec<GDeviceInfo> {
        self.devices
            .iter()
            .map(|dev| GDeviceInfo {
                model: dev.get_model().get_name(),
                serial: dev.serial_number().to_string(),
            })
            .collect()
    }

    pub fn get_drivers(&mut self) -> Vec<&'static str> {
        self.drivers
            .iter()
            .map(|drv| drv.get_model().get_name())
            .collect()
    }

    pub fn load_devices(&mut self) -> CommandResult<()> {
        info!("Scan devices");
        let usb_devices = self.context.devices().context("listing USB devices")?;
        self.devices = usb_devices
            .iter()
            .filter_map(|device| self.try_open_device(&device))
            .collect();
        info!("Found {} device(s)", self.devices.len());
        self.apply_config();
        Ok(())
    }

    fn find_driver_for_device(&self, device: &Device<Context>) -> Option<&dyn GDeviceDriver> {
        let descriptor = device.device_descriptor().unwrap();
        if descriptor.vendor_id() == LOGITECH_USB_VENDOR_ID {
            self.drivers
                .iter()
                .find(|driver| descriptor.product_id() == driver.get_model().usb_product_id())
                .map(|driver| driver.deref())
        } else {
            None
        }
    }

    fn try_open_device(&self, device: &UsbDevice) -> Option<Box<dyn GDevice>> {
        if let Some(driver) = self.find_driver_for_device(device) {
            info!("Found device {}", driver.get_model().get_name());
            driver.open_device(device)
        } else {
            None
        }
    }

    pub fn send_command(&mut self, cmd: Command) {
        for device in &mut self.devices {
            if let Err(err) = device.send_command(cmd.clone()) {
                error!("Sending command failed for device: {:?}", err);
            }

            self.config.save_command(&*device.get_model(), cmd.clone())
        }
    }

    fn apply_config(&mut self) {
        for device in &mut self.devices {
            Self::apply_device_config(device, &self.config);
        }
    }

    fn apply_device_config(device: &mut GDeviceRef, config: &Config) {
        info!("Setting config for {}", device.get_model().get_name());
        for command in config.commands_for(&*device.get_model()) {
            if let Err(err) = device.send_command(command.clone()) {
                error!("Unable to send command to device {device}: {:?}", err);
            }
        }
    }

    pub fn refresh(&mut self) {
        info!("Refreshing");
        self.config = Config::load();
        self.apply_config();
    }

    pub fn on_new_usb_device(&mut self, dev: UsbDevice) {
        if let Some(mut gdev) = self.try_open_device(&dev) {
            if self.devices.iter().any(|existing| existing.dev() == &dev) {
                warn!("Plugged in device {} already exists", gdev)
            } else {
                info!("Device plugged in: {}", gdev);
                Self::apply_device_config(&mut gdev, &self.config);
                self.devices.push(gdev);
            }
        }
    }

    pub fn on_lost_usb_device(&mut self, dev: UsbDevice) {
        self.devices.retain(|existing| {
            if existing.dev() == &dev {
                info!("Device unplugged: {}", existing);
                false
            } else {
                true
            }
        });
    }
}

pub struct GDeviceManager {
    state: Mutex<GDeviceManagerState>,
    rx: Mutex<mpsc::Receiver<GDeviceManagerEvent>>,
    tx: mpsc::SyncSender<GDeviceManagerEvent>,
}

impl GDeviceManager {
    /// Try to create device manager with USB connection
    pub fn try_new() -> CommandResult<Self> {
        let (tx, rx) = mpsc::sync_channel(1024);
        let state = GDeviceManagerState::new(tx.clone())?;
        Ok(Self {
            tx,
            rx: Mutex::new(rx),
            state: Mutex::new(state),
        })
    }

    pub fn context(&self) -> Context {
        self.state().context.clone()
    }

    pub fn channel(&self) -> &mpsc::SyncSender<GDeviceManagerEvent> {
        &self.tx
    }

    pub fn load_devices(&self) -> CommandResult<()> {
        self.state().load_devices()
    }

    /// Send command to all devices
    pub fn list(&self) -> Vec<GDeviceInfo> {
        self.state().get_devices()
    }

    /// Send command to all devices
    pub fn list_drivers(&self) -> Vec<&'static str> {
        self.state().get_drivers()
    }

    /// Send command to all devices
    pub fn send_command(&self, cmd: Command) {
        self.state().send_command(cmd)
    }

    /// Send current config to device
    pub fn apply_config(&mut self) {
        self.state().apply_config()
    }

    /// Refresh config from filesystem and send config
    pub fn refresh(&self) {
        self.state().refresh()
    }

    pub fn run(&self) {
        while let Ok(msg) = self.rx.lock().unwrap().recv() {
            match msg {
                GDeviceManagerEvent::DevicePluggedIn(dev) => self.state().on_new_usb_device(dev),
                GDeviceManagerEvent::DevicePluggedOut(dev) => self.state().on_lost_usb_device(dev),
                GDeviceManagerEvent::Shutdown => break,
            }
        }
    }

    fn state(&self) -> MutexGuard<'_, GDeviceManagerState> {
        self.state.lock().unwrap()
    }
}

impl fmt::Debug for GDeviceManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("GDeviceManager")
    }
}

struct HotPlugHandler {
    channel: mpsc::SyncSender<GDeviceManagerEvent>,
}

impl HotPlugHandler {
    fn send(&self, cmd: GDeviceManagerEvent) {
        self.channel.send(cmd).expect("channel should be alive");
    }
}

impl Hotplug<Context> for HotPlugHandler {
    fn device_arrived(&mut self, device: UsbDevice) {
        self.send(GDeviceManagerEvent::DevicePluggedIn(device));
    }

    fn device_left(&mut self, device: UsbDevice) {
        self.send(GDeviceManagerEvent::DevicePluggedOut(device));
    }
}
