use std::time::Duration;

use quick_error::ResultExt;
use rusb::{Context, Device, DeviceHandle};

use crate::usb_ext::DetachedHandle;
use crate::{CommandError, CommandResult, Dpi, Speed, UsbDevice};

pub mod g203_lightsync;
pub mod g213;

// USB interface constants
const ENDPOINT_ADDRESS: u8 = 0x82;
const REQUEST_TYPE: u8 = 0x21; // request_type(Direction::Out, RequestType::Class, Recipient::Interface);
const REQUEST: u8 = 0x09; // HID_REQ_SET_REPORT
const VALUE: i32 = 0x0211;
const INTERFACE: u8 = 0x0001;

struct DeviceDescription {
    product_id: u16,
    min_speed: Speed,
    default_speed: Speed,
    max_speed: Speed,
    #[allow(unused)]
    min_dpi: Dpi,
}

impl DeviceDescription {
    fn get_speed(&self, speed: Option<Speed>) -> CommandResult<Speed> {
        if let Some(speed) = speed {
            if speed < self.min_speed {
                return Err(CommandError::InvalidArgument(
                    "speed",
                    format!("{} < {}", speed.0, self.min_speed.0),
                ));
            }
            if speed > self.max_speed {
                return Err(CommandError::InvalidArgument(
                    "speed",
                    format!("{} > {}", speed.0, self.max_speed.0),
                ));
            }
        }
        Ok(speed.unwrap_or(self.default_speed))
    }

    #[allow(unused)]
    fn check_dpi(&self, dpi: Dpi) -> CommandResult<()> {
        assert_ne!(self.min_dpi.0, u16::MAX);
        if dpi < self.min_dpi {
            Err(CommandError::InvalidArgument(
                "speed",
                format!("{} < {}", dpi.0, self.min_dpi.0),
            ))
        } else {
            Ok(())
        }
    }
}

struct GUsbDriver {
    dev: UsbDevice,
    serial_number: String,
    handle: DeviceHandle<Context>,
    description: &'static DeviceDescription,
}

impl GUsbDriver {
    pub fn open_device(
        description: &'static DeviceDescription,
        device: &Device<Context>,
    ) -> Option<Self> {
        match Self::try_open_device(description, device) {
            Ok(s) => Some(s),
            Err(err) => {
                warn!("Failed to open USB device: {:?}", err);
                None
            }
        }
    }

    pub fn try_open_device(
        description: &'static DeviceDescription,
        device: &Device<Context>,
    ) -> CommandResult<Self> {
        debug!("Opening device");
        let handle = device.open().context("opening USB device")?;
        let descriptor = device
            .device_descriptor()
            .context("reading device descriptor")?;
        Ok(Self {
            description,
            dev: device.clone(),
            serial_number: handle
                .read_serial_number_string_ascii(&descriptor)
                .context("reading serial number")?,
            handle,
        })
    }

    fn open_interface(&mut self) -> CommandResult<GInterface<'_>> {
        let handle = DetachedHandle::new(&mut self.handle, INTERFACE)
            .context("detaching USB device from kernel")?;
        Ok(GInterface {
            handle,
            description: self.description,
        })
    }

    fn serial_number(&self) -> &str {
        &self.serial_number
    }

    fn dev(&self) -> &UsbDevice {
        &self.dev
    }
}

struct GInterface<'t> {
    #[allow(unused)]
    handle: DetachedHandle<'t, Context>,
    #[allow(unused)]
    description: &'static DeviceDescription,
}

impl<'t> GInterface<'t> {
    fn send_data(&self, data: &[u8]) -> CommandResult<()> {
        debug!("Sending command");

        self.handle
            .write_control(
                REQUEST_TYPE,
                REQUEST,
                VALUE as u16,
                INTERFACE as u16,
                data,
                Duration::from_secs(5),
            )
            .context("write_control")?;

        let mut dummy = [0u8; 20];
        self.handle
            .read_interrupt(ENDPOINT_ADDRESS, &mut dummy, Duration::from_secs(5))
            .context("read_interrupt")?;

        Ok(())
    }
}
