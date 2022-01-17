use crate::drivers::{DeviceDescription, GUsbDriver};
use crate::usb_ext::DetachedHandle;
use crate::{
    Brightness, Command, CommandError, CommandResult, DeviceType, Direction, Dpi, GDevice,
    GDeviceDriver, GDeviceModel, GDeviceModelRef, GModelId, RgbColor, Speed,
};
use quick_error::ResultExt;
use rusb::{Context, Device, DeviceHandle, DeviceList, UsbContext};
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

const DEFAULT_DIRECTION: Direction = Direction::RightToLeft;

const DEVICE: DeviceDescription = DeviceDescription {
    product_id: 0xc092,
    min_speed: 1000,
    default_speed: Speed(10000),
    min_dpi: Dpi(50),
};

pub struct G203LightsyncDriver {
    model: GDeviceModelRef,
}

impl G203LightsyncDriver {
    pub fn new() -> Self {
        Self {
            model: Rc::new(G203LightsyncModel),
        }
    }
}

impl GDeviceDriver for G203LightsyncDriver {
    fn get_model(&self) -> GDeviceModelRef {
        self.model.clone()
    }

    fn open_device(&self, device: &Device<Context>) -> Option<Box<dyn GDevice>> {
        GUsbDriver::open_device(&DEVICE, device).map(|driver| {
            Box::new(G203LightsyncDevice {
                driver,
                model: self.model.clone(),
            }) as Box<dyn GDevice>
        })
    }
}

pub struct G203LightsyncModel;

impl G203LightsyncModel {
    pub fn new() -> Self {
        Self
    }
}

impl Default for G203LightsyncModel {
    fn default() -> Self {
        Self
    }
}

impl GDeviceModel for G203LightsyncModel {
    fn get_sectors(&self) -> u8 {
        3
    }

    fn get_default_color(&self) -> RgbColor {
        RgbColor(0, 0, 0) // TODO
    }

    fn get_name(&self) -> &'static str {
        "G203 LIGHTSYNC"
    }

    fn get_type(&self) -> DeviceType {
        DeviceType::Mouse
    }

    fn usb_product_id(&self) -> u16 {
        DEVICE.product_id
    }
}

pub struct G203LightsyncDevice {
    driver: GUsbDriver,
    model: GDeviceModelRef,
}

struct DeviceCommand {
    bytes: [u8; 20],
}
//00 00 00 00 00 00 00 01 00 00 00
impl DeviceCommand {
    pub fn for_color(color: RgbColor) -> Self {
        Self::new(&[
            0x11,
            0xff,
            0x0e,
            0x1b,
            0,
            0x01,
            color.red(),
            color.green(),
            color.blue(),
            0,
            0,
            0,
            0,
            0,
            0,
            0,
            1,
        ])
    }

    pub fn for_reset() -> Self {
        Self::new(&[0x10, 0xff, 0x0e, 0x5b, 0x01, 0x03, 0x05])
    }

    pub fn for_breathe(color: RgbColor, speed: Speed, brightness: Brightness) -> Self {
        Self::new(&[
            0x11,
            0xff,
            0x0e,
            0x1b,
            0,
            0x04,
            color.red(),
            color.green(),
            color.blue(),
            (speed.0 >> 8) as u8,
            (speed.0 >> 0) as u8,
            0,
            brightness.0,
            0,
            0,
            0,
            1,
        ])
    }

    pub fn for_cycle(speed: Speed, brightness: Brightness) -> Self {
        Self::new(&[
            0x11,
            0xff,
            0x0e,
            0x1b,
            0,
            0x02,
            0,
            0,
            0,
            0,
            0,
            (speed.0 >> 8) as u8,
            (speed.0 >> 0) as u8,
            brightness.0,
            0,
            0,
            1,
        ])
    }

    pub fn for_wave(direction: Direction, speed: Speed, brightness: Brightness) -> Self {
        Self::new(&[
            0x11,
            0xff,
            0x0e,
            0x1b,
            0,
            0x03,
            0,
            0,
            0,
            0,
            0,
            0,
            (speed.0 >> 0) as u8,
            direction as u8,
            brightness.0,
            (speed.0 >> 8) as u8,
            1,
        ])
    }

    pub fn for_blend(speed: Speed, brightness: Brightness) -> Self {
        Self::new(&[
            0x11,
            0xff,
            0x0e,
            0x1b,
            0,
            0x06,
            0,
            0,
            0,
            0,
            0,
            0,
            (speed.0 >> 0) as u8,
            (speed.0 >> 8) as u8,
            brightness.0,
            0,
            1,
        ])
    }

    pub fn for_triple(left: RgbColor, middle: RgbColor, right: RgbColor) -> Self {
        Self::new(&[
            0x11,
            0xff,
            0x12,
            0x1b,
            0x01,
            left.red(),
            left.green(),
            left.blue(),
            0x02,
            middle.red(),
            middle.green(),
            middle.blue(),
            0x03,
            right.red(),
            right.green(),
            right.blue(),
        ])
    }

    pub fn for_start_effect(state: bool) -> Self {
        Self::new(&[
            0x11,
            0xff,
            0x0e,
            0x3b,
            0x01,
            0x00,
            0x01,
            if state { 1 } else { 2 },
        ])
    }

    pub fn new(b: &[u8]) -> Self {
        let mut bytes = [0; 20];
        bytes[0..b.len()].copy_from_slice(b);
        Self { bytes }
    }
}

// Extra
// disable onboard memory: VALUE=0x210 DATA=10ff0e5b010305

fn sector_unsupported(sector: Option<u8>) -> CommandResult<()> {
    if sector.is_some() {
        Err(CommandError::InvalidArgument(
            "sector",
            "sector unsupported for G203 LIGHTSYNC".to_string(),
        ))
    } else {
        Ok(())
    }
}

impl GDevice for G203LightsyncDevice {
    fn get_debug_info(&self) -> String {
        self.driver.debug_info()
    }

    fn get_model(&self) -> GDeviceModelRef {
        self.model.clone()
    }

    fn send_command(&mut self, cmd: Command) -> CommandResult<()> {
        use Command::*;

        let interface = self.driver.open_interface()?;
        interface.send_data(&DeviceCommand::for_reset().bytes)?;

        match cmd {
            Breathe(rgb, speed, brightness) => interface.send_data(
                &DeviceCommand::for_breathe(
                    rgb,
                    DEVICE.get_speed(speed)?,
                    brightness.unwrap_or_default(),
                )
                .bytes,
            ),
            Cycle(speed, brightness) => interface.send_data(
                &DeviceCommand::for_cycle(DEVICE.get_speed(speed)?, brightness.unwrap_or_default())
                    .bytes,
            ),
            Wave(direction, speed, brightness) => interface.send_data(
                &DeviceCommand::for_wave(
                    direction,
                    DEVICE.get_speed(speed)?,
                    brightness.unwrap_or_default(),
                )
                .bytes,
            ),
            StartEffect(state) => {
                interface.send_data(&DeviceCommand::for_start_effect(state).bytes)
            }
            ColorSector(color, sector) => {
                sector_unsupported(sector)?;
                interface.send_data(&DeviceCommand::for_color(color).bytes)
            }
            _ => Err(CommandError::InvalidCommand),
        }
    }
}
