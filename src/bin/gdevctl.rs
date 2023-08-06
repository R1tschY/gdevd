use std::error::Error;
use std::fs::Permissions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use std::{fmt, fs, io};

use dbus::blocking::Connection;
use structopt::StructOpt;

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
    /// Install daemon as systemd service
    InstallService {
        /// Prefix for service installation
        #[structopt(long, parse(from_os_str), default_value = "/usr/local")]
        prefix: PathBuf,
    },
    /// Uninstall daemon as systemd service
    UninstallService {
        /// Prefix of service installation
        #[structopt(long, parse(from_os_str), default_value = "/usr/local")]
        prefix: PathBuf,
    },
}

fn main() {
    match _main() {
        Ok(_) => {}
        Err(err) => {
            eprintln!("ERROR: {err}")
        }
    }
}

fn _main() -> Result<(), Box<dyn Error>> {
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
        Cli::InstallService { prefix } => install_service(&prefix)?,
        Cli::UninstallService { prefix } => uninstall_service(&prefix)?,
    }

    Ok(())
}

static SERVICE_FILES: &[(&str, &str)] = &[
    (
        "/etc/dbus-1/system.d/gdevd-dbus.conf",
        include_str!("../systemd/gdevd-dbus.conf"),
    ),
    (
        "/etc/systemd/system/gdevd.service",
        include_str!("../systemd/gdevd.service.in"),
    ),
    (
        "/etc/systemd/system/gdevrefresh.service",
        include_str!("../systemd/gdevrefresh.service.in"),
    ),
];

fn paths() -> Result<(PathBuf, PathBuf), io::Error> {
    let path = std::env::current_exe()?;
    let root = path.parent().unwrap();
    Ok((root.join("gdevd"), path))
}

fn install_service(prefix: &Path) -> Result<(), io::Error> {
    let (daemon, ctrl) = paths()?;

    copy_file(&daemon, &prefix.join("bin/gdevd"))?;
    copy_file(&ctrl, &prefix.join("bin/gdevctl"))?;

    let prefix_str = prefix
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Other, "invalid prefix path"))?;

    for (path, content) in SERVICE_FILES {
        install_file(path, content.replace("$$PREFIX$$", prefix_str).as_bytes())?;
    }

    progress(format_args!("Restart service"), || {
        run_command(Command::new("systemctl").arg("daemon-reload"))?;
        run_command(
            Command::new("systemctl")
                .arg("reload-or-restart")
                .arg("gdevd"),
        )
    })?;

    Ok(())
}

fn copy_file(src: &Path, dest: &Path) -> Result<(), io::Error> {
    progress(format_args!("Installing {}", dest.display()), || {
        fs::copy(src, dest)?;
        set_permissions(dest)?;
        Ok(())
    })
}

fn install_file(path: &str, content: &[u8]) -> Result<(), io::Error> {
    progress(format_args!("Installing {path}"), || {
        fs::write(path, content)?;
        set_permissions(path)?;
        Ok(())
    })
}

#[cfg(unix)]
fn set_permissions(path: impl AsRef<Path>) -> Result<(), io::Error> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn set_permissions(path: &str) -> Result<(), io::Error> {
    Ok(())
}

fn uninstall_service(prefix: &Path) -> Result<(), io::Error> {
    progress(format_args!("Stop service"), || {
        run_command(Command::new("systemctl").arg("stop").arg("gdevd"))
    })?;

    uninstall_file(&prefix.join("bin/gdevd"))?;
    uninstall_file(&prefix.join("bin/gdevctl"))?;

    for (path, _) in SERVICE_FILES {
        uninstall_file(path)?;
    }

    Ok(())
}

fn uninstall_file(path: impl AsRef<Path>) -> Result<(), io::Error> {
    let path = path.as_ref();
    progress(
        format_args!("Uninstalling {}", path.display()),
        || match fs::remove_file(path) {
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
            res => res,
        },
    )
}

fn run_command(cmd: &mut Command) -> io::Result<()> {
    let out = cmd.output()?;
    if !out.status.success() {
        Err(io::Error::new(
            io::ErrorKind::Other,
            String::from_utf8_lossy(&out.stderr),
        ))
    } else {
        Ok(())
    }
}

fn progress(op: fmt::Arguments<'_>, f: impl Fn() -> io::Result<()>) -> io::Result<()> {
    let mut stderr = io::stderr();
    let _ = stderr.write_fmt(op);
    let _ = stderr.write_all(b" ... ");
    let _ = stderr.flush();

    let res = f();
    match &res {
        Ok(_) => eprintln!("OK"),
        Err(err) => eprintln!("ERROR: {err}"),
    };
    let _ = stderr.flush();

    res
}
