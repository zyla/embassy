#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
teleprobe_meta::target!(b"nrf52840-dk");

#[path = "../common.rs"]
mod common;
use common::*;

use embassy_executor::Spawner;
use embassy_time::{Timer, Duration};
use embassy_nrf::nvmc::Nvmc;
use embassy_sync::blocking_mutex::Mutex;
use core::cell::RefCell;

#[embassy_executor::main]
async fn main(s: Spawner) {
    let p = embassy_nrf::init(Default::default());

    s.spawn(watchdog_task()).unwrap();

    let nvmc = Nvmc::new(p.NVMC);
    let nvmc = Mutex::new(RefCell::new(nvmc));

    let mut state = TestState::init(&nvmc);
    state.inc(&nvmc);

    // Simulate hang
    loop {}
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
