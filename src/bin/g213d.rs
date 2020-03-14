extern crate dbus;
#[macro_use]
extern crate log;

use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use dbus::blocking::LocalConnection;
use dbus::tree;
use dbus::tree::{Factory, Interface, MTFn, MethodErr};

use g213d::Command::ColorSector;
use g213d::{GDevice, GDeviceManager, RgbColor};
use std::cell::RefCell;

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
            f.method("color_sector", (), move |m| {
                let mut manager = m.path.get_data().borrow_mut();
                let (color, sector): (&str, u8) = m.msg.read2()?;
                let rgb =
                    RgbColor::from_hex(color).map_err(|_err| MethodErr::invalid_arg("color"))?;

                info!("Color sector {} with {}", sector, color);
                manager.send_command(&ColorSector(rgb, Some(sector)));

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
                manager.send_command(&ColorSector(rgb, None));

                Ok(vec![m.msg.method_return()])
            })
            .inarg::<&str, _>("color"),
        )
}

fn main() -> Result<(), Box<dyn Error>> {
    simple_logger::init()?;

    let mut c = LocalConnection::new_system()?;
    c.request_name("de.richardliebscher.g213d", false, true, true)?;

    let device_manager_if = create_interface();
    let device_manager = GDeviceManager::try_new()?;

    let f = Factory::new_fn::<TreeData>();
    let tree = f.tree(()).add(
        f.object_path("/devices", Arc::new(RefCell::new(device_manager)))
            .introspectable()
            .add(device_manager_if),
    );

    tree.start_receive(&c);

    info!("Starting server");
    loop {
        c.process(Duration::from_millis(1000))?;
    }
}
