use crate::usb_ext::DetachedHandle;
use crate::{
    Command, CommandError, CommandResult, Direction, GDevice, GDeviceModel, RgbColor, Speed,
};
use quick_error::ResultExt;
use rusb::{Context, Device, DeviceHandle, DeviceList, UsbContext};
use std::time::Duration;

// Standard color, i found this color to produce a white color on my G213
//const STANDARD_COLOR_HEX: &str = "ffb4aa";
// The id of the Logitech company
const ID_VENDOR: u16 = 0x046d;
// The id of the G213
const ID_PRODUCT: u16 = 0xc336;
// Endpoint to read data back from
const ENDPOINT_ADDRESS: u8 = 0x82;
// --.
const REQUEST_TYPE: u8 = 0x21;
//    \ The control transfer
const REQUEST: u8 = 0x09;
//    / configuration for the G213
const VALUE: i32 = 0x0211;
// --'
const INTERFACE: u8 = 0x0001;

// const DEFAULT_FREQUENCY: u16 = 1000;
// const DEFAULT_BRIGHTNESS: u8 = 100;
const DEFAULT_RGB: RgbColor = RgbColor(0x00, 0xA9, 0xE0);

pub struct G213Model;

impl G213Model {
    pub fn new() -> Self {
        Self
    }
}

impl Default for G213Model {
    fn default() -> Self {
        Self
    }
}

impl G213Model {
    fn try_open_device(device: &Device<Context>) -> CommandResult<Box<dyn GDevice>> {
        Ok(Box::new(G213Device {
            handle: device.open().context("opening G213 USB device")?,
        }))
    }

    fn open_device(device: &Device<Context>) -> Option<Box<dyn GDevice>> {
        Self::try_open_device(device)
            .map_err(|err| {
                warn!("Failed to open G213 device: {:?}", err);
                err
            })
            .ok()
    }
}

impl GDeviceModel for G213Model {
    fn find(&self, devices: &DeviceList<Context>) -> Vec<Box<dyn GDevice>> {
        devices
            .iter()
            .filter(|device| {
                let device_descriptor = device.device_descriptor().unwrap();
                device_descriptor.product_id() == ID_PRODUCT
                    && device_descriptor.vendor_id() == ID_VENDOR
            })
            .flat_map(|device| Self::open_device(&device))
            .collect()
    }

    fn get_sectors(&self) -> u8 {
        5
    }

    fn get_default_color(&self) -> RgbColor {
        DEFAULT_RGB
    }

    fn get_name(&self) -> &'static str {
        "G213"
    }
}

pub struct G213Device {
    handle: DeviceHandle<Context>,
}

impl G213Device {
    fn send_data<'t, T: UsbContext>(
        handle: &mut DeviceHandle<T>,
        data: &UsbCommand,
    ) -> CommandResult<()> {
        handle
            .write_control(
                REQUEST_TYPE,
                REQUEST,
                VALUE as u16,
                INTERFACE as u16,
                &data.bytes,
                Duration::from_secs(5),
            )
            .context("write_control")?;

        let mut data = [0u8; 20];
        handle
            .read_interrupt(ENDPOINT_ADDRESS, &mut data, Duration::from_secs(5))
            .context("read_interrupt")?;

        Ok(())
    }
}

fn check_speed(speed: Speed) -> CommandResult<()> {
    if speed.0 < 32 {
        Err(CommandError::InvalidArgument(
            "speed",
            format!("{} < 32", speed.0),
        ))
    } else {
        Ok(())
    }
}

struct UsbCommand {
    bytes: [u8; 20],
}

impl UsbCommand {
    pub fn for_color(color: RgbColor) -> Self {
        Self::new(&[
            0x11,
            0xff,
            0x0c,
            0x3a,
            0,
            0x01,
            color.red(),
            color.green(),
            color.blue(),
            0x02,
        ])
    }

    pub fn for_region_color(region: u8, color: RgbColor) -> Self {
        Self::new(&[
            0x11,
            0xff,
            0x0c,
            0x3a,
            region + 1,
            0x01,
            color.red(),
            color.green(),
            color.blue(),
            0x02,
        ])
    }

    pub fn for_reset() -> Self {
        Self::new(&[0x11, 0xff, 0x0c, 0x0d])
    }

    pub fn for_breathe(color: RgbColor, speed: Speed) -> Self {
        Self::new(&[
            0x11,
            0xff,
            0x0c,
            0x3a,
            0,
            0x02,
            color.red(),
            color.green(),
            color.blue(),
            (speed.0 >> 8) as u8,
            (speed.0 >> 0) as u8,
        ])
    }

    pub fn for_cycle(speed: Speed) -> Self {
        Self::new(&[
            0x11,
            0xff,
            0x0c,
            0x3a,
            0,
            0x03,
            0xff,
            0xff,
            0xff,
            0,
            0,
            (speed.0 >> 8) as u8,
            (speed.0 >> 0) as u8,
            0x64,
        ])
    }

    pub fn for_wave(direction: Direction, speed: Speed) -> Self {
        Self::new(&[
            0x11,
            0xff,
            0x0c,
            0x3a,
            0,
            0x04,
            0x00,
            0x00,
            0x00,
            0,
            0,
            0,
            (speed.0 >> 0) as u8,
            direction as u8,
            0x64,
            (speed.0 >> 8) as u8,
        ])
    }

    pub fn new(b: &[u8]) -> Self {
        let mut bytes = [0; 20];
        bytes[0..b.len()].copy_from_slice(b);
        Self { bytes }
    }
}

impl GDevice for G213Device {
    fn get_debug_info(&self) -> String {
        unimplemented!()
    }

    fn send_command(&mut self, cmd: Command) -> CommandResult<()> {
        use Command::*;

        let mut handle = DetachedHandle::new(&mut self.handle, INTERFACE)
            .context("detaching USB device from kernel")?;

        Self::send_data(&mut handle, &UsbCommand::for_reset())?;

        match cmd {
            ColorSector(rgb, sector) => {
                if let Some(sector) = sector {
                    if sector > 4 {
                        return Err(CommandError::InvalidArgument(
                            "sector",
                            format!("{} > 4", sector),
                        ));
                    }
                    Self::send_data(&mut handle, &UsbCommand::for_region_color(sector, rgb))
                } else {
                    Self::send_data(&mut handle, &UsbCommand::for_color(rgb))
                }
            }
            Breathe(rgb, speed) => {
                check_speed(speed)?;
                Self::send_data(&mut handle, &UsbCommand::for_breathe(rgb, speed))
            }
            Cycle(speed) => {
                check_speed(speed)?;
                Self::send_data(&mut handle, &UsbCommand::for_cycle(speed))
            }
            Wave(direction, speed) => {
                check_speed(speed)?;
                Self::send_data(&mut handle, &UsbCommand::for_wave(direction, speed))
            }
        }
    }
}
