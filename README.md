# Logitech Gaming Devices Daemon

*Systemd daemon to control background LEDs of Logitech gaming devices.*

# Features

* Set different pre-defined color animations or static colors
* Reapply last set configuration after
  * reboot
  * suspend
  * re-plugging (USB hotplugging)

## Supported Devices

* G213 Keyboard
* G203 LIGHTSYNC Mouse

## Installation

Install prerequisites for building:

```bash
# For Debian-based distributions:
sudo apt install libdbus-1-dev libusb-1.0-0-dev pkg-config
```

Build daemon and install Systemd service:
```bash
cargo install gdevd && sudo ~/.cargo/bin/gdevctl install-service
```

It installs two binaries:

* `/usr/local/bin/gdevctl`: Command line utility to speak to the daemon via DBus
* `/usr/local/bin/gdevd`: Daemon that exposes DBus service on system bus (`de.richardliebscher.gdevd`)

## Usage

```bash
gdevctl --help
```