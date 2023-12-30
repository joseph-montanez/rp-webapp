#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(async_fn_in_trait)]
#![allow(stable_features, unknown_lints, async_fn_in_trait)]

use embedded_sdmmc::SdCardError;
use core::fmt::Debug;
use crate::http::{BUFFER_SIZE, ByteString, get_query_param_value, MAX_HEADER_KEY, MAX_HEADER_VALUE, Request, Response};
use core::str::from_utf8;
use cyw43_pio::PioSpi;
use defmt::*;
use embassy_executor::Spawner;
use embassy_net::tcp::TcpSocket;
use embassy_net::{Config, Stack, StackResources};
use embassy_rp::bind_interrupts;
use embassy_rp::gpio::{Level, Output, Pin};
use embassy_rp::peripherals::{DMA_CH0, PIN_23, PIN_25, PIO0, SPI1};
use embassy_rp::pio::{InterruptHandler, Pio};
use embassy_rp::spi::{Spi, Config as Spi_Config};
use embedded_sdmmc::{Directory, Error as Sdmmc_Error, File, SdCard, TimeSource, Timestamp, Volume, VolumeIdx, VolumeManager};
use embassy_time::{Duration, Timer};
use embedded_io_async::Write;
use static_cell::make_static;
use crate::kv::{KeyValueStore, Serializable};
use crate::user::User;
use {defmt_rtt as _, panic_probe as _};
use crate::routes::sign_up::post::route_sign_up_post;
use embedded_hal::blocking::delay::DelayUs;
use core::fmt::Write as CoreWrite;
use critical_section::CriticalSection;
use critical_section::with;
use crate::base64::base64_url_encode;
use crate::jwt::generate_keys;
use crate::sdcard::{CALLBACK, Delayer, MyTimeSource, read_file_async, ReadCallback, SDCARD_MANAGER, SdCardManager, SdCardError as SdError, read_file, list_directory, FileInfo};
use crate::template::replace;


mod http;
mod kv;
mod user;
mod template;
mod routes;
mod sdcard;
mod jwt;
mod base64;

bind_interrupts!(struct Irqs {
    PIO0_IRQ_0 => InterruptHandler<PIO0>;
});

const WIFI_NETWORK: &str = "WIFI_SSID_HERE";
const WIFI_PASSWORD: &str = "WIFI_PASSWORD_HERE";


#[embassy_executor::task]
async fn wifi_task(
    runner: cyw43::Runner<'static, Output<'static, PIN_23>, PioSpi<'static, PIN_25, PIO0, 0, DMA_CH0>>,
) -> ! {
    runner.run().await
}

