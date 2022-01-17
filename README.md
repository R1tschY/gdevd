# Logitech Gaming Devices Daemon

*Systemd daemon to control background LEDs of Logitech gaming devices.*

## Installation

Install Rust toolchain: https://www.rust-lang.org/tools/install

Install gdevd:

    make && sudo make install

You can later uninstall with:
    
    sudo make uninstall

## Usage

    gdevctl --help

## Supported Devices

* G213
* G203 LIGHTSYNC