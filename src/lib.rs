use rusb::{Context, DeviceHandle as UsbDeviceHandle, DeviceList, Result, UsbContext};
use std::mem::MaybeUninit;
use std::{fmt, io, slice};
#[macro_use]
extern crate log;
#[macro_use]
extern crate quick_error;

use crate::Command::{Breathe, ColorSector, Cycle};

use crate::config::Config;
use crate::g213::G213Model;
use hex::FromHexError;
use ini::Ini;
use std::collections::HashMap;
use std::fmt::Pointer;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::time::Duration;
use structopt::StructOpt;

pub mod config;
pub mod g213;

/// RGB color
#[derive(Copy, Clone, Debug)]
pub struct RgbColor(pub u8, pub u8, pub u8);

impl RgbColor {
    pub fn red_channel(&self) -> u8 {
        self.0
    }

    pub fn green_channel(&self) -> u8 {
        self.1
    }

    pub fn blue_channel(&self) -> u8 {
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

/// speed of effect
#[derive(Copy, Clone, Debug, PartialOrd, PartialEq)]
pub struct Speed(u16);

impl From<u16> for Speed {
    fn from(input: u16) -> Self {
        Speed(input)
    }
}

/// command to send to device to change color
#[derive(Copy, Clone, Debug)]
pub enum Command {
    ColorSector(RgbColor, Option<u8>),
    Breathe(RgbColor, Speed),
    Cycle(Speed),
}

/// model series
pub trait GDeviceModel {
    fn find(&self, ctx: &DeviceList<Context>) -> Vec<Box<dyn GDevice>>;

    fn get_sectors(&self) -> u8;

    fn get_default_color(&self) -> RgbColor;

    fn get_name(&self) -> &'static str;
}

/// a device
pub trait GDevice {
    fn get_debug_info(&self) -> String;
    fn send_command(&self, cmd: &Command) -> Result<()>;
}

quick_error! {
    #[derive(Debug)]
    pub enum CommandError {
        Usb(err: rusb::Error) {
            from()
            display("USB error: {}", err)
            cause(err)
        }
        InvalidArgument(arg: &'static str, msg: String) {
            display("Invalid argument {}: {}", arg, msg)
        }
    }
}

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
    devices: HashMap<Box<dyn GDeviceModel>, Vec<Box<dyn GDevice>>>,
}

impl GDeviceManager {
    fn get_models() -> Vec<Box<dyn GDeviceModel>> {
        vec![Box::new(G213Model::new())]
    }

    pub fn try_new() -> Result<Self> {
        let context = Context::new()?;
        let usb_devices = context.devices()?;
        let models_list = Self::get_models();
        let devices = models_list
            .into_iter()
            .map(|model| {
                let devices = model.find(&usb_devices);
                (model, devices)
            })
            .collect::<HashMap<Box<dyn GDeviceModel>, Vec<Box<dyn GDevice>>>>();
        let config = Config::load();

        let mut self_ = Self {
            context,
            devices,
            config,
        };

        for (model, devices) in &self_.devices {
            for command in self_.config.commands_for(model.deref()) {
                for device in devices {
                    if let Err(err) = device.send_command(&command) {
                        error!("Sending command failed for device: {:?}", err);
                    }
                }
            }
        }

        Ok(self_)
    }

    pub fn send_command(&mut self, cmd: &Command) {
        for (model, devices) in &self.devices {
            for device in devices {
                if let Err(err) = device.send_command(cmd) {
                    error!("Sending command failed for device: {:?}", err);
                }
            }

            self.config.save_command(model.deref(), cmd)
        }
    }
}

impl fmt::Debug for GDeviceManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("GDeviceManager")
            .field(&self.devices.len())
            .finish()
    }
}
