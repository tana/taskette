use taskette::scheduler::Scheduler;

#[cfg(feature = "rp2040")]
pub use rp2040_hal::entry;

#[cfg(feature = "rp235x")]
pub use rp235x_hal::entry;

// This is necessary when directly using HAL without BSP
// Reference: https://github.com/rp-rs/rp-hal/blob/50a77826533f759b331076712d151e93650cc2bc/rp2040-hal-examples/src/bin/blinky.rs#L27-L33
#[cfg(feature = "rp2040")]
#[unsafe(link_section = ".boot2")]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_GENERIC_03H;

#[cfg(feature = "rp235x")]
#[unsafe(link_section = ".start_block")]
#[used]
pub static IMAGE_DEF: rp235x_hal::block::ImageDef = rp235x_hal::block::ImageDef::secure_exe();

#[cfg(feature = "rp2040")]
pub fn init_scheduler(tick_freq: u32) -> Scheduler {
    use rp2040_hal::Clock as _;
    use taskette::scheduler::SchedulerConfig;

    const XTAL_FREQ: u32 = 12_000_000;

    let mut peripherals = rp2040_hal::pac::Peripherals::take().unwrap();

    // Init RP2040 system
    let mut watchdog = rp2040_hal::Watchdog::new(peripherals.WATCHDOG);
    let clocks = rp2040_hal::clocks::init_clocks_and_plls(
        XTAL_FREQ,
        peripherals.XOSC,
        peripherals.CLOCKS,
        peripherals.PLL_SYS,
        peripherals.PLL_USB,
        &mut peripherals.RESETS,
        &mut watchdog,
    )
    .unwrap();

    // Init scheduler
    let core_peripherals = cortex_m::Peripherals::take().unwrap();
    taskette_cortex_m::init_scheduler(
        core_peripherals.SYST,
        core_peripherals.SCB,
        clocks.system_clock.freq().to_Hz(),
        SchedulerConfig::default().with_tick_freq(tick_freq),
    )
    .unwrap()
}

#[cfg(feature = "rp235x")]
pub fn init_scheduler(tick_freq: u32) -> Scheduler {
    use rp235x_hal::Clock as _;
    use taskette::scheduler::SchedulerConfig;

    const XTAL_FREQ: u32 = 12_000_000;

    let mut peripherals = rp235x_hal::pac::Peripherals::take().unwrap();

    // Init RP235x system
    let mut watchdog = rp235x_hal::Watchdog::new(peripherals.WATCHDOG);
    let clocks = rp235x_hal::clocks::init_clocks_and_plls(
        XTAL_FREQ,
        peripherals.XOSC,
        peripherals.CLOCKS,
        peripherals.PLL_SYS,
        peripherals.PLL_USB,
        &mut peripherals.RESETS,
        &mut watchdog,
    )
    .unwrap();

    // Init scheduler
    let core_peripherals = cortex_m::Peripherals::take().unwrap();
    taskette_cortex_m::init_scheduler(
        core_peripherals.SYST,
        core_peripherals.SCB,
        clocks.system_clock.freq().to_Hz(),
        SchedulerConfig::default().with_tick_freq(tick_freq),
    )
    .unwrap()
}
