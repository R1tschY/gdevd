use rusb::{DeviceHandle, Result, UsbContext};
use std::ops::Deref;

/// Handle with detached kernel
pub struct DetachedHandle<'t, T: UsbContext> {
    handle: &'t mut DeviceHandle<T>,
    index: u8,
    was_attached: bool,
}

impl<'t, T: UsbContext> DetachedHandle<'t, T> {
    pub fn new(handle: &'t mut DeviceHandle<T>, index: u8) -> Result<Self> {
        let is_attached = handle.kernel_driver_active(index)?;
        if is_attached {
            handle.detach_kernel_driver(index)?;
        }

        Ok(Self {
            handle,
            index,
            was_attached: is_attached,
        })
    }
}

impl<'t, T: UsbContext> Deref for DetachedHandle<'t, T> {
    type Target = DeviceHandle<T>;

    fn deref(&self) -> &Self::Target {
        &self.handle
    }
}

impl<'t, T: UsbContext> Drop for DetachedHandle<'t, T> {
    fn drop(&mut self) {
        if self.was_attached {
            self.handle.attach_kernel_driver(self.index);
        }
    }
}
