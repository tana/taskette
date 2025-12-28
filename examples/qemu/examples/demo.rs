// This file is released in the public domain.

#![no_std]
#![no_main]

use embedded_hal::delay::DelayNs;
use heapless::String;
use log::info;
use panic_semihosting as _;
use static_cell::StaticCell;
use taskette::{scheduler::{SchedulerConfig, spawn}, task::TaskConfig};
use taskette_cortex_m::{Stack, init_scheduler};
use taskette_utils::delay::Delay;

const TICK_FREQ: u32 = 100;

static LOGGER: Logger = Logger;

static TASK1_STACK: StaticCell<Stack<8192>> = StaticCell::new();
static TASK2_STACK: StaticCell<Stack<8192>> = StaticCell::new();

#[cortex_m_rt::entry]
fn main() -> ! {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Debug);

    info!("Started");

    let peripherals = cortex_m::Peripherals::take().unwrap();
    let scheduler = init_scheduler(
        peripherals.SYST,
        peripherals.SCB,
        168_000_000,
        SchedulerConfig::default().with_tick_freq(TICK_FREQ),
    ).unwrap();

    let task1_str = String::<8>::try_from("aaaa").unwrap();
    let task1_stack = TASK1_STACK.init(Stack::new());
    let _task1 = spawn(move || {
        let mut delay = Delay::new().unwrap();
        let mut i = 0;
        loop {
            log::info!("task1 {} {}", i, task1_str);
            i = (i + 1) % 10000;
            delay.delay_ms(1000);
        }
    }, task1_stack, TaskConfig::default()).unwrap();

    let task2_str = String::<8>::try_from("bbbb").unwrap();
    let task2_stack = TASK2_STACK.init(Stack::new());
    let _task2 = spawn(move || {
        let mut delay = Delay::new().unwrap();
        let mut i = 0;
        loop {
            log::info!("task2 {} {}", i, task2_str);
            i = (i + 1) % 10000;
            delay.delay_ms(1000);
        }
    }, task2_stack, TaskConfig::default()).unwrap();

    scheduler.start();
}

struct Logger;

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= log::Level::Trace
    }
    
    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            cortex_m_semihosting::hprintln!("[{}] {}: {}", record.level(), record.target(), record.args())
        }
    }
    
    fn flush(&self) {}
}
