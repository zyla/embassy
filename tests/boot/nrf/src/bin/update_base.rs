#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
teleprobe_meta::target!(b"nrf52840-dk");

#[path = "../common.rs"]
mod common;
use common::*;

use embassy_boot_nrf::{BlockingFirmwareUpdater, FirmwareUpdaterConfig};
use embassy_executor::Spawner;
use embassy_nrf::nvmc::Nvmc;
use embassy_time::{Timer, Duration};
use embassy_sync::blocking_mutex::Mutex;
use core::cell::RefCell;
use embedded_storage::nor_flash::NorFlash;

static APP_SUCCESS: &[u8] = include_bytes!("../../update_success.bin");
static APP_FAILURE: &[u8] = include_bytes!("../../update_failure.bin");

#[embassy_executor::main]
async fn main(s: Spawner) {
    let p = embassy_nrf::init(Default::default());

    s.spawn(watchdog_task()).unwrap();

    let nvmc = Nvmc::new(p.NVMC);
    let nvmc = Mutex::new(RefCell::new(nvmc));

    let state = TestState::init(&nvmc);

    // First time we boot the bad firmare
    let app = if state.num_boots == 0 {
        APP_FAILURE
    // Second time we boot the good firmware
    } else if state.num_boots == 1 {
        APP_SUCCESS
    } else {
        defmt::panic!("Unexpected number of boots");
    };

    let config = FirmwareUpdaterConfig::from_linkerfile_blocking(&nvmc);
    let mut magic = [0; 4];
    let mut updater = BlockingFirmwareUpdater::new(config, &mut magic);

    // Check state if we've attempted update yet
    let dfu = updater.prepare_update().unwrap();

    let mut offset = 0;
    for chunk in app.chunks(4096) {
        let mut buf: [u8; 4096] = [0; 4096];
        buf[..chunk.len()].copy_from_slice(chunk);
        dfu.write(offset, &buf).unwrap();
        offset += chunk.len() as u32;
    }
    updater.mark_updated().unwrap();
    cortex_m::peripheral::SCB::sys_reset();
}

// Keeps our system alive
#[embassy_executor::task]
async fn watchdog_task() {
    let mut handle = unsafe { embassy_nrf::wdt::WatchdogHandle::steal(0) };
    loop {
        handle.pet();
        Timer::after(Duration::from_secs(2)).await;
    }
}
