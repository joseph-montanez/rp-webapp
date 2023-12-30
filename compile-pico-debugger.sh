#!/bin/bash

echo "Compiling"
cargo build --target=thumbv6m-none-eabi --release

sudo openocd -f interface/cmsis-dap.cfg -f target/rp2040.cfg -c "adapter speed 5000" -c "program target/thumbv6m-none-eabi/release/pico-webapp verify reset exit"