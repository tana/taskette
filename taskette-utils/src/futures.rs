//! Support for asynchronous (`async`/`await`) code

use core::{
    pin::pin, sync::atomic::Ordering, task::{Context, Poll, RawWaker, RawWakerVTable, Waker}
};

use taskette::
    futex::Futex
;

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
    let futex = Futex::new(0);

    // SAFETY: `futex` will live during the execution of the future (i.e. within this function)
    let waker = unsafe {
        Waker::from_raw(RawWaker::new(
            &futex as *const Futex as *const (),
            &RAW_WAKER_VTABLE,
        ))
    };
    let mut context = Context::from_waker(&waker);

    let mut fut = pin!(fut);

    loop {
        match fut.as_mut().poll(&mut context) {
            Poll::Ready(ret) => break ret,
            Poll::Pending => futex.wait(0).expect("Failed to wait a futex"),
        }

        futex.as_ref().store(0, Ordering::SeqCst);
    }
}

unsafe fn raw_waker_clone(data: *const ()) -> RawWaker {
    RawWaker::new(data, &RAW_WAKER_VTABLE)
}

unsafe fn raw_waker_wake(data: *const ()) {
    let futex = unsafe { &*(data as *const Futex) };
    futex.as_ref().store(1, Ordering::SeqCst);
    futex.wake_all().expect("Failed to wake the waiting task");
}

unsafe fn raw_waker_wake_by_ref(data: *const ()) {
    let futex = unsafe { &*(data as *const Futex) };
    futex.as_ref().store(1, Ordering::SeqCst);
    futex.wake_all().expect("Failed to wake the waiting task");
}

unsafe fn raw_waker_drop(_data: *const ()) {
    // Do nothing
}
