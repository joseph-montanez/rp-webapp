# Start from the official Rust image
FROM rust:latest

# Install the ARMv7 cross-compiler
RUN apt-get update && apt-get install -y gcc-arm-linux-gnueabihf pkg-config libssl-dev
RUN apt install -y libusb-1.0-0-dev libudev-dev
RUN apt install -y --fix-missing cmake gcc-arm-none-eabi libnewlib-arm-none-eabi libstdc++-arm-none-eabi-newlib

# Debugging support for Pico
#RUN cargo install probe-run
#RUN mkdir -p /etc/udev/rules.d
#RUN COPY 69-probe-rs.rules /etc/udev/rules.d/69-probe-rs.rules
#RUN udevadm control --reload
#RUN udevadm trigger
#RUN usermod -aG plugdev $USER

# Download and compile Pico SDK
RUN git clone https://github.com/raspberrypi/pico-sdk.git /usr/src/pico-sdk  \
    && cd /usr/src/pico-sdk  \
    && git submodule update --init \
    && cmake .  \
    && make -j8 \
    && cd /usr/src/pico-sdk/tools/elf2uf2 \
    && cmake . \
    && make -j8

# Download embassy, no crates available
RUN git clone https://github.com/embassy-rs/embassy.git /usr/src/embassy-main

RUN cargo install flip-link

# Change to nigtly
RUN rustup default nightly

# Add the ARMv7 target
RUN rustup target add thumbv6m-none-eabi

# Set the working directory inside the container
WORKDIR /usr/src/myapp

# Copy your source code into the container
COPY . .

CMD ["tail", "-f", "/dev/null"]

# Build your application
#RUN cargo build --target=thumbv6m-none-eabi --release

#RUN /usr/src/pico-sdk/tools/elf2uf2/elf2uf2 /usr/src/myapp/target/thumbv6m-none-eabi/release/pico-webapp /media/joseph/RPI-RP2/pico-webapp