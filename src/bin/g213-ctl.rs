use std::mem::MaybeUninit;
use std::time::Duration;
use std::{io, slice};

use structopt::StructOpt;

use g213d::g213::G213Model;
use g213d::Command::{Breathe, ColorSector, Cycle};
use g213d::{GDeviceManager, RgbColor};
use rusb::Result;
use std::error::Error;

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
    Breathe {
        color: String,
        speed: u16,
    },
    Cycle {
        speed: u16,
    },
}

fn main() -> std::result::Result<(), Box<dyn Error>> {
    simple_logger::init()?;

    let args = Cli::from_args();

    let mut manager = GDeviceManager::try_new()?;
    match args {
        Cli::Color { color, sector } => {
            let rgb = hex::decode(color).expect("invalid hex color");
            if rgb.len() != 3 {
                panic!("invalid hex color");
            }

            manager.send_command(&ColorSector(
                RgbColor(
                    *rgb.get(0).unwrap(),
                    *rgb.get(1).unwrap(),
                    *rgb.get(2).unwrap(),
                ),
                Some(sector),
            ));
        }
        _ => unimplemented!(),
    }

    Ok(())
}
