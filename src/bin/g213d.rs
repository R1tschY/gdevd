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
use gdev::{GDeviceManager, RgbColor};
use std::cell::RefCell;
use std::convert::TryInto;

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

fn create_interface() -> Interface<MTFn<TreeData>, TreeData> {
    let f = Factory::new_fn::<TreeData>();
    f.interface("de.richardliebscher.g213d.GDeviceManager", ())
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
                let (color, speed): (&str, u16) = m.msg.read2()?;
                let rgb =
                    RgbColor::from_hex(color).map_err(|_err| MethodErr::invalid_arg("color"))?;

                info!("Set breathe mode with {} and {}", color, speed);
                manager.send_command(Breathe(rgb, speed.into()));

                Ok(vec![m.msg.method_return()])
            })
            .inarg::<&str, _>("color")
            .inarg::<u16, _>("speed"),
        )
        .add_m(
            f.method("cycle", (), move |m| {
                let mut manager = m.path.get_data().borrow_mut();
                let speed: u16 = m.msg.read1()?;

                info!("Set cycle mode with {}", speed);
                manager.send_command(Cycle(speed.into()));

                Ok(vec![m.msg.method_return()])
            })
            .inarg::<u16, _>("speed"),
        )
        .add_m(
            f.method("wave", (), move |m| {
                let mut manager = m.path.get_data().borrow_mut();
                let (direction, speed): (&str, u16) = m.msg.read2()?;

                info!("Set wave with {} in {:?}", speed, direction);
                manager.send_command(Wave(
                    direction
                        .try_into()
                        .map_err(|_err| MethodErr::invalid_arg("direction"))?,
                    speed.into(),
                ));

                Ok(vec![m.msg.method_return()])
            })
            .inarg::<&str, _>("direction")
            .inarg::<u16, _>("speed"),
        )
        .add_m(f.method("refresh", (), move |m| {
            let mut manager = m.path.get_data().borrow_mut();

            info!("Refresh");
            manager.refresh();

            Ok(vec![m.msg.method_return()])
        }))
}

fn main() -> Result<(), Box<dyn Error>> {
    simple_logger::init()?;

    let mut c = LocalConnection::new_system()?;
    c.request_name("de.richardliebscher.g213d", false, true, true)?;

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
