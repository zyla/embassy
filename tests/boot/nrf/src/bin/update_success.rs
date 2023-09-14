#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
teleprobe_meta::target!(b"nrf52840-dk");

#[path = "../common.rs"]
mod common;
use common::*;

use embassy_boot_nrf::{FirmwareUpdater, FirmwareUpdaterConfig};
use embassy_embedded_hal::adapter::BlockingAsync;
use embassy_executor::Spawner;
use embassy_nrf::gpio::{Input, Level, Output, OutputDrive, Pull};
use embassy_nrf::nvmc::Nvmc;
use embassy_nrf::wdt::{self, Watchdog};
use embassy_sync::mutex::Mutex;
use panic_reset as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_nrf::init(Default::default());

    let wdt_config = wdt::Config::try_new(&p.WDT).unwrap();
    let (_wdt, [_wdt_handle]) = match Watchdog::try_new(p.WDT, wdt_config) {
        Ok(x) => x,
        Err(_) => {
            // Watchdog already active with the wrong number of handles, waiting for it to timeout...
            loop {
                cortex_m::asm::wfe();
            }
        }
    };

    let nvmc = Nvmc::new(p.NVMC);
    let nvmc = Mutex::new(BlockingAsync::new(nvmc));

    let test_state = Partition::new(&mut )
    let config = FirmwareUpdaterConfig::from_linkerfile(&nvmc);
    let mut magic = [0; 4];
    let mut updater = FirmwareUpdater::new(config, &mut magic);

    // Check state if we've attempted update yet
    let dfu = updater.prepare().await.unwrap();

    let mut offset = 0;
    for chunk in APP_B.chunks(4096) {
        let mut buf: [u8; 4096] = [0; 4096];
        buf[..chunk.len()].copy_from_slice(chunk);
        updater.write_firmware(offset, &buf).await.unwrap();
        offset += chunk.len();
    }
    updater.mark_updated().await.unwrap();
    led.set_high();
    cortex_m::peripheral::SCB::sys_reset();
}
