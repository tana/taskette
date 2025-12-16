//! `embedded-hal`-compatible delay that yields CPU to other tasks instead of busy looping.
//! The precision is limited by the tick frequency setting of the scheduler (usually order of a millisecond or more).
use taskette::{Error, scheduler::get_config, timer::{current_time, wait_until}};

#[derive(Clone)]
pub struct Delay {
    tick_freq: u32,
}

impl Delay {
    pub fn new() -> Result<Self, Error> {
        let tick_freq = get_config()?.tick_freq;

        Ok(Self { tick_freq })
    }

    pub fn delay_ticks(&mut self, ticks: u64) {
        let now = current_time().expect("Failed to acquire current time");
        wait_until(now + ticks).expect("Failed to register timeout");
    }
}

impl embedded_hal::delay::DelayNs for Delay {
    fn delay_ns(&mut self, ns: u32) {
        self.delay_ticks(((ns * self.tick_freq) as u64).div_ceil(1_000_000_000));
    }

    fn delay_us(&mut self, us: u32) {
        self.delay_ticks(((us * self.tick_freq) as u64).div_ceil(1_000_000));
    }

    fn delay_ms(&mut self, ms: u32) {
        self.delay_ticks(((ms * self.tick_freq) as u64).div_ceil(1_000));
    }
}
