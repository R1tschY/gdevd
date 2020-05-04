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
        sector: u8,
    },
    /*    Breathe {
        color: String,
        speed: u16,
    },
    Cycle {
        speed: u16,
    },*/
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
        Cli::Color { color, sector } => {
            devices.method_call(
                "de.richardliebscher.g213d.GDeviceManager",
                "color_sector",
                (&color as &str, sector),
            )?;
        }
        Cli::Refresh => {
            devices.method_call("de.richardliebscher.g213d.GDeviceManager", "refresh", ())?;
        }
    }

    Ok(())
}
