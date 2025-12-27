//! Test of stack overflow detection by canary

#![no_std]
#![no_main]

use core::{fmt::Write, panic::PanicInfo};

use cortex_m_semihosting::{
    debug::{self, EXIT_FAILURE, EXIT_SUCCESS},
    hprintln,
};
use heapless::String;
use static_cell::{ConstStaticCell, StaticCell};
use taskette::{
    arch::yield_now,
    scheduler::{Scheduler, SchedulerConfig, spawn},
    task::TaskConfig,
};
use taskette_cortex_m::{Stack, init_scheduler};

static SCHEDULER: StaticCell<Scheduler> = StaticCell::new();
static TASK1_STACK: ConstStaticCell<Stack<8192>> = ConstStaticCell::new(Stack::new());

#[panic_handler]
fn panic_handler(info: &PanicInfo<'_>) -> ! {
    let mut message = String::<128>::new();
    if write!(&mut message, "{}", info.message()).is_ok() {
        if message.starts_with("Stack overflow detected") {
            debug::exit(EXIT_SUCCESS);
        }
    }

    hprintln!("{:?}", info);
    debug::exit(EXIT_FAILURE);

    loop {}
}

#[cortex_m_rt::entry]
fn main() -> ! {
    let peripherals = cortex_m::Peripherals::take().unwrap();
    let scheduler = SCHEDULER.init(
        init_scheduler(
            peripherals.SYST,
            peripherals.SCB,
            168_000_000,
            SchedulerConfig::default().with_tick_freq(100),
        )
        .unwrap(),
    );

    let _task1 = spawn(task1, TASK1_STACK.take(), TaskConfig::default()).unwrap();

    scheduler.start();
}

fn task1() {
    crash();
    debug::exit(EXIT_FAILURE);
}

#[allow(unconditional_recursion)]
fn crash() {
    let _big = [0u8; 128];
    yield_now();    // Not to destroy critical data before being detected
    crash();
}
