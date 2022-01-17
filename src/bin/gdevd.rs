extern crate dbus;
#[macro_use]
extern crate log;

use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use dbus::blocking::LocalConnection;
use dbus::tree;
use dbus::tree::{Factory, Interface, MTFn, MethodErr};

use gdev::Command::{Breathe, ColorSector, Cycle, Wave};
use gdev::{Brightness, CommandError, GDeviceManager, RgbColor};
use std::cell::RefCell;
use std::convert::{TryFrom, TryInto};

#[derive(Copy, Clone, Default, Debug)]
struct TreeData;

impl tree::DataType for TreeData {
    type Tree = ();
    type ObjectPath = Arc<RefCell<GDeviceManager>>;
    type Property = ();
    type Interface = ();
    type Method = ();
    type Signal = ();
}

fn parse_brightness(brightness: u8) -> Result<Option<Brightness>, MethodErr> {
    match Brightness::try_from(brightness) {
        Ok(brightness) => Ok(Some(brightness)),
        Err(_) => Err(MethodErr::invalid_arg(
            "brightness must be between 0 and 100",
        )),
    }
}

fn create_interface() -> Interface<MTFn<TreeData>, TreeData> {
    // TODO: missing commands: start, blend, dpi
    let f = Factory::new_fn::<TreeData>();
    f.interface("de.richardliebscher.gdevd.GDeviceManager", ())
        .add_m(
            f.method("list_drivers", (), move |m| {
                let mut manager = m.path.get_data().borrow_mut();

                let drivers = manager.list_drivers();
                let drivers_info: Vec<(&str,)> = drivers
                    .iter()
                    .map(|driver| (driver.get_model().get_name(),))
                    .collect();
                Ok(vec![m.msg.method_return().append1(drivers_info)])
            })
            .outarg::<&[(&str,)], _>("drivers"),
        )
        .add_m(
            f.method("list", (), move |m| {
                let mut manager = m.path.get_data().borrow_mut();

                let devices = manager.list();
                let devices_info: Vec<(&str, String)> = devices
                    .iter()
                    .map(|device| (device.get_model().get_name(), device.get_debug_info()))
                    .collect();
                Ok(vec![m.msg.method_return().append1(devices_info)])
            })
            .outarg::<&[(&str, &str)], _>("devices"),
        )
        .add_m(
            f.method("color_sector", (), move |m| {
                let mut manager = m.path.get_data().borrow_mut();
                let (color, sector): (&str, u8) = m.msg.read2()?;
                let rgb =
                    RgbColor::from_hex(color).map_err(|_err| MethodErr::invalid_arg("color"))?;

                info!("Color sector {} with {}", sector, color);
                manager.send_command(ColorSector(rgb, Some(sector)));

                Ok(vec![m.msg.method_return()])
            })
            .inarg::<&str, _>("color")
            .inarg::<u8, _>("sector"),
        )
        .add_m(
            f.method("color_sectors", (), move |m| {
                let mut manager = m.path.get_data().borrow_mut();
                let color: &str = m.msg.read1()?;
                let rgb =
                    RgbColor::from_hex(color).map_err(|_err| MethodErr::invalid_arg("color"))?;

                info!("Color sectors with {}", color);
                manager.send_command(ColorSector(rgb, None));

                Ok(vec![m.msg.method_return()])
            })
            .inarg::<&str, _>("color"),
        )
        .add_m(
            f.method("breathe", (), move |m| {
                let mut manager = m.path.get_data().borrow_mut();
                let (color, speed, brightness): (&str, u16, u8) = m.msg.read3()?;
                let rgb =
                    RgbColor::from_hex(color).map_err(|_err| MethodErr::invalid_arg("color"))?;

                info!(
                    "Set breathe mode: color={} speed={} brightness={}",
                    color, speed, brightness
                );
                manager.send_command(Breathe(
                    rgb,
                    Some(speed.into()),
                    parse_brightness(brightness)?,
                ));

                Ok(vec![m.msg.method_return()])
            })
            .inarg::<&str, _>("color")
            .inarg::<u16, _>("speed")
            .inarg::<u8, _>("brightness"),
        )
        .add_m(
            f.method("cycle", (), move |m| {
                let mut manager = m.path.get_data().borrow_mut();
                let (speed, brightness): (u16, u8) = m.msg.read2()?;

                info!("Set cycle mode: speed={} brightness={}", speed, brightness);
                manager.send_command(Cycle(Some(speed.into()), parse_brightness(brightness)?));

                Ok(vec![m.msg.method_return()])
            })
            .inarg::<u16, _>("speed")
            .inarg::<u8, _>("brightness"),
        )
        .add_m(
            f.method("wave", (), move |m| {
                let mut manager = m.path.get_data().borrow_mut();
                let (direction, speed, brightness): (&str, u16, u8) = m.msg.read3()?;

                info!(
                    "Set wave: speed={} direction={:?} brightness={}",
                    speed, direction, brightness
                );
                manager.send_command(Wave(
                    direction
                        .try_into()
                        .map_err(|_err| MethodErr::invalid_arg("direction"))?,
                    Some(speed.into()),
                    parse_brightness(brightness)?,
                ));

                Ok(vec![m.msg.method_return()])
            })
            .inarg::<&str, _>("direction")
            .inarg::<u16, _>("speed")
            .inarg::<u8, _>("brightness"),
        )
        .add_m(f.method("refresh", (), move |m| {
            let mut manager = m.path.get_data().borrow_mut();

            info!("Refresh");
            manager.refresh();

            Ok(vec![m.msg.method_return()])
        }))
}

fn main() -> Result<(), Box<dyn Error>> {
    simple_logger::init_by_env();

    let mut c = LocalConnection::new_system()?;
    c.request_name("de.richardliebscher.gdevd", false, true, true)?;

    let device_manager_if = create_interface();
    let mut device_manager = GDeviceManager::try_new()?;
    device_manager.load_devices();

    let f = Factory::new_fn::<TreeData>();
    let tree = f.tree(()).add(
        f.object_path("/devices", Arc::new(RefCell::new(device_manager)))
            .introspectable()
            .add(device_manager_if),
    );

    tree.start_receive(&c);

    info!("Starting server");
    loop {
        c.process(Duration::from_millis(60000))?;
    }
}
