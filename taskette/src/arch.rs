//! Interface for architecture-dependent functions implemented in separate crates.

unsafe extern "Rust" {
    /// INTERNAL USE ONLY
    pub unsafe fn _taskette_setup(clock_freq: u32, tick_freq: u32);
    /// INTERNAL USE ONLY
    pub unsafe fn _taskette_start_timer();
    /// INTERNAL USE ONLY
    pub unsafe fn _taskette_yield_now();
    /// INTERNAL USE ONLY
    pub unsafe fn _taskette_init_stack(
        sp: *mut u8,
        pc: usize,
        arg: *const u8,
        arg_size: usize,
    ) -> *mut u8;
    /// INTERNAL USE ONLY
    pub unsafe fn _taskette_wait_for_interrupt();
}

/// Incurs a context switch and yields the CPU to another task.
pub fn yield_now() {
    unsafe {
        _taskette_yield_now();
    }
}

/// Trait for a stack allocation that meets architecture-specific requirements such as alignment.
/// Modeled after `rp2040_hal`. https://docs.rs/rp2040-hal/0.11.0/rp2040_hal/multicore/struct.StackAllocation.html
pub trait StackAllocation {
    fn as_mut_slice(&mut self) -> &mut [u8];
}
