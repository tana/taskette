//! Test of async support (`taskette_utils::futures::block_on`)

#![no_std]
#![no_main]

use cortex_m_semihosting::debug::{self, EXIT_FAILURE, EXIT_SUCCESS};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel};
use heapless::Vec;
use panic_semihosting as _;
use static_cell::{ConstStaticCell, StaticCell};
use taskette::{
    scheduler::{Scheduler, SchedulerConfig, spawn},
    task::TaskConfig,
    timer::{current_time, wait_until},
};
use taskette_cortex_m::{Stack, init_scheduler};
use taskette_utils::futures::block_on;

static SCHEDULER: StaticCell<Scheduler> = StaticCell::new();
static TASK1_STACK: ConstStaticCell<Stack<8192>> = ConstStaticCell::new(Stack::new());
static TASK2_STACK: ConstStaticCell<Stack<8192>> = ConstStaticCell::new(Stack::new());

static CHANNEL: Channel<CriticalSectionRawMutex, i32, 1> = Channel::new();

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
            debug::exit(EXIT_FAILURE);
        }
    }

    debug::exit(EXIT_SUCCESS);
}

fn task2() {
    let mut i = 0;

    loop {
        block_on(CHANNEL.send(i));
        wait_until(current_time().unwrap() + 1).unwrap();

        i += 1;
    }
}
