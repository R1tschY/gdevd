use std::ops::{Deref, DerefMut};

use rusb::{DeviceHandle, Result, UsbContext};

/// Handle with detached kernel and claimed interface
pub struct DetachedHandle<'t, T: UsbContext> {
    handle: &'t mut DeviceHandle<T>,
    iface: u8,
    was_attached: bool,
}

impl<'t, T: UsbContext> DetachedHandle<'t, T> {
    pub fn new(handle: &'t mut DeviceHandle<T>, iface: u8) -> Result<Self> {
        let is_attached = handle.kernel_driver_active(iface)?;
        if is_attached {
            handle.detach_kernel_driver(iface)?;
        }
        handle.claim_interface(iface)?;

        Ok(Self {
            handle,
            iface,
            was_attached: is_attached,
        })
    }
}

impl<'t, T: UsbContext> Deref for DetachedHandle<'t, T> {
    type Target = DeviceHandle<T>;

    fn deref(&self) -> &Self::Target {
        self.handle
    }
}

impl<'t, T: UsbContext> DerefMut for DetachedHandle<'t, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.handle
    }
}

impl<'t, T: UsbContext> Drop for DetachedHandle<'t, T> {
    fn drop(&mut self) {
        if let Err(err) = self.handle.release_interface(self.iface) {
            warn!("Error while releasing usb interface: {:?}", err)
        }

        if self.was_attached {
            if let Err(err) = self.handle.attach_kernel_driver(self.iface) {
                warn!("Error while attaching kernel driver: {:?}", err)
            }
        }
    }
}
