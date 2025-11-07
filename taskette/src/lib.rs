#![no_std]

use cortex_m::peripheral::{SCB, syst::SystClkSource};
use log::{info, trace};

#[non_exhaustive]
pub struct KernelConfig {
    pub tick_freq: u32,
}

impl KernelConfig {
    pub fn with_tick_freq(self, tick_freq: u32) -> Self {
        Self { tick_freq, ..self }
    }
}

impl Default for KernelConfig {
    fn default() -> Self {
        Self { tick_freq: 1000 }
    }
}

pub struct Kernel {
    syst: cortex_m::peripheral::SYST,
    clock_freq: u32,
    config: KernelConfig,
}

impl Kernel {
    pub fn new(syst: cortex_m::peripheral::SYST, clock_freq: u32, config: KernelConfig) -> Self {
        Kernel {
            syst,
            clock_freq,
            config,
        }
    }

    pub fn start(&mut self) {
        self.syst.set_clock_source(SystClkSource::Core);
        self.syst
            .set_reload(self.clock_freq / self.config.tick_freq);
        self.syst.enable_interrupt();
        self.syst.enable_counter();

        info!("Kernel started");
    }
}

/// Context switching procedure
#[unsafe(no_mangle)]
#[unsafe(naked)]
extern "C" fn PendSV() {
    core::arch::naked_asm!(
        "mov r0, sp",   // Use the SP value as the argument for `switch_context`
        "push {{lr}}",   // Save LR that is modified by the next BL
        "bl {switch_context}",  // Call `switch_context` function
        "pop {{lr}}",    // Restore LR (to EXC_RETURN)
        "mov sp, r0",   // Change SP into the value returned by `switch_context`
        "bx lr",
        switch_context = sym switch_context,
    );
}

extern "C" fn switch_context(orig_sp: usize) -> usize {
    trace!("orig_sp = {:08X}", orig_sp);
    orig_sp
}

#[cortex_m_rt::exception]
fn SysTick() {
    trace!("SysTick handler");
    yield_now();
}

pub fn yield_now() {
    SCB::set_pendsv();
}
