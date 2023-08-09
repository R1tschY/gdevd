#[macro_use]
extern crate log;

use std::convert::{TryFrom, TryInto};
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use dbus::blocking::Connection;
use dbus::MethodErr;
use dbus_tree::{Factory, Interface, MTSync};
use rusb::UsbContext;
use signal_hook::consts::TERM_SIGNALS;
use signal_hook::iterator::Signals;

use gdevd::Command::{Breathe, ColorSector, Cycle, Wave};
use gdevd::{Brightness, GDeviceManager, GDeviceManagerEvent, RgbColor};

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

fn create_interface() -> Interface<MTSync<TreeData>, TreeData> {
    // TODO: missing commands: start, blend, dpi
    let f = Factory::new_sync::<TreeData>();
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
    let term_now = register_forced_shutdown()?;
    let mut signals = Signals::new(TERM_SIGNALS)?;
    let sigs_handle = signals.handle();

    simple_logger::init_with_env()?;

    // Register DBus service
    let c = Connection::new_system()?;
    c.request_name("de.richardliebscher.gdevd", false, false, true)?;

    // Start USB service
    let device_manager = Arc::new(GDeviceManager::try_new()?);
    device_manager.load_devices()?;

    let gdevmgr = device_manager.clone();
    let usb_context = device_manager.context();
    let term_now_ = term_now.clone();
    let events_thd = thread::spawn(move || {
        while !term_now_.load(Ordering::Relaxed) {
            if let Err(err) = usb_context.handle_events(None) {
                error!("libusb event handling aborted: {err}");
                let _ = gdevmgr.channel().send(GDeviceManagerEvent::Shutdown);
                return;
            }
        }
    });

    // DBus
    let devmgr = device_manager.clone();
    let term_now_ = term_now.clone();
    let dbus_thd = thread::spawn(move || {
        let device_manager_if = create_interface();
        let f = Factory::new_sync::<TreeData>();
        let tree = f.tree(()).add(
            f.object_path("/devices", devmgr.clone())
                .introspectable()
                .add(device_manager_if),
        );

        tree.start_receive_send(&c);

        info!("Starting DBus server");
        while !term_now_.load(Ordering::Relaxed) {
            if let Err(err) = c.process(Duration::from_millis(2000)) {
                error!("DBus server aborted: {err}");
                let _ = devmgr.channel().send(GDeviceManagerEvent::Shutdown);
                return;
            }
        }
    });

    // Signals
    let gdevmgr = device_manager.clone();
    let sigs_thd = thread::spawn(move || {
        if signals.forever().next().is_some() {
            let _ = gdevmgr.channel().send(GDeviceManagerEvent::Shutdown);
        }
    });

    // Main
    device_manager.run();

    info!("Terminating...");
    // Interrupt threads
    term_now.store(true, Ordering::Release);
    device_manager.context().interrupt_handle_events();
    sigs_handle.close();

    // Wait till the end
    dbus_thd.join().expect("DBus thread panicked");
    events_thd.join().expect("USB thread panicked");
    sigs_thd.join().expect("Signal thread panicked");

    Ok(())
}

fn register_forced_shutdown() -> Result<Arc<AtomicBool>, Box<dyn Error>> {
    // Make sure double CTRL+C and similar kills
    let term_now = Arc::new(AtomicBool::new(false));
    for sig in TERM_SIGNALS {
        signal_hook::flag::register_conditional_shutdown(*sig, 1, Arc::clone(&term_now))?;
        signal_hook::flag::register(*sig, Arc::clone(&term_now))?;
    }
    Ok(term_now)
}
