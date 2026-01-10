//! Test of async support (`taskette_utils::futures::block_on`)

#![no_std]
#![no_main]

mod panic_handler;
mod utils;

use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use heapless::Vec;
use semihosting::process::ExitCode;
use static_cell::{ConstStaticCell, StaticCell};
use taskette::{
    scheduler::{Scheduler, spawn},
    task::TaskConfig,
    timer::{current_time, wait_until},
};
use taskette_utils::futures::block_on;

use crate::utils::{Stack, entry, init_scheduler};

static SCHEDULER: StaticCell<Scheduler> = StaticCell::new();
static TASK1_STACK: ConstStaticCell<Stack<8192>> = ConstStaticCell::new(Stack::new());
static TASK2_STACK: ConstStaticCell<Stack<8192>> = ConstStaticCell::new(Stack::new());

static CHANNEL: Channel<CriticalSectionRawMutex, i32, 1> = Channel::new();

#[entry]
fn main() -> ! {
    let scheduler = SCHEDULER.init(init_scheduler(100).unwrap());

    let _task1 = spawn(
        task1,
        TASK1_STACK.take(),
        TaskConfig::default().with_priority(2),
    )
    .unwrap();
    let _task2 = spawn(
        task2,
        TASK2_STACK.take(),
        TaskConfig::default().with_priority(1),
    )
    .unwrap();

    scheduler.start();
}

fn task1() {
    let numbers = block_on(async {
        let mut numbers = Vec::<i32, 16>::new();
        for _ in 0..10 {
            numbers.push(CHANNEL.receive().await).unwrap();
        }

        numbers
    });

    for i in 0..10 {
        if numbers[i] != i as i32 {
            ExitCode::FAILURE.exit_process();
        }
    }

    ExitCode::SUCCESS.exit_process();
}

fn task2() {
    let mut i = 0;

    loop {
        block_on(CHANNEL.send(i));
        wait_until(current_time().unwrap() + 1).unwrap();

        i += 1;
    }
}
