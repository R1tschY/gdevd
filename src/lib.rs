use rusb::{Context, Device, DeviceList, UsbContext};
use std::fmt;
#[macro_use]
extern crate log;
#[macro_use]
extern crate quick_error;

use crate::config::Config;
use crate::g213::{G213Driver, G213Model};
use hex::FromHexError;
use quick_error::ResultExt;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::rc::Rc;
use std::sync::Arc;

pub mod config;
pub mod g213;
pub mod usb_ext;

const LOGITECH_USB_VENDOR_ID: u16 = 0x046d;

/// RGB color
#[derive(Clone, Debug)]
pub struct RgbColor(pub u8, pub u8, pub u8);

impl RgbColor {
    pub fn red(&self) -> u8 {
        self.0
    }

    pub fn green(&self) -> u8 {
        self.1
    }

    pub fn blue(&self) -> u8 {
        self.2
    }

    pub fn from_hex(rgb_hex: &str) -> std::result::Result<Self, FromHexError> {
        let mut bytes = [0u8; 3];
        hex::decode_to_slice(rgb_hex, &mut bytes as &mut [u8])?;
        Ok(RgbColor(bytes[0], bytes[1], bytes[2]))
    }

    pub fn to_hex(&self) -> String {
        hex::encode(&[self.0, self.1, self.2])
    }

    pub fn to_int(&self) -> u32 {
        ((self.0 as u32) << 16) | ((self.1 as u32) << 8) | (self.2 as u32)
    }
}

#[derive(PartialEq, Debug, Copy, Clone)]
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
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq)]
pub struct Speed(u16);

impl From<u16> for Speed {
    fn from(input: u16) -> Self {
        Speed(input)
    }
}

/// command to send to device to change color
#[derive(Clone, Debug)]
pub enum Command {
    ColorSector(RgbColor, Option<u8>),
    Breathe(RgbColor, Speed),
    Cycle(Speed),
    Wave(Direction, Speed),
    StartEffect(bool),
}

#[derive(Debug)]
pub enum DeviceType {
    Keyboard,
    Mouse,
}

pub struct GModelId(String);

pub trait GDeviceDriver {
    fn get_model(&self) -> GDeviceModelRef;
    fn open_device(&self, device: &Device<Context>) -> Option<Box<dyn GDevice>>;
}

pub type GDeviceDriverRef = Box<dyn GDeviceDriver>;

/// model series
pub trait GDeviceModel {
    fn get_sectors(&self) -> u8;

    fn get_default_color(&self) -> RgbColor;

    fn get_name(&self) -> &'static str;

    fn get_type(&self) -> DeviceType;

    fn usb_product_id(&self) -> u16;
}

pub type GDeviceModelRef = Rc<dyn GDeviceModel>;

/// a device
pub trait GDevice {
    fn get_debug_info(&self) -> String;
    fn get_model(&self) -> GDeviceModelRef;
    fn send_command(&mut self, cmd: Command) -> CommandResult<()>;
}

pub type GDeviceRef = Box<dyn GDevice>;

quick_error! {
    #[derive(Debug)]
    pub enum CommandError {
        Usb(context: String, err: rusb::Error) {
            display("USB error: {}: {}", context, err)
            cause(err)
            context(message: &'a str, err: rusb::Error)
                -> (message.to_string(), err)
        }
        InvalidArgument(arg: &'static str, msg: String) {
            display("Invalid argument {}: {}", arg, msg)
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

pub struct GDeviceManager {
    context: Context,
    config: Config,
    drivers: Vec<GDeviceDriverRef>,
    devices: Vec<GDeviceRef>,
}

impl GDeviceManager {
    /// Try to create device manager with USB connection
    pub fn try_new() -> CommandResult<Self> {
        let context = Context::new().context("creating USB context")?;
        let config = Config::load();
        Ok(Self {
            context,
            drivers: vec![Box::new(G213Driver::new())],
            devices: vec![],
            config,
        })
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

    fn try_open_device(&self, device: &Device<Context>) -> Option<Box<dyn GDevice>> {
        if let Some(driver) = self.find_driver_for_device(&device) {
            info!("Found device {}", driver.get_model().get_name());
            driver.open_device(&device)
        } else {
            None
        }
    }

    /// Send command to all devices
    pub fn list(&self) -> &[GDeviceRef] {
        info!("List {} device(s)", self.devices.len());
        &self.devices
    }

    /// Send command to all devices
    pub fn list_drivers(&self) -> &[GDeviceDriverRef] {
        &self.drivers
    }

    /// Send command to all devices
    pub fn send_command(&mut self, cmd: Command) {
        for device in &mut self.devices {
            if let Err(err) = device.send_command(cmd.clone()) {
                error!("Sending command failed for device: {:?}", err);
            }

            self.config.save_command(&*device.get_model(), cmd.clone())
        }
    }

    /// Send current config to device
    pub fn apply_config(&mut self) {
        for device in &mut self.devices {
            info!("Setting config for {}", device.get_model().get_name());
            for command in self.config.commands_for(&*device.get_model()) {
                if let Err(err) = device.send_command(command.clone()) {
                    error!("Sending command failed for device: {:?}", err);
                }
            }
        }
    }

    /// Refresh config from filesystem and send config
    pub fn refresh(&mut self) {
        info!("Refreshing");
        self.config = Config::load();
        self.apply_config();
    }
}

impl fmt::Debug for GDeviceManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("GDeviceManager")
            .field(&self.devices.len())
            .finish()
    }
}
