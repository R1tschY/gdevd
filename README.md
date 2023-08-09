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

Install as Systemd service:

    cargo install gdevd && sudo ~/.cargo/bin/gdevctl install-service

It installs two binaries:

* `gdevctl`: Command line utility to speak to the daemon via DBus
* `gdevd`: Daemon that exposes DBus service on system bus (`de.richardliebscher.gdevd`)

## Usage

    gdevctl --help
