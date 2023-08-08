#[macro_use]
extern crate log;

use std::convert::{TryFrom, TryInto};
use std::error::Error;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use dbus::blocking::LocalConnection;
use dbus::MethodErr;
use dbus_tree::{Factory, Interface, MTFn};
use rusb::UsbContext;

use gdev::Command::{Breathe, ColorSector, Cycle, Wave};
use gdev::{Brightness, GDeviceManager, RgbColor};

#[derive(Copy, Clone, Default, Debug)]
struct TreeData;

impl dbus_tree::DataType for TreeData {
    type Tree = ();
    type ObjectPath = Arc<GDeviceManager>;
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
                let manager = m.path.get_data();
                let drivers: Vec<(&str,)> = manager
                    .list_drivers()
                    .iter()
                    .map(|driver| (*driver,))
                    .collect();
                Ok(vec![m.msg.method_return().append1(drivers)])
            })
            .outarg::<&[(&str,)], _>("drivers"),
        )
        .add_m(
            f.method("list", (), move |m| {
                let manager = m.path.get_data();
                let devices = manager.list();
                let devices_info: Vec<(&str, &str)> = devices
                    .iter()
                    .map(|dev| (dev.model, &dev.serial as &str))
                    .collect();
                Ok(vec![m.msg.method_return().append1(devices_info)])
            })
            .outarg::<&[(&str, &str)], _>("devices"),
        )
        .add_m(
            f.method("color_sector", (), move |m| {
                let manager = m.path.get_data();
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
                let manager = m.path.get_data();
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
                let manager = m.path.get_data();
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
                let manager = m.path.get_data();
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
                let manager = m.path.get_data();
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
            let manager = m.path.get_data();

            info!("Refresh");
            manager.refresh();

            Ok(vec![m.msg.method_return()])
        }))
}

fn main() -> Result<(), Box<dyn Error>> {
    simple_logger::init_with_env()?;

    // Register DBus service
    let c = LocalConnection::new_system()?;
    c.request_name("de.richardliebscher.gdevd", false, true, true)?;

    // Start USB service
    let device_manager = Arc::new(GDeviceManager::try_new()?);
    device_manager.load_devices()?;

    let usb_context = device_manager.context().clone();
    let _events_thd = thread::spawn(move || loop {
        if let Err(err) = usb_context.handle_events(None) {
            error!("libusb event handling aborted: {err}");
            break;
        }
    });
    let gdevmgr = device_manager.clone();
    let _gdevmgr_thd = thread::spawn(move || {
        gdevmgr.run();
    });

    // DBus
    let device_manager_if = create_interface();
    let f = Factory::new_fn::<TreeData>();
    let tree = f.tree(()).add(
        f.object_path("/devices", device_manager.clone())
            .introspectable()
            .add(device_manager_if),
    );

    tree.start_receive(&c);

    info!("Starting server");
    loop {
        c.process(Duration::from_millis(60000))?;
    }
}
