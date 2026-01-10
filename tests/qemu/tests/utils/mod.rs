use taskette::scheduler::{Scheduler, SchedulerConfig};

#[cfg(feature = "esp32c3")]
esp_bootloader_esp_idf::esp_app_desc!();

#[cfg(feature = "cortex-m")]
pub use taskette_cortex_m::Stack;
#[cfg(feature = "esp32c3")]
pub use taskette_esp_riscv::Stack;

#[cfg(feature = "cortex-m")]
pub use cortex_m_rt::entry;
#[cfg(feature = "esp32c3")]
pub use esp_hal::main as entry;

pub fn init_scheduler(tick_freq: u32) -> Option<Scheduler> {
    #[cfg(feature = "cortex-m")]
    {
        let peripherals = cortex_m::Peripherals::take().unwrap();
        taskette_cortex_m::init_scheduler(
            peripherals.SYST,
            peripherals.SCB,
            168_000_000,
            SchedulerConfig::default().with_tick_freq(tick_freq),
        )
    }
    #[cfg(feature = "esp32c3")]
    {
        let peripherals = esp_hal::init(esp_hal::Config::default());
        let swint = esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
        taskette_esp_riscv::init_scheduler(
            peripherals.SYSTIMER,
            swint.software_interrupt0,
            168_000_000,
            SchedulerConfig::default().with_tick_freq(tick_freq),
        )
    }
}
