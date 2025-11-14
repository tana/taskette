#![no_std]
#![no_main]

use log::info;
use panic_semihosting as _;
use static_cell::StaticCell;
use taskette::{Kernel, KernelConfig, Stack};

static TASK1_STACK: StaticCell<Stack<8192>> = StaticCell::new();
static TASK2_STACK: StaticCell<Stack<8192>> = StaticCell::new();

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
        peripherals.SCB,
        12_000_000,
        KernelConfig::default().with_tick_freq(10),
    );

    let task1_stack = TASK1_STACK.init(Stack::new());
    let _task1 = kernel.spawn(task1_stack, || {
        let mut i = 0;
        loop {
            log::info!("task1 {}", i);
            i = (i + 1) % 10000;
        }
    }).unwrap();

    let task2_stack = TASK2_STACK.init(Stack::new());
    let _task2 = kernel.spawn(task2_stack, || {
        let mut i = 0;
        loop {
            log::info!("task2 {}", i);
            i = (i + 1) % 10000;
        }
    }).unwrap();

    kernel.start();
}
