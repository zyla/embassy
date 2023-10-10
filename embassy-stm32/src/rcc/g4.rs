use stm32_metapac::flash::vals::Latency;
use stm32_metapac::rcc::vals::{Adcsel, Pllsrc, Sw};
use stm32_metapac::FLASH;

pub use crate::pac::rcc::vals::{
    Hpre as AHBPrescaler, Pllm as PllM, Plln as PllN, Pllp as PllP, Pllq as PllQ, Pllr as PllR, Ppre as APBPrescaler,
};
use crate::pac::{PWR, RCC};
use crate::rcc::sealed::RccPeripheral;
use crate::rcc::{set_freqs, Clocks};
use crate::time::Hertz;

/// HSI speed
pub const HSI_FREQ: Hertz = Hertz(16_000_000);

/// LSI speed
pub const LSI_FREQ: Hertz = Hertz(32_000);

#[derive(Clone, Copy)]
pub enum AdcClockSource {
    NoClk,
    SysClk,
    PllP,
}

impl AdcClockSource {
    pub fn adcsel(&self) -> Adcsel {
        match self {
            AdcClockSource::NoClk => Adcsel::NOCLK,
            AdcClockSource::SysClk => Adcsel::SYSCLK,
            AdcClockSource::PllP => Adcsel::PLLP,
        }
    }
}

impl Default for AdcClockSource {
    fn default() -> Self {
        Self::NoClk
    }
}

/// System clock mux source
#[derive(Clone, Copy)]
pub enum ClockSrc {
    HSE(Hertz),
    HSI16,
    PLL,
}

/// PLL clock input source
#[derive(Clone, Copy, Debug)]
pub enum PllSrc {
    HSI16,
    HSE(Hertz),
}

impl Into<Pllsrc> for PllSrc {
    fn into(self) -> Pllsrc {
        match self {
            PllSrc::HSE(..) => Pllsrc::HSE,
            PllSrc::HSI16 => Pllsrc::HSI16,
        }
    }
}

/// PLL Configuration
///
/// Use this struct to configure the PLL source, input frequency, multiplication factor, and output
/// dividers. Be sure to keep check the datasheet for your specific part for the appropriate
/// frequency ranges for each of these settings.
pub struct Pll {
    /// PLL Source clock selection.
    pub source: PllSrc,

    /// PLL pre-divider
    pub prediv_m: PllM,

    /// PLL multiplication factor for VCO
    pub mul_n: PllN,

    /// PLL division factor for P clock (ADC Clock)
    pub div_p: Option<PllP>,

    /// PLL division factor for Q clock (USB, I2S23, SAI1, FDCAN, QSPI)
    pub div_q: Option<PllQ>,

    /// PLL division factor for R clock (SYSCLK)
    pub div_r: Option<PllR>,
}

fn ahb_div(ahb: AHBPrescaler) -> u32 {
    match ahb {
        AHBPrescaler::DIV1 => 1,
        AHBPrescaler::DIV2 => 2,
        AHBPrescaler::DIV4 => 4,
        AHBPrescaler::DIV8 => 8,
        AHBPrescaler::DIV16 => 16,
        AHBPrescaler::DIV64 => 64,
        AHBPrescaler::DIV128 => 128,
        AHBPrescaler::DIV256 => 256,
        AHBPrescaler::DIV512 => 512,
        _ => unreachable!(),
    }
}

fn apb_div(apb: APBPrescaler) -> u32 {
    match apb {
        APBPrescaler::DIV1 => 1,
        APBPrescaler::DIV2 => 2,
        APBPrescaler::DIV4 => 4,
        APBPrescaler::DIV8 => 8,
        APBPrescaler::DIV16 => 16,
        _ => unreachable!(),
    }
}

/// Sets the source for the 48MHz clock to the USB and RNG peripherals.
pub enum Clock48MhzSrc {
    /// Use the High Speed Internal Oscillator. For USB usage, the CRS must be used to calibrate the
    /// oscillator to comply with the USB specification for oscillator tolerance.
    Hsi48(Option<CrsConfig>),
    /// Use the PLLQ output. The PLL must be configured to output a 48MHz clock. For USB usage the
    /// PLL needs to be using the HSE source to comply with the USB specification for oscillator
    /// tolerance.
    PllQ,
}

/// Sets the sync source for the Clock Recovery System (CRS).
pub enum CrsSyncSource {
    /// Use an external GPIO to sync the CRS.
    Gpio,
    /// Use the Low Speed External oscillator to sync the CRS.
    Lse,
    /// Use the USB SOF to sync the CRS.
    Usb,
}

