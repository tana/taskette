//! Test of stack overflow detection by canary

#![no_std]
#![no_main]

mod utils;

use core::{fmt::Write, panic::PanicInfo};

use heapless::String;
use semihosting::{println, process::ExitCode};
use static_cell::{ConstStaticCell, StaticCell};
use taskette::{
    arch::yield_now,
    scheduler::{Scheduler, spawn},
    task::TaskConfig,
};

use crate::utils::{Stack, entry, init_scheduler};

static SCHEDULER: StaticCell<Scheduler> = StaticCell::new();
static TASK1_STACK: ConstStaticCell<Stack<8192>> = ConstStaticCell::new(Stack::new());

#[panic_handler]
fn panic_handler(info: &PanicInfo<'_>) -> ! {
    let mut message = String::<128>::new();
    if write!(&mut message, "{}", info.message()).is_ok() {
        if message.starts_with("Stack overflow detected") {
            ExitCode::SUCCESS.exit_process();
        }
    }

    println!("{:?}", info);
    ExitCode::FAILURE.exit_process();
}

#[entry]
fn main() -> ! {
    let scheduler = SCHEDULER.init(init_scheduler(100).unwrap());

    let _task1 = spawn(task1, TASK1_STACK.take(), TaskConfig::default()).unwrap();

    scheduler.start();
}

fn task1() {
    crash();
    ExitCode::FAILURE.exit_process();
}

#[allow(unconditional_recursion)]
fn crash() {
    let _big = [0u8; 128];
    yield_now();    // Not to destroy critical data before being detected
    crash();
}
