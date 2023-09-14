#![macro_use]

#[allow(unused)]
pub use defmt::*;
use embassy_embedded_hal::flash::partition::asynch::Partition;
use embassy_nrf::nvmc::Nvmc;
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use {defmt_rtt as _, panic_probe as _};

pub struct TestState<'a> {
    flash: Partition<'a, NoopRawMutex, Nvmc<'a>>,
}
