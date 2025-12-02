use core::{cell::RefCell, sync::atomic::Ordering};

use critical_section::Mutex;
use heapless::{BinaryHeap, binary_heap::Min};

use crate::{Error, Futex};

const MAX_TIMER_REGS: usize = 32;

static TIMER: Mutex<RefCell<Option<Timer>>> = Mutex::new(RefCell::new(None));

struct TimerRegistry {
    time: u64,
    futex: Futex,
    set_value: usize,
}

impl Ord for TimerRegistry {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.time.cmp(&other.time)
    }
}

impl PartialOrd for TimerRegistry {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// This is strange, but necessary for consistency of `Ord` and `Eq`.
impl PartialEq for TimerRegistry {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl Eq for TimerRegistry {}

/// Implements a heap based timer, which is a variation of Scheme 3 described in the following paper:
///     G. Varghese and T. Lauck, “Hashed and hierarchical timing wheels: data structures for the efficient implementation of a timer facility,” in Proceedings of the eleventh ACM Symposium on Operating systems principles - SOSP ’87, Austin, Texas, United States, 1987.
struct Timer {
    time: u64,
    queue: BinaryHeap<TimerRegistry, Min, MAX_TIMER_REGS>,
}

pub(crate) fn init() {
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
            if top.time >= timer.time {
                // Timer ringing
                let top = unsafe { timer.queue.pop_unchecked() }; // Safe because the heap is obviously not empty.
                let _ = wake_timeout(&top);
            }
        }
    })
}

/// Registers a one-shot timeout that sets `set_value` to the `futex` and wakes it up after `time` ticks.
pub fn register_timeout(time: u64, futex: Futex, set_value: usize) -> Result<(), Error> {
    let registry = TimerRegistry {
        time,
        futex,
        set_value,
    };

    critical_section::with(|cs| {
        let mut timer = TIMER.borrow_ref_mut(cs);
        let Some(timer) = timer.as_mut() else {
            return Err(Error::NotInitialized);
        };

        if registry.time >= timer.time {
            // The timer is ringing before queueing
            let _ = wake_timeout(&registry);
            return Ok(());
        }

        timer.queue.push(registry).or(Err(Error::TimerFull))?;

        Ok(())
    })
}

pub fn current_time() -> Result<u64, Error> {
    critical_section::with(|cs| {
        let timer = TIMER.borrow_ref(cs);
        let Some(timer) = timer.as_ref() else {
            return Err(Error::NotInitialized);
        };

        Ok(timer.time)
    })
}

fn wake_timeout(registry: &TimerRegistry) -> Result<(), Error> {
    registry
        .futex
        .as_ref()
        .store(registry.set_value, Ordering::Release);
    registry.futex.wake_all()?;

    Ok(())
}
