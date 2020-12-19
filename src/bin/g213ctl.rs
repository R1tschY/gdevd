use structopt::StructOpt;

use dbus::blocking::Connection;
use std::error::Error;
use std::time::Duration;

#[derive(StructOpt)]
#[structopt(
    about = "Change color of Logitech G213 keyboards",
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
        /// speed inverse (must be greater than 31; default is 1000)
        speed: u16,
    },
    /// Apply cycle effect
    Cycle {
        /// speed inverse (must be greater than 31; default is 1000)
        speed: u16,
    },
    /// Apply wave effect
    Wave {
        /// direction of effect (left-to-right, right-to-left, center-to-edge, edge-to-center;
        ///   default is left-to-right)
        direction: String,
        /// speed inverse (must be greater than 31, default is 1000)
        speed: u16,
    },
    /// Reapply saved effect
    Refresh,
}

fn main() -> std::result::Result<(), Box<dyn Error>> {
    simple_logger::init()?;

    // DBus
    let conn = Connection::new_system()?;
    let devices = conn.with_proxy(
        "de.richardliebscher.g213d",
        "/devices",
        Duration::from_millis(5000),
    );

    match Cli::from_args() {
        Cli::Color {
            color,
            sector: Some(sector),
        } => {
            devices.method_call(
                "de.richardliebscher.g213d.GDeviceManager",
                "color_sector",
                (&color as &str, sector),
            )?;
        }
        Cli::Color { color, sector: _ } => {
            devices.method_call(
                "de.richardliebscher.g213d.GDeviceManager",
                "color_sectors",
                (&color as &str,),
            )?;
        }
        Cli::Breathe { color, speed } => {
            devices.method_call(
                "de.richardliebscher.g213d.GDeviceManager",
                "breathe",
                (color, speed),
            )?;
        }
        Cli::Cycle { speed } => {
            devices.method_call(
                "de.richardliebscher.g213d.GDeviceManager",
                "cycle",
                (speed,),
            )?;
        }
        Cli::Wave { direction, speed } => {
            devices.method_call(
                "de.richardliebscher.g213d.GDeviceManager",
                "wave",
                (&direction as &str, speed),
            )?;
        }
        Cli::Refresh => {
            devices.method_call("de.richardliebscher.g213d.GDeviceManager", "refresh", ())?;
        }
    }

    Ok(())
}