// #[embassy_executor::task]
// async fn delayer(duration: u32) {
//     Timer::after(Duration::from_millis(duration as u64)).await;
// }

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<cyw43::NetDriver<'static>>) -> ! {
    stack.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let fw = include_bytes!("../cyw43-firmware/43439A0.bin");
    let clm = include_bytes!("../cyw43-firmware/43439A0_clm.bin");

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let mut pio = Pio::new(p.PIO0, Irqs);
    let spi = PioSpi::new(&mut pio.common, pio.sm0, pio.irq0, cs, p.PIN_24, p.PIN_29, p.DMA_CH0);

    // SPI and Chip Select Pin setup for SD Card
    // let dma_tx = embassy_rp::dma::Channel::number(); // Claim a DMA channel
    // let dma_rx = embassy_rp::dma::write(); // Claim another DMA channel

    let sdmmc_cs = Output::new(p.PIN_9, Level::High); // CS pin

    let sdmmc_spi = Spi::new(
        p.SPI1,           // SPI peripheral
        p.PIN_10,           // SCK pin
        p.PIN_11,          // MOSI pin
        p.PIN_8,          // MISO pin
        p.DMA_CH1,       // Transmit DMA channel
        p.DMA_CH2,       // Receive DMA channel
        Spi_Config::default(), // Default SPI configuration
    );

    let delayer = Delayer;
    let time_source = MyTimeSource;


    let mut sdmmc_err = ByteString::<1024>::new(&[]);

    let sdcard = SdCard::new(sdmmc_spi, sdmmc_cs, delayer);


    match sdcard.num_bytes() {
        Ok(num_of_bytes) => {
            core::writeln!(sdmmc_err, "Card size {} bytes", num_of_bytes).unwrap();
        }
        Err(e) => {
            core::write!(sdmmc_err, "{:?}\n", e).unwrap();
        }
    }

    match sdcard.get_card_type() {
        None => {
            core::writeln!(sdmmc_err, "Unable to get card type").unwrap();
        }
        Some(card_type) => {
            core::writeln!(sdmmc_err, "Card type {:?}", card_type).unwrap();
        }
    }

    // println!("Card size {} bytes", ?);
    let mut volume_mgr = VolumeManager::new(sdcard, time_source);

    //
    // with(|cs| {
    //     let mut manager_ref = SDCARD_MANAGER.borrow(cs);
    //     manager_ref.replace(Some(SdCardManager { volume_mgr }));
    // });


    info!("creating cyw43...");
    let state = make_static!(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;
    unwrap!(spawner.spawn(wifi_task(runner)));

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    let config = Config::dhcpv4(Default::default());

    // Generate random seed
    let seed = 0x0123_4567_89ab_cdef; // chosen by fair dice roll. guarenteed to be random.

    // Init network stack
    let stack = &*make_static!(Stack::new(
        net_device,
        config,
        make_static!(StackResources::<2>::new()),
        seed
    ));

    unwrap!(spawner.spawn(net_task(stack)));


    let mut id_store = KeyValueStore::<u16, u16>::new();
    let mut user_store = KeyValueStore::<u16, User>::new();

    info!("joining network...");
    loop {
        //control.join_open(WIFI_NETWORK).await;
        match control.join_wpa2(WIFI_NETWORK, WIFI_PASSWORD).await {
            Ok(_) => break,
            Err(err) => {
                info!("join failed with status={}", err.status);
            }
        }
    }

    // Wait for DHCP, not necessary when using static IP
    info!("waiting for DHCP...");
    while !stack.is_config_up() {
        Timer::after_millis(100).await;
    }
    info!("DHCP is now up!");

    let mut rx_buffer = [0; 4096];
    let mut tx_buffer = [0; 4096];
    let mut buf = [0; 4096];

    control.gpio_set(0, false).await;
    let mut socket = TcpSocket::new(stack, &mut rx_buffer, &mut tx_buffer);
    socket.set_timeout(Some(Duration::from_secs(10)));

    info!("Listening on TCP:8000...");
    control.gpio_set(0, true).await;

    loop {
        if let Err(e) = socket.accept(8000).await {
            warn!("accept error: {:?}", e);
            continue;
        }

        info!("Received connection from {:?}", socket.remote_endpoint());

        loop {
            let n = match socket.read(&mut buf).await {
                Ok(0) => {
                    warn!("read EOF");
                    break;
                }
                Ok(n) => n,
                Err(e) => {
                    warn!("read error: {:?}", e);
                    break;
                }
            };

            info!("rxd {}", from_utf8(&buf[..n]).unwrap());

            let mut req = Request::new();
            let mut resp = Response::<MAX_HEADER_KEY, MAX_HEADER_VALUE>::new();

            req.parse(&buf[..n], n);

            let mut control_action = None;

            let (http_response, response_length) = match req.path.as_bytes() {
                b"/" => {
                    resp.status = 200;

                    resp.headers.append(ByteString::new(b"Content-Type"), Some(ByteString::new(b"text/html")));
                    resp.headers.append(ByteString::new(b"Connection"), Some(ByteString::new(b"close")));

                    resp.write(b"<html><head><link href=\"/style.css\" rel=\"stylesheet\" /></head><body><h1>Hello /</h1><p>");

                    // Add the Accept-Encoding value if it exists
                    for (_, header_option) in req.headers.data.iter().enumerate() {
                        if let (Some(key), value_count, Some(value)) = header_option {
                            // Append the header key for debugging
                            resp.write(key.as_bytes());
                            resp.write(b": ");

                            // for i in 0..*value_count {
                            //     if let Some(value) = values[i] {
                            //         // Append the header value for debugging
                            resp.write(value.as_bytes());
                            resp.write(b", ");
                            //     }
                            // }
                            resp.write(b"<br>");
                        }
                    }

                    // Complete the HTML response
                    resp.write(b"</p></body></html>");

                    resp.generate()
                }
                b"/jwt/generate" => {
                    resp.status = 200;
                    resp.headers.append(ByteString::new(b"Content-Type"), Some(ByteString::new(b"text/html")));
                    resp.headers.append(ByteString::new(b"Connection"), Some(ByteString::new(b"close")));

                    let (private_key, public_key) = generate_keys();
                    let mut public_key_encoded = [0u8; 256];
                    let mut private_key_encoded = [0u8; 128];

                    let public_key_encoded_length = base64_url_encode(&public_key, &mut public_key_encoded);
                    let private_key_encoded_length = base64_url_encode(&private_key, &mut private_key_encoded);

                    resp.write(b"Public Key: ");
                    resp.write(&public_key_encoded[..public_key_encoded_length]);
                    resp.write(b"<br>");
                    resp.write(b"Private Key: ");
                    resp.write(&private_key_encoded[..private_key_encoded_length]);

                    resp.generate()
                }
                b"/sd-card/list" => {
                    resp.status = 200;
                    resp.headers.append(ByteString::new(b"Content-Type"), Some(ByteString::new(b"text/html")));
                    resp.headers.append(ByteString::new(b"Connection"), Some(ByteString::new(b"close")));

                    match list_directory(&mut volume_mgr, "/") {
                        Ok((files, total_files)) => {
                            for (_, file) in files.iter().enumerate() {
                                let filename = &file.name[..file.name_len];
                                resp.write(b"<a target=\"_new\" href=\"/sd-card/edit?filename=");
                                resp.write(filename);
                                resp.write(b"\">");
                                resp.write(filename);
                                resp.write(b"</a><br>")
                            }
                        }
                        Err(e) => {
                            resp.write(b"Cannot read directory");
                        }
                    }

                    resp.generate()
                }
                b"/sd-card/edit" => {
                    resp.status = 200;
                    resp.headers.append(ByteString::new(b"Content-Type"), Some(ByteString::new(b"text/html")));
                    resp.headers.append(ByteString::new(b"Connection"), Some(ByteString::new(b"close")));

                    let mut path_buffer = [0u8; 128];  // Adjust the size as needed
                    let query_filename = get_query_param_value(
                        req.query_param_count,
                        &req.query_param_keys,
                        &req.query_param_values,
                        b"filename",
                    );

                    if let Some(filename) = query_filename {

                        if filename.len() <= path_buffer.len() {
                            path_buffer[..filename.len()].copy_from_slice(filename);

                            if let Ok(file_path_str) = core::str::from_utf8(&path_buffer[..filename.len()]) {
                                let mut file_data = ByteString::<{ 1024 * 16 }>::new(b"");
                                let _ = read_file(&mut volume_mgr, file_path_str, &mut file_data);

                                // ... rest of your code ...
                                resp.write(&file_path_str.as_bytes());

                                let mut file_data = ByteString::<{ 1024 * 16 }>::new(b"");
                                let file_path = core::str::from_utf8(&file_path_str.as_bytes()).unwrap_or("");

                                let _ = read_file(&mut volume_mgr, file_path, &mut file_data);

                                let tpl = r#"
                                    <form action="/sd-card/save" method="POST">
                                        <input type="hidden" name="filename" value="{{filename}}" />
                                        <textarea name="data">{{data}}</textarea>
                                        <br>
                                        <input type="submit" value="Save File">
                                    </form>
                                    "#;

                                let mut tpl_bytes: [u8; 1024] = [0u8; 1024];
                                tpl_bytes.copy_from_slice(tpl.as_bytes());

                                replace(&mut tpl_bytes, "{{filename}}", file_path);
                                replace(&mut tpl_bytes, "{{data}}", core::str::from_utf8(&file_data.as_bytes()).unwrap_or(""));

                                resp.write(&tpl_bytes);
                            }
                        } else {}
                    }
                    resp.generate()
                }
                b"/sd-card" => {
                    resp.status = 200;
                    resp.headers.append(ByteString::new(b"Content-Type"), Some(ByteString::new(b"text/html")));
                    resp.headers.append(ByteString::new(b"Connection"), Some(ByteString::new(b"close")));

                    let _ = read_file(&mut volume_mgr, "my_file.txt", &mut resp.body);

                    resp.generate()
                }
                b"/query" => {
                    resp.status = 200;
                    let mut response_data = ByteString::<BUFFER_SIZE>::new(&[]);

                    response_data.append(b"<html><head><link href=\"/style.css\" rel=\"stylesheet\" /></head><body><h1>Hello /</h1><p>Hello");

                    // Add name of the person
                    let name = get_query_param_value(req.query_param_count, &req.query_param_keys, &req.query_param_values, b"name");
                    if let Some(name_value) = name {
                        resp.write(name_value);
                    }

                    // Complete the HTML response
                    resp.write(b"!</p></body></html>");

                    resp.headers.append(ByteString::new(b"Content-Type"), Some(ByteString::new(b"text/html")));
                    resp.headers.append(ByteString::new(b"Connection"), Some(ByteString::new(b"close")));

                    resp.generate()
                }
                b"/off" => {
                    resp.status = 200;
                    control_action = Some((0, false));

                    resp.headers.append(ByteString::new(b"Content-Type"), Some(ByteString::new(b"text/html")));
                    resp.headers.append(ByteString::new(b"Connection"), Some(ByteString::new(b"close")));


                    resp.generate()
                }
                b"/on" => {
                    resp.status = 200;
                    control_action = Some((0, true));

                    resp.headers.append(ByteString::new(b"Content-Type"), Some(ByteString::new(b"text/html")));
                    resp.headers.append(ByteString::new(b"Connection"), Some(ByteString::new(b"close")));


                    resp.generate()
                }
                b"/style.css" => {
                    resp.status = 200;

                    resp.headers.append(ByteString::new(b"Content-Type"), Some(ByteString::new(b"text/css")));
                    resp.headers.append(ByteString::new(b"Connection"), Some(ByteString::new(b"close")));

                    resp.write(b"body{color: #333; font-family: sans-serif;'}h1{}p{}");


                    resp.generate()
                }
                b"/sign-up" => {
                    if req.method.as_bytes() == b"POST" {
                        route_sign_up_post(&req, &mut resp, &mut id_store, &mut user_store)
                    } else {
                        handle_get_sign_up_route(&req, &mut resp)
                    }
                }
                _ => {
                    resp.status = 400;

                    resp.headers.append(ByteString::new(b"Content-Type"), Some(ByteString::new(b"text/html")));
                    resp.headers.append(ByteString::new(b"Connection"), Some(ByteString::new(b"close")));

                    resp.generate()
                }
            };

            if let Some((pin, state)) = control_action {
                control.gpio_set(pin, state).await;
            }

            match socket.write_all(&http_response[..response_length]).await {
                Ok(()) => {
                    socket.close();
                }
                Err(e) => {
                    warn!("write error: {:?}", e);
                    break;
                }
            };
        }
    }
}

fn handle_get_sign_up_route(
    req: &Request,
    resp: &mut Response<MAX_HEADER_KEY, MAX_HEADER_VALUE>,
)
    -> ([u8; BUFFER_SIZE], usize) {
    resp.status = 200;

    resp.headers.append(ByteString::new(b"Content-Type"), Some(ByteString::new(b"text/html")));
    resp.headers.append(ByteString::new(b"Connection"), Some(ByteString::new(b"close")));
    let tpl = include_str!("templates/sign-up.html");
    resp.write(tpl.as_bytes());

    resp.generate()
}

