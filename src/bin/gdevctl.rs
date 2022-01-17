use structopt::StructOpt;

use dbus::blocking::Connection;
use std::error::Error;
use std::time::Duration;

#[derive(StructOpt)]
#[structopt(
    about = "Change background lights of Logitech gaming devices",
    rename_all = "kebab"
)]
enum Cli {
    /// Set color for keyboard sector
    Color {
        /// Hex string for color
        color: String,
        /// sector index
        sector: Option<u8>,
    },
    /// Apply breathe effect
    Breathe {
        /// Hex string for color
        color: String,
        /// animation time step in milliseconds
        /// (minimum value depends on device, default value depends on device)
        time_step: u16,
        /// brightness (must be greater or equal than 0 and less or equal than 100; default is 100)
        brightness: u8,
    },
    /// Apply cycle effect
    Cycle {
        /// animation time step in milliseconds
        /// (minimum value depends on device, default value depends on device)
        time_step: u16,
        /// brightness (must be greater or equal than 0 and less or equal than 100; default is 100)
        brightness: u8,
    },
    /// Apply wave effect
    Wave {
        /// direction of effect (left-to-right, right-to-left, center-to-edge, edge-to-center;
        ///   default is left-to-right)
        direction: String,
        /// animation time step in milliseconds
        /// (minimum value depends on device, default value depends on device)
        time_step: u16,
        /// brightness (must be greater or equal than 0 and less or equal than 100; default is 100)
        brightness: u8,
    },
    /// Reapply saved effect
    Refresh,
    /// List drivers
    ListDrivers,
    /// List devices
    List,
}

fn main() -> std::result::Result<(), Box<dyn Error>> {
    simple_logger::init()?;

    // DBus
    let conn = Connection::new_system()?;
    let devices = conn.with_proxy(
        "de.richardliebscher.gdevd",
        "/devices",
        Duration::from_millis(5000),
    );

    match Cli::from_args() {
        Cli::Color {
            color,
            sector: Some(sector),
        } => {
            devices.method_call(
                "de.richardliebscher.gdevd.GDeviceManager",
                "color_sector",
                (&color as &str, sector),
            )?;
        }
        Cli::Color { color, sector: _ } => {
            devices.method_call(
                "de.richardliebscher.gdevd.GDeviceManager",
                "color_sectors",
                (&color as &str,),
            )?;
        }
        Cli::Breathe {
            color,
            time_step,
            brightness,
        } => {
            devices.method_call(
                "de.richardliebscher.gdevd.GDeviceManager",
                "breathe",
                (color, time_step, brightness),
            )?;
        }
        Cli::Cycle {
            time_step,
            brightness,
        } => {
            devices.method_call(
                "de.richardliebscher.gdevd.GDeviceManager",
                "cycle",
                (time_step, brightness),
            )?;
        }
        Cli::Wave {
            direction,
            time_step,
            brightness,
        } => {
            devices.method_call(
                "de.richardliebscher.gdevd.GDeviceManager",
                "wave",
                (&direction as &str, time_step, brightness),
            )?;
        }
        Cli::Refresh => {
            devices.method_call("de.richardliebscher.gdevd.GDeviceManager", "refresh", ())?;
        }
        Cli::ListDrivers => {
            let drivers: (Vec<(String,)>,) = devices.method_call(
                "de.richardliebscher.gdevd.GDeviceManager",
                "list_drivers",
                (),
            )?;
            for driver in drivers.0 {
                println!("{}", driver.0);
            }
        }
        Cli::List => {
            let devices: (Vec<(String, String)>,) =
                devices.method_call("de.richardliebscher.gdevd.GDeviceManager", "list", ())?;
            for device in devices.0 {
                println!("{}: {}", device.0, device.1);
            }
        }
    }

    Ok(())
}