/// Clocks configutation
pub struct Config {
    pub mux: ClockSrc,
    pub ahb_pre: AHBPrescaler,
    pub apb1_pre: APBPrescaler,
    pub apb2_pre: APBPrescaler,
    pub low_power_run: bool,
    /// Iff PLL is requested as the main clock source in the `mux` field then the PLL configuration
    /// MUST turn on the PLLR output.
    pub pll: Option<Pll>,
    /// Sets the clock source for the 48MHz clock used by the USB and RNG peripherals.
    pub clock_48mhz_src: Option<Clock48MhzSrc>,
    pub adc12_clock_source: AdcClockSource,
    pub adc345_clock_source: AdcClockSource,
}

/// Configuration for the Clock Recovery System (CRS) used to trim the HSI48 oscillator.
pub struct CrsConfig {
    /// Sync source for the CRS.
    pub sync_src: CrsSyncSource,
}

impl Default for Config {
    #[inline]
    fn default() -> Config {
        Config {
            mux: ClockSrc::HSI16,
            ahb_pre: AHBPrescaler::DIV1,
            apb1_pre: APBPrescaler::DIV1,
            apb2_pre: APBPrescaler::DIV1,
            low_power_run: false,
            pll: None,
            clock_48mhz_src: None,
            adc12_clock_source: Default::default(),
            adc345_clock_source: Default::default(),
        }
    }
}

pub struct PllFreq {
    pub pll_p: Option<Hertz>,
    pub pll_q: Option<Hertz>,
    pub pll_r: Option<Hertz>,
}

