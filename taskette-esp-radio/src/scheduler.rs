use core::{cell::RefCell, ffi::c_void};

use alloc::{boxed::Box, collections::BTreeMap};

use critical_section::Mutex;
use taskette::{
    arch::yield_now,
    scheduler::{MAX_PRIORITY, get_config, spawn},
    task::{self, TaskConfig, TaskHandle},
    timer::{current_time, wait_until},
};

use crate::dynamic_stack::DynamicStack;

extern crate alloc;

static TASK_HANDLE_POOL: Mutex<RefCell<BTreeMap<TaskHandle, Box<TaskHandle>>>> =
    Mutex::new(RefCell::new(BTreeMap::new()));

struct SchedulerImpl {}

impl esp_radio_rtos_driver::Scheduler for SchedulerImpl {
    fn initialized(&self) -> bool {
        get_config().is_ok()
    }

    fn yield_task(&self) {
        yield_now();
    }

    fn yield_task_from_isr(&self) {
        yield_now();
    }

    fn max_task_priority(&self) -> u32 {
        MAX_PRIORITY as u32
    }

    fn task_create(
        &self,
        _name: &str,
        task: extern "C" fn(*mut core::ffi::c_void),
        param: *mut core::ffi::c_void,
        priority: u32,
        _core_id: Option<u32>,
        task_stack_size: usize,
    ) -> *mut core::ffi::c_void {
        let stack = DynamicStack::new(task_stack_size);
        let config = TaskConfig::default().with_priority(priority as usize);
        let param = param as usize;
        let handle = spawn(move || task(param as *mut core::ffi::c_void), stack, config)
            .expect("Task creation failed");
        let mut boxed_handle = Box::new(handle);
        let handle_ptr = boxed_handle.as_mut() as *mut TaskHandle as *mut core::ffi::c_void;

        critical_section::with(|cs| {
            let mut pool = TASK_HANDLE_POOL.borrow_ref_mut(cs);
            pool.insert(handle, boxed_handle);
        });

        handle_ptr
    }

    fn current_task(&self) -> *mut core::ffi::c_void {
        let handle = task::current().expect("Cannot retrieve current task");

        critical_section::with(|cs| {
            let mut pool = TASK_HANDLE_POOL.borrow_ref_mut(cs);
            if let Some(boxed_handle) = pool.get_mut(&handle) {
                boxed_handle.as_mut() as *mut TaskHandle as *mut core::ffi::c_void
            } else {
                let mut boxed_handle = Box::new(handle);
                let handle_ptr = boxed_handle.as_mut() as *mut TaskHandle as *mut core::ffi::c_void;
                pool.insert(handle, boxed_handle);
                handle_ptr
            }
        })
    }

    fn schedule_task_deletion(&self, _task_handle: *mut core::ffi::c_void) {
        unimplemented!()
    }

    fn current_task_thread_semaphore(&self) -> esp_radio_rtos_driver::semaphore::SemaphorePtr {
        unimplemented!()
    }

    fn usleep(&self, us: u32) {
        let now = current_time().expect("Unable to retrieve current time");
        let tick_freq = get_config().expect("Cannot retrieve config").tick_freq;
        let delay = ((us * tick_freq) as u64).div_ceil(1_000_000);
        wait_until(now + delay).expect("Wait failed")
    }

    fn now(&self) -> u64 {
        let tick_freq = get_config().expect("Cannot retrieve config").tick_freq;
        let tick = current_time().expect("Unable to retrieve current time");
        1_000_000 * tick / tick_freq as u64
    }
}

esp_radio_rtos_driver::scheduler_impl!(static SCHEDULER_IMPL: SchedulerImpl = SchedulerImpl {});
