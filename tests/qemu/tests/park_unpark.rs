//! Test of task park/unpark

#![no_std]
#![no_main]

mod panic_handler;
mod utils;

use core::cell::RefCell;

use critical_section::Mutex;
use heapless::Vec;
use semihosting::{print, println, process::ExitCode};
use static_cell::StaticCell;
use taskette::{
    scheduler::{Scheduler, spawn},
    task::{self, TaskConfig, TaskHandle},
};

use crate::utils::{Stack, entry, init_scheduler};

static SCHEDULER: StaticCell<Scheduler> = StaticCell::new();
static TASK_LOW_STACK: StaticCell<Stack<8192>> = StaticCell::new();
static TASK_HIGH_STACK: StaticCell<Stack<8192>> = StaticCell::new();

static TASK_HIGH_HANDLE: Mutex<RefCell<Option<TaskHandle>>> = Mutex::new(RefCell::new(None));

static NUMBERS: Mutex<RefCell<Vec<i32, 2000>>> = Mutex::new(RefCell::new(Vec::new()));

#[entry]
fn main() -> ! {
    let scheduler = SCHEDULER.init(init_scheduler(100).unwrap());

    // Stacks are allocated here because `StaticCell::init`` temporarily place the value on stack and may cause overflow
    let task_low_stack = TASK_LOW_STACK.init(Stack::new());
    let task_high_stack = TASK_HIGH_STACK.init(Stack::new());

    let _task_low = spawn(
        || task_low(task_high_stack),
        task_low_stack,
        TaskConfig::default().with_priority(1),
    )
    .unwrap();

    scheduler.start();
}

fn task_low(task_high_stack: &mut Stack<8192>) {
    // Launch a high-priority task (but it blocks first)
    let _task_high = spawn(
        task_high,
        task_high_stack,
        TaskConfig::default().with_priority(2),
    )
    .unwrap();

    // This runs the first despite `task_high` has higher priority
    for i in 0..1000 {
        put_number(i);
    }

    // When `task_high` blocks and `task_low` restarts, `TASK_HIGH_HANDLE` is set to the handle of `task_high`.
    let task_high_handle =
        critical_section::with(|cs| TASK_HIGH_HANDLE.borrow_ref(cs).as_ref().unwrap().clone());

    // Unblock `task_high`
    task_high_handle.unpark().unwrap();

    // Check result
    critical_section::with(|cs| {
        let numbers = NUMBERS.borrow_ref(cs);
        if numbers.iter().cloned().eq(0..2000) {
            ExitCode::SUCCESS.exit_process();
        } else {
            // If the low priority task is not preempted correctly, the numbers will be incorrectly ordered
            for num in numbers.iter() {
                print!("{} ", num);
            }
            println!("");
            ExitCode::FAILURE.exit_process();
        }
    });
}

fn task_high() {
    critical_section::with(|cs| {
        TASK_HIGH_HANDLE.replace(cs, Some(task::current().unwrap()));
    });

    // Wait until unparked by `task_low`
    task::park().unwrap();

    for i in 1000..2000 {
        put_number(i);
    }
}

fn put_number(num: i32) {
    critical_section::with(|cs| {
        let mut numbers = NUMBERS.borrow_ref_mut(cs);
        numbers.push(num).unwrap();
    });
}
