#!/bin/bash

cargo build --target=thumbv6m-none-eabi --release

# Convert to
rm -f /usr/src/myapp/target/thumbv6m-none-eabi/release/pico-webapp-uf2
/usr/src/pico-sdk/tools/elf2uf2/elf2uf2 /usr/src/myapp/target/thumbv6m-none-eabi/release/pico-webapp /usr/src/myapp/target/thumbv6m-none-eabi/release/pico-webapp-uf2
echo "Done!"