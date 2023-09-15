#![macro_use]

use core::cell::RefCell;

#[allow(unused)]
pub use defmt::*;
use embassy_embedded_hal::flash::partition::BlockingPartition;
use embassy_nrf::nvmc::Nvmc;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
use {defmt_rtt as _, panic_probe as _};

pub struct TestState {
    pub num_boots: u32,
}

impl TestState {
    pub fn init(flash: &Mutex<NoopRawMutex, RefCell<Nvmc<'_>>>) -> Self {
        extern "C" {
            static __test_state_start: u32;
            static __test_state_end: u32;
        }

        let mut flash = unsafe {
            let start = &__test_state_start as *const u32 as u32;
            let end = &__test_state_end as *const u32 as u32;
            BlockingPartition::new(flash, start, end - start)
        };

        let mut buf = [0; 4];
        let _ = flash.read(0, &mut buf).unwrap();
        if &buf[..] == &[0xff, 0xff, 0xff, 0xff] {
            TestState { num_boots: 0 }
        } else {
            TestState {
                num_boots: u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]),
            }
        }
    }

    pub fn inc(&mut self, flash: &Mutex<NoopRawMutex, RefCell<Nvmc<'_>>>) {
        extern "C" {
            static __test_state_start: u32;
            static __test_state_end: u32;
        }

        let mut flash = unsafe {
            let start = &__test_state_start as *const u32 as u32;
            let end = &__test_state_end as *const u32 as u32;
            BlockingPartition::new(flash, start, end - start)
        };
        self.num_boots += 1;

        flash.erase(0, 4096).unwrap();
        flash.write(0, &self.num_boots.to_be_bytes()).unwrap();
    }
}
