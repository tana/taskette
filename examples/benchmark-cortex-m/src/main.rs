#![no_std]
#![no_main]

mod wrapper;

use defmt::info;
use defmt_rtt as _;
use panic_probe as _;
use static_cell::ConstStaticCell;
use taskette::{arch::yield_now, scheduler::spawn, task::TaskConfig, timer::current_time};
use taskette_cortex_m::Stack;

use crate::wrapper::init_scheduler;

static TASK1_STACK: ConstStaticCell<Stack<8192>> = ConstStaticCell::new(Stack::new());
static TASK2_STACK: ConstStaticCell<Stack<8192>> = ConstStaticCell::new(Stack::new());

const TICK_FREQ: u32 = 1000;

const SWITCH_COUNT: usize = 1000;

#[wrapper::entry]
fn main() -> ! {
    info!("Started");

    let scheduler = init_scheduler(TICK_FREQ);

    let _task1 = spawn(
        task1_func,
        TASK1_STACK.take(),
        TaskConfig::default().with_priority(1),
    )
    .unwrap();

    let _task2 = spawn(
        task2_func,
        TASK2_STACK.take(),
        TaskConfig::default().with_priority(1),
    )
    .unwrap();

    scheduler.start();
}

fn task1_func() {
    loop {
        let start_time = current_time().unwrap();

        for _ in 0..(SWITCH_COUNT / 2) {
            // Switch to `task2` and back => 2 context switches
            yield_now();
        }

        let end_time = current_time().unwrap();
        let time_ms = 1000 * (end_time - start_time) / TICK_FREQ as u64;

        info!("Time diff = {} ms", time_ms);
        info!(
            "Context switch time = {} us",
            1000 * time_ms / SWITCH_COUNT as u64
        );
    }
}

fn task2_func() {
    loop {
        yield_now();
    }
}
