# Logitech Gaming Devices Daemon

*Systemd daemon to control background LEDs of Logitech gaming devices.*

## Installation

Install as Systemd service:

    cargo install gdevd && sudo gdevctl install-service

It installs two binaries:

* `gdevctl`: Command line utility to speak to the daemon via DBus
* `gdevd`: Daemon that exposes DBus service on system bus (`de.richardliebscher.gdevd`)

## Usage

    gdevctl --help

## Supported Devices

* G213
* G203 LIGHTSYNC