//! Time management, sleeping, and other timer functions.
//!
//! Time is represented as the number of ticks since the start of the scheduler.
//! Implements a heap based timer, which is a variation of Scheme 3 described in the following paper:
//!     G. Varghese and T. Lauck, “Hashed and hierarchical timing wheels: data structures for the efficient implementation of a timer facility,” in Proceedings of the eleventh ACM Symposium on Operating systems principles - SOSP ’87, Austin, Texas, United States, 1987.

use core::{cell::RefCell, ptr::addr_of_mut, sync::atomic::Ordering};

use critical_section::Mutex;
use heapless::{
    BinaryHeap,
    binary_heap::Min,
    pool::arc::{Arc, ArcBlock},
};

use crate::{
    Error,
    scheduler::{block_task, current_task_id, unblock_task},
};

const MAX_TIMER_REGS: usize = 32;

static TIMER: Mutex<RefCell<Option<Timer>>> = Mutex::new(RefCell::new(None));

/// Because `arc_pool` creates a public struct, it is hidden in a private module
mod pool {
    use heapless::arc_pool;
    use portable_atomic::AtomicBool;

    arc_pool!(TimerDataPool: TimerData);

    pub struct TimerData {
        pub time: u64,
        pub func: fn(usize),
        pub arg: usize,
        pub canceled: AtomicBool,
    }
}

use pool::{TimerData, TimerDataPool};

static mut TIMER_DATA_BLOCKS: [ArcBlock<TimerData>; MAX_TIMER_REGS] =
    [const { ArcBlock::new() }; MAX_TIMER_REGS];

impl Ord for TimerData {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.time.cmp(&other.time)
    }
}

impl PartialOrd for TimerData {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// This is strange, but necessary for consistency of `Ord` and `Eq`.
impl PartialEq for TimerData {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl Eq for TimerData {}

/// Opaque handle for a timer registration.
pub struct TimerRegistration {
    inner: Arc<TimerDataPool>,
}

impl TimerRegistration {
    pub fn cancel(self) {
        self.inner.canceled.store(true, Ordering::SeqCst);
    }
}

struct Timer {
    time: u64,
    queue: BinaryHeap<Arc<TimerDataPool>, Min, MAX_TIMER_REGS>,
}

pub(crate) fn init() {
    // Supply memory for the object pool
    unsafe {
        for block in addr_of_mut!(TIMER_DATA_BLOCKS).as_mut().unwrap() {
            TimerDataPool.manage(block);
        }
    }

    critical_section::with(|cs| {
        TIMER.replace(
            cs,
            Some(Timer {
                time: 0,
                queue: BinaryHeap::new(),
            }),
        )
    });
}

pub(crate) fn tick() {
    critical_section::with(|cs| {
        let mut timer = TIMER.borrow_ref_mut(cs);
        let Some(timer) = timer.as_mut() else {
            return;
        };

        timer.time += 1;

        if let Some(top) = timer.queue.peek() {
            if top.time <= timer.time {
                // Timer ringing
                let top = unsafe { timer.queue.pop_unchecked() }; // Safe because the heap is obviously not empty.

                if !top.canceled.load(Ordering::SeqCst) {
                    // Execute handler function
                    (top.func)(top.arg)
                }
            }
        }
    })
}

pub(crate) fn register_timer(
    time: u64,
    func: fn(usize),
    arg: usize,
) -> Result<TimerRegistration, Error> {
    let Ok(registry) = TimerDataPool.alloc(TimerData {
        time,
        func,
        arg,
        canceled: false.into(),
    }) else {
        return Err(Error::TimerFull);
    };

    critical_section::with(|cs| {
        let mut timer = TIMER.borrow_ref_mut(cs);
        let Some(timer) = timer.as_mut() else {
            return Err(Error::NotInitialized);
        };

        if registry.time <= timer.time {
            return Err(Error::TimerPast);
        }

        timer
            .queue
            .push(Arc::clone(&registry))
            .or(Err(Error::TimerFull))?;

        Ok(TimerRegistration { inner: registry })
    })
}

/// Registers a one-shot timeout that wakes the specified task up on `time`.
pub(crate) fn wait_task_until(time: u64, task_id: usize) -> Result<(), Error> {
    let func = |arg| {
        let _ = unblock_task(arg as usize);
    };

    let _reg = register_timer(time, func, task_id as usize)?;
    block_task(task_id)?;

    Ok(())
}

/// Blocks the current task until the specificed time.
pub fn wait_until(time: u64) -> Result<(), Error> {
    wait_task_until(time, current_task_id()?)
}

/// Retrieves current time (in ticks).
pub fn current_time() -> Result<u64, Error> {
    critical_section::with(|cs| {
        let timer = TIMER.borrow_ref(cs);
        let Some(timer) = timer.as_ref() else {
            return Err(Error::NotInitialized);
        };

        Ok(timer.time)
    })
}
