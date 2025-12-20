//! Support for asynchronous (`async`/`await`) code

use core::{
    pin::pin,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use taskette::task::{self, TaskHandle};

const RAW_WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(
    raw_waker_clone,
    raw_waker_wake,
    raw_waker_wake_by_ref,
    raw_waker_drop,
);

/// Executes a `Future` and blocks the current task until it completes.
///
/// It yields CPU to other tasks while blocking and does not involve busy loop.
pub fn block_on<F: Future>(fut: F) -> F::Output {
    let current_task = task::current().expect("Failed to get the current task");

    // SAFETY: `current_task` will live during the execution of the future (i.e. within this function)
    let waker = unsafe {
        Waker::from_raw(RawWaker::new(
            &current_task as *const TaskHandle as *const (),
            &RAW_WAKER_VTABLE,
        ))
    };
    let mut context = Context::from_waker(&waker);

    let mut fut = pin!(fut);

    loop {
        match fut.as_mut().poll(&mut context) {
            Poll::Ready(ret) => break ret,
            Poll::Pending => task::park().expect("Failed to park the task"),
        }
    }
}

unsafe fn raw_waker_clone(data: *const ()) -> RawWaker {
    RawWaker::new(data, &RAW_WAKER_VTABLE)
}

unsafe fn raw_waker_wake(data: *const ()) {
    let task_handle = unsafe { &*(data as *const TaskHandle) };
    task_handle.unpark().expect("Failed to unpark the task");
}

unsafe fn raw_waker_wake_by_ref(data: *const ()) {
    let task_handle = unsafe { &*(data as *const TaskHandle) };
    task_handle.unpark().expect("Failed to unpark the task");
}

unsafe fn raw_waker_drop(_data: *const ()) {
    // Do nothing
}
