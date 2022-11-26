# Logitech Gaming Devices Daemon

*Systemd daemon to control background LEDs of Logitech gaming devices.*

## Installation
Clone the repository. Open a terminal in the cloned directory.

Install Rust toolchain: 
   
    sh ./rustup-init.sh

Install gdevd:

    make && sudo make install

You can later uninstall with:
    
    sudo make uninstall

## Usage

    gdevctl --help

## Supported Devices

* G213
* G203 LIGHTSYNC
