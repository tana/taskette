#![no_std]
#![no_main]

use log::info;
use panic_semihosting as _;
use taskette::{Kernel, KernelConfig};

#[cortex_m_rt::entry]
fn main() -> ! {
    // Configure logger to use semihosting
    goolog::init_logger(
        Some(log::Level::Trace),
        None,
        &|_ts, target, level, args| {
            cortex_m_semihosting::hprintln!("[{}] {}: {}", level, target, args);
        },
    )
    .unwrap();

    info!("Started");

    let peripherals = cortex_m::Peripherals::take().unwrap();
    let mut kernel = Kernel::new(
        peripherals.SYST,
        12_000_000,
        KernelConfig::default().with_tick_freq(1),
    );
    kernel.start();

    loop {}
}
