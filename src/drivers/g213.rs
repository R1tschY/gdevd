use std::rc::Rc;

use rusb::{Context, Device};

use crate::drivers::{DeviceDescription, GUsbDriver};
use crate::{
    Brightness, Command, CommandError, CommandResult, DeviceType, Direction, Dpi, GDevice,
    GDeviceDriver, GDeviceModel, GDeviceModelRef, RgbColor, Speed,
};

const DEFAULT_RGB: RgbColor = RgbColor(0x00, 0xA9, 0xE0);

const DEVICE: DeviceDescription = DeviceDescription {
    product_id: 0xc336,
    min_speed: Speed(32), // ???
    default_speed: Speed(1000),
    max_speed: Speed(u16::MAX), // ???
    min_dpi: Dpi(u16::MAX),
};

pub struct G213Driver {
    model: GDeviceModelRef,
}

impl Default for G213Driver {
    fn default() -> Self {
        Self {
            model: Rc::new(G213Model),
        }
    }
}

impl GDeviceDriver for G213Driver {
    fn get_model(&self) -> GDeviceModelRef {
        self.model.clone()
    }

    fn open_device(&self, device: &Device<Context>) -> Option<Box<dyn GDevice>> {
        GUsbDriver::open_device(&DEVICE, device).map(|driver| {
            Box::new(G213Device {
                driver,
                model: self.model.clone(),
            }) as Box<dyn GDevice>
        })
    }
}

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

impl GDeviceModel for G213Model {
    fn get_sectors(&self) -> u8 {
        5
    }

    fn get_default_color(&self) -> RgbColor {
        DEFAULT_RGB
    }

    fn get_name(&self) -> &'static str {
        "G213"
    }

    fn get_type(&self) -> DeviceType {
        DeviceType::Keyboard
    }

    fn usb_product_id(&self) -> u16 {
        DEVICE.product_id
    }
}

pub struct G213Device {
    driver: GUsbDriver,
    model: GDeviceModelRef,
}

struct DeviceCommand {
    bytes: [u8; 20],
}

impl DeviceCommand {
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

    pub fn for_breathe(color: RgbColor, speed: Speed, brightness: Brightness) -> Self {
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
            speed.0 as u8,
            0,
            brightness.0,
        ])
    }

    pub fn for_cycle(speed: Speed, brightness: Brightness) -> Self {
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
            speed.0 as u8,
            brightness.0,
        ])
    }

    pub fn for_wave(direction: Direction, speed: Speed, brightness: Brightness) -> Self {
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
            speed.0 as u8,
            direction as u8,
            brightness.0,
            (speed.0 >> 8) as u8,
        ])
    }

    pub fn for_start_effect(state: bool) -> Self {
        Self::new(&[
            0x11,
            0xff,
            0x0c,
            0x5d,
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

impl GDevice for G213Device {
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
            ColorSector(rgb, sector) => {
                if let Some(sector) = sector {
                    if sector > 4 {
                        return Err(CommandError::InvalidArgument(
                            "sector",
                            format!("{} > 4", sector),
                        ));
                    }
                    interface.send_data(&DeviceCommand::for_region_color(sector, rgb).bytes)
                } else {
                    interface.send_data(&DeviceCommand::for_color(rgb).bytes)
                }
            }
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
            _ => Err(CommandError::InvalidCommand),
        }
    }
}
