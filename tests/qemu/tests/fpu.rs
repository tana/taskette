//! Test of FPU register saving in context switch
//! Inspired by the RegTests of FreeRTOS:
//!     https://freertos.org/Documentation/02-Kernel/06-Coding-guidelines/02-FreeRTOS-Coding-Standard-and-Style-Guide#testing
//!     https://github.com/FreeRTOS/FreeRTOS/blob/5424d9d36a364ba9c73955c500d16773f543bb9c/FreeRTOS/Demo/CORTEX_M4F_M0_LPC43xx_Keil/M4/RegTest.c

#![no_std]
#![no_main]

mod panic_handler;
mod utils;

use semihosting::{println, process::ExitCode};
use static_cell::StaticCell;
use taskette::{scheduler::spawn, task::TaskConfig};

use crate::utils::{Stack, entry, init_scheduler};

static TASK1_STACK: StaticCell<Stack<8192>> = StaticCell::new();
static TASK2_STACK: StaticCell<Stack<8192>> = StaticCell::new();

#[entry]
fn main() -> ! {
    let scheduler = init_scheduler(1000).unwrap();

    let task1_stack = TASK1_STACK.init(Stack::new());
    let _task1 = spawn(move || unsafe {
        loop {
            // Continuously overwrite FPU registers
            core::arch::asm!(
                "vmov.f32 s0, #-1.0",
                "vmov.f32 s1, #-1.0",
                "vmov.f32 s2, #-1.0",
                "vmov.f32 s3, #-1.0",
                "vmov.f32 s4, #-1.0",
                "vmov.f32 s5, #-1.0",
                "vmov.f32 s6, #-1.0",
                "vmov.f32 s7, #-1.0",
                "vmov.f32 s8, #-1.0",
                "vmov.f32 s9, #-1.0",
                "vmov.f32 s10, #-1.0",
                "vmov.f32 s11, #-1.0",
                "vmov.f32 s12, #-1.0",
                "vmov.f32 s13, #-1.0",
                "vmov.f32 s14, #-1.0",
                "vmov.f32 s15, #-1.0",
                "vmov.f32 s16, #-1.0",
                "vmov.f32 s17, #-1.0",
                "vmov.f32 s18, #-1.0",
                "vmov.f32 s19, #-1.0",
                "vmov.f32 s20, #-1.0",
                "vmov.f32 s21, #-1.0",
                "vmov.f32 s22, #-1.0",
                "vmov.f32 s23, #-1.0",
                "vmov.f32 s24, #-1.0",
                "vmov.f32 s25, #-1.0",
                "vmov.f32 s26, #-1.0",
                "vmov.f32 s27, #-1.0",
                "vmov.f32 s28, #-1.0",
                "vmov.f32 s29, #-1.0",
                "vmov.f32 s30, #-1.0",
                "vmov.f32 s31, #-1.0",
                out("s0") _,
                out("s1") _,
                out("s2") _,
                out("s3") _,
                out("s4") _,
                out("s5") _,
                out("s6") _,
                out("s7") _,
                out("s8") _,
                out("s9") _,
                out("s10") _,
                out("s11") _,
                out("s12") _,
                out("s13") _,
                out("s14") _,
                out("s15") _,
                out("s16") _,
                out("s17") _,
                out("s18") _,
                out("s19") _,
                out("s20") _,
                out("s21") _,
                out("s22") _,
                out("s23") _,
                out("s24") _,
                out("s25") _,
                out("s26") _,
                out("s27") _,
                out("s28") _,
                out("s29") _,
                out("s30") _,
                out("s31") _,
            );
        }
    }, task1_stack, TaskConfig::default()).unwrap();

    let task2_stack = TASK2_STACK.init(Stack::new());
    let _task2 = spawn(move || unsafe {
        let mut result = true;

        for _ in 0..100 {
                let mut values = [0.0f32; 32];

                // Set values to registers
                core::arch::asm!(
                    "vmov.f32 s0, #1.0",
                    "vmov.f32 s1, #1.0",
                    "vmov.f32 s2, #2.0",
                    "vmov.f32 s3, #3.0",
                    "vmov.f32 s4, #4.0",
                    "vmov.f32 s5, #5.0",
                    "vmov.f32 s6, #6.0",
                    "vmov.f32 s7, #7.0",
                    "vmov.f32 s8, #8.0",
                    "vmov.f32 s9, #9.0",
                    "vmov.f32 s10, #10.0",
                    "vmov.f32 s11, #11.0",
                    "vmov.f32 s12, #12.0",
                    "vmov.f32 s13, #13.0",
                    "vmov.f32 s14, #14.0",
                    "vmov.f32 s15, #15.0",
                    "vmov.f32 s16, #16.0",
                    "vmov.f32 s17, #17.0",
                    "vmov.f32 s18, #18.0",
                    "vmov.f32 s19, #19.0",
                    "vmov.f32 s20, #20.0",
                    "vmov.f32 s21, #21.0",
                    "vmov.f32 s22, #22.0",
                    "vmov.f32 s23, #23.0",
                    "vmov.f32 s24, #24.0",
                    "vmov.f32 s25, #25.0",
                    "vmov.f32 s26, #26.0",
                    "vmov.f32 s27, #27.0",
                    "vmov.f32 s28, #28.0",
                    "vmov.f32 s29, #29.0",
                    "vmov.f32 s30, #30.0",
                    "vmov.f32 s31, #31.0",
                    // Force a context switch
                    // To avoid function call, it directly cause PendSV exception by setting PENDSVSET bit of ICSR
                    "str r1, [r0]",
                    // Load register values
                    "vstr.32 s0, [r2, #4*0]",
                    "vstr.32 s1, [r2, #4*1]",
                    "vstr.32 s2, [r2, #4*2]",
                    "vstr.32 s3, [r2, #4*3]",
                    "vstr.32 s4, [r2, #4*4]",
                    "vstr.32 s5, [r2, #4*5]",
                    "vstr.32 s6, [r2, #4*6]",
                    "vstr.32 s7, [r2, #4*7]",
                    "vstr.32 s8, [r2, #4*8]",
                    "vstr.32 s9, [r2, #4*9]",
                    "vstr.32 s10, [r2, #4*10]",
                    "vstr.32 s11, [r2, #4*11]",
                    "vstr.32 s12, [r2, #4*12]",
                    "vstr.32 s13, [r2, #4*13]",
                    "vstr.32 s14, [r2, #4*14]",
                    "vstr.32 s15, [r2, #4*15]",
                    "vstr.32 s16, [r2, #4*16]",
                    "vstr.32 s17, [r2, #4*17]",
                    "vstr.32 s18, [r2, #4*18]",
                    "vstr.32 s19, [r2, #4*19]",
                    "vstr.32 s20, [r2, #4*20]",
                    "vstr.32 s21, [r2, #4*21]",
                    "vstr.32 s22, [r2, #4*22]",
                    "vstr.32 s23, [r2, #4*23]",
                    "vstr.32 s24, [r2, #4*24]",
                    "vstr.32 s25, [r2, #4*25]",
                    "vstr.32 s26, [r2, #4*26]",
                    "vstr.32 s27, [r2, #4*27]",
                    "vstr.32 s28, [r2, #4*28]",
                    "vstr.32 s29, [r2, #4*29]",
                    "vstr.32 s30, [r2, #4*30]",
                    "vstr.32 s31, [r2, #4*31]",
                    out("s0") _,
                    out("s1") _,
                    out("s2") _,
                    out("s3") _,
                    out("s4") _,
                    out("s5") _,
                    out("s6") _,
                    out("s7") _,
                    out("s8") _,
                    out("s9") _,
                    out("s10") _,
                    out("s11") _,
                    out("s12") _,
                    out("s13") _,
                    out("s14") _,
                    out("s15") _,
                    out("s16") _,
                    out("s17") _,
                    out("s18") _,
                    out("s19") _,
                    out("s20") _,
                    out("s21") _,
                    out("s22") _,
                    out("s23") _,
                    out("s24") _,
                    out("s25") _,
                    out("s26") _,
                    out("s27") _,
                    out("s28") _,
                    out("s29") _,
                    out("s30") _,
                    out("s31") _,
                    in("r0") 0xE000ED04u32,
                    in("r1") (1 << 28),
                    in("r2") values.as_mut_ptr(),
                );

                // Verify values
                let correct = [
                    1.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0,
                    10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0,
                    20.0, 21.0, 22.0, 23.0, 24.0, 25.0, 26.0, 27.0, 28.0, 29.0,
                    30.0, 31.0,
                ];
                for i in 0..=31 {
                    if (values[i] - correct[i]).abs() > core::f32::EPSILON {
                        result = false;
                        println!("r{} = {}", i, values[i]);
                    }
                }
        }

        if result {
            ExitCode::SUCCESS.exit_process();
        } else {
            ExitCode::FAILURE.exit_process();
        }
    }, task2_stack, TaskConfig::default()).unwrap();

    scheduler.start();
}