pub(crate) unsafe fn init(config: Config) {
    let pll_freq = config.pll.map(|pll_config| {
        let src_freq = match pll_config.source {
            PllSrc::HSI16 => {
                RCC.cr().write(|w| w.set_hsion(true));
                while !RCC.cr().read().hsirdy() {}

                HSI_FREQ
            }
            PllSrc::HSE(freq) => {
                RCC.cr().write(|w| w.set_hseon(true));
                while !RCC.cr().read().hserdy() {}
                freq
            }
        };

        // Disable PLL before configuration
        RCC.cr().modify(|w| w.set_pllon(false));
        while RCC.cr().read().pllrdy() {}

        let internal_freq = src_freq / pll_config.prediv_m * pll_config.mul_n;

        RCC.pllcfgr().write(|w| {
            w.set_plln(pll_config.mul_n.into());
            w.set_pllm(pll_config.prediv_m.into());
            w.set_pllsrc(pll_config.source.into());
        });

        let pll_p_freq = pll_config.div_p.map(|div_p| {
            RCC.pllcfgr().modify(|w| {
                w.set_pllp(div_p);
                w.set_pllpen(true);
            });
            internal_freq / div_p
        });

        let pll_q_freq = pll_config.div_q.map(|div_q| {
            RCC.pllcfgr().modify(|w| {
                w.set_pllq(div_q);
                w.set_pllqen(true);
            });
            internal_freq / div_q
        });

        let pll_r_freq = pll_config.div_r.map(|div_r| {
            RCC.pllcfgr().modify(|w| {
                w.set_pllr(div_r);
                w.set_pllren(true);
            });
            internal_freq / div_r
        });

        // Enable the PLL
        RCC.cr().modify(|w| w.set_pllon(true));
        while !RCC.cr().read().pllrdy() {}

        PllFreq {
            pll_p: pll_p_freq,
            pll_q: pll_q_freq,
            pll_r: pll_r_freq,
        }
    });

    let (sys_clk, sw) = match config.mux {
        ClockSrc::HSI16 => {
            // Enable HSI16
            RCC.cr().write(|w| w.set_hsion(true));
            while !RCC.cr().read().hsirdy() {}

            (HSI_FREQ.0, Sw::HSI16)
        }
        ClockSrc::HSE(freq) => {
            // Enable HSE
            RCC.cr().write(|w| w.set_hseon(true));
            while !RCC.cr().read().hserdy() {}

            (freq.0, Sw::HSE)
        }
        ClockSrc::PLL => {
            assert!(pll_freq.is_some());
            assert!(pll_freq.as_ref().unwrap().pll_r.is_some());

            let freq = pll_freq.as_ref().unwrap().pll_r.unwrap().0;

            assert!(freq <= 170_000_000);

            if freq >= 150_000_000 {
                // Enable Core Boost mode on freq >= 150Mhz ([RM0440] p234)
                PWR.cr5().modify(|w| w.set_r1mode(false));
                // Set flash wait state in boost mode based on frequency ([RM0440] p191)
                if freq <= 36_000_000 {
                    FLASH.acr().modify(|w| w.set_latency(Latency::WS0));
                } else if freq <= 68_000_000 {
                    FLASH.acr().modify(|w| w.set_latency(Latency::WS1));
                } else if freq <= 102_000_000 {
                    FLASH.acr().modify(|w| w.set_latency(Latency::WS2));
                } else if freq <= 136_000_000 {
                    FLASH.acr().modify(|w| w.set_latency(Latency::WS3));
                } else {
                    FLASH.acr().modify(|w| w.set_latency(Latency::WS4));
                }
            } else {
                PWR.cr5().modify(|w| w.set_r1mode(true));
                // Set flash wait state in normal mode based on frequency ([RM0440] p191)
                if freq <= 30_000_000 {
                    FLASH.acr().modify(|w| w.set_latency(Latency::WS0));
                } else if freq <= 60_000_000 {
                    FLASH.acr().modify(|w| w.set_latency(Latency::WS1));
                } else if freq <= 80_000_000 {
                    FLASH.acr().modify(|w| w.set_latency(Latency::WS2));
                } else if freq <= 120_000_000 {
                    FLASH.acr().modify(|w| w.set_latency(Latency::WS3));
                } else {
                    FLASH.acr().modify(|w| w.set_latency(Latency::WS4));
                }
            }

            (freq, Sw::PLLRCLK)
        }
    };

    RCC.cfgr().modify(|w| {
        w.set_sw(sw);
        w.set_hpre(config.ahb_pre.into());
        w.set_ppre1(config.apb1_pre.into());
        w.set_ppre2(config.apb2_pre.into());
    });

    let ahb_freq: u32 = match config.ahb_pre {
        AHBPrescaler::DIV1 => sys_clk,
        pre => sys_clk / ahb_div(pre),
    };

    let (apb1_freq, apb1_tim_freq) = match config.apb1_pre {
        APBPrescaler::DIV1 => (ahb_freq, ahb_freq),
        pre => {
            let freq = ahb_freq / apb_div(pre);
            (freq, freq * 2)
        }
    };

    let (apb2_freq, apb2_tim_freq) = match config.apb2_pre {
        APBPrescaler::DIV1 => (ahb_freq, ahb_freq),
        pre => {
            let freq = ahb_freq / apb_div(pre);
            (freq, freq * 2)
        }
    };

    // Setup the 48 MHz clock if needed
    if let Some(clock_48mhz_src) = config.clock_48mhz_src {
        let source = match clock_48mhz_src {
            Clock48MhzSrc::PllQ => {
                // Make sure the PLLQ is enabled and running at 48Mhz
                let pllq_freq = pll_freq.as_ref().and_then(|f| f.pll_q);
                assert!(pllq_freq.is_some() && pllq_freq.unwrap().0 == 48_000_000);

                crate::pac::rcc::vals::Clk48sel::PLLQCLK
            }
            Clock48MhzSrc::Hsi48(crs_config) => {
                // Enable HSI48
                RCC.crrcr().modify(|w| w.set_hsi48on(true));
                // Wait for HSI48 to turn on
                while RCC.crrcr().read().hsi48rdy() == false {}

                // Enable and setup CRS if needed
                if let Some(crs_config) = crs_config {
                    crate::peripherals::CRS::enable();

                    let sync_src = match crs_config.sync_src {
                        CrsSyncSource::Gpio => crate::pac::crs::vals::Syncsrc::GPIO,
                        CrsSyncSource::Lse => crate::pac::crs::vals::Syncsrc::LSE,
                        CrsSyncSource::Usb => crate::pac::crs::vals::Syncsrc::USB,
                    };

                    crate::pac::CRS.cfgr().modify(|w| {
                        w.set_syncsrc(sync_src);
                    });

                    // These are the correct settings for standard USB operation. If other settings
                    // are needed there will need to be additional config options for the CRS.
                    crate::pac::CRS.cr().modify(|w| {
                        w.set_autotrimen(true);
                        w.set_cen(true);
                    });
                }
                crate::pac::rcc::vals::Clk48sel::HSI48
            }
        };

        RCC.ccipr().modify(|w| w.set_clk48sel(source));
    }

    RCC.ccipr()
        .modify(|w| w.set_adc12sel(config.adc12_clock_source.adcsel()));
    RCC.ccipr()
        .modify(|w| w.set_adc345sel(config.adc345_clock_source.adcsel()));

    let adc12_ck = match config.adc12_clock_source {
        AdcClockSource::NoClk => None,
        AdcClockSource::PllP => match &pll_freq {
            Some(pll) => pll.pll_p,
            None => None,
        },
        AdcClockSource::SysClk => Some(Hertz(sys_clk)),
    };

    let adc345_ck = match config.adc345_clock_source {
        AdcClockSource::NoClk => None,
        AdcClockSource::PllP => match &pll_freq {
            Some(pll) => pll.pll_p,
            None => None,
        },
        AdcClockSource::SysClk => Some(Hertz(sys_clk)),
    };

    if config.low_power_run {
        assert!(sys_clk <= 2_000_000);
        PWR.cr1().modify(|w| w.set_lpr(true));
    }

    set_freqs(Clocks {
        sys: Hertz(sys_clk),
        ahb1: Hertz(ahb_freq),
        ahb2: Hertz(ahb_freq),
        apb1: Hertz(apb1_freq),
        apb1_tim: Hertz(apb1_tim_freq),
        apb2: Hertz(apb2_freq),
        apb2_tim: Hertz(apb2_tim_freq),
        adc: adc12_ck,
        adc34: adc345_ck,
    });
}
