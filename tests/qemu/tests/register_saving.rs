//! Test of register saving in context switch
//! Inspired by the RegTests of FreeRTOS:
//!     https://freertos.org/Documentation/02-Kernel/06-Coding-guidelines/02-FreeRTOS-Coding-Standard-and-Style-Guide#testing
//!     https://github.com/FreeRTOS/FreeRTOS/blob/5424d9d36a364ba9c73955c500d16773f543bb9c/FreeRTOS/Demo/CORTEX_M4F_M0_LPC43xx_Keil/M4/RegTest.c

#![no_std]
#![no_main]

use cortex_m_semihosting::{debug, hprintln};
use panic_semihosting as _;
use static_cell::StaticCell;
use taskette::{SchedulerConfig, TaskConfig};
use taskette_cortex_m::{Stack, init_scheduler};

static TASK1_STACK: StaticCell<Stack<8192>> = StaticCell::new();
static TASK2_STACK: StaticCell<Stack<8192>> = StaticCell::new();

#[cortex_m_rt::entry]
fn main() -> ! {
    let peripherals = cortex_m::Peripherals::take().unwrap();
    let scheduler = init_scheduler(
        peripherals.SYST,
        peripherals.SCB,
        12_000_000,
        SchedulerConfig::default().with_tick_freq(1000),
    ).unwrap();

    let task1_stack = TASK1_STACK.init(Stack::new());
    let _task1 = scheduler.spawn(move || unsafe {
        loop {
            // Continuously overwrite to some general-purpose registers
            core::arch::asm!(
                "mov r0, #42",
                "mov r1, #42",
                "mov r2, #42",
                "mov r3, #42",
                "mov r4, #42",
                "mov r5, #42",
                "mov r8, #42",
                "mov r9, #42",
                "mov r10, #42",
                "mov r11, #42",
                "mov r12, #42",
                out("r0") _,
                out("r1") _,
                out("r2") _,
                out("r3") _,
                out("r4") _,
                out("r5") _,
                out("r8") _,
                out("r9") _,
                out("r10") _,
                out("r11") _,
                out("r12") _,
            );
        }
    }, task1_stack, TaskConfig::default()).unwrap();

    let task2_stack = TASK2_STACK.init(Stack::new());
    let _task2 = scheduler.spawn(move || unsafe {
        let mut result = true;

        for _ in 0..100 {
                let mut values = [0u32; 13];

                // Set values to registers
                core::arch::asm!(
                    "mov r3, #3",
                    "mov r4, #4",
                    "mov r5, #5",
                    "mov r8, #8",
                    "mov r9, #9",
                    "mov r10, #10",
                    "mov r11, #11",
                    "mov r12, #12",
                    // Force a context switch
                    // To avoid function call, it directly cause PendSV exception by setting PENDSVSET bit of ICSR
                    "str r1, [r0]",
                    // Load register values
                    "str r3, [r2, #4*3]",
                    "str r4, [r2, #4*4]",
                    "str r5, [r2, #4*5]",
                    "str r8, [r2, #4*8]",
                    "str r9, [r2, #4*9]",
                    "str r10, [r2, #4*10]",
                    "str r11, [r2, #4*11]",
                    "str r12, [r2, #4*12]",
                    out("r3") _,
                    out("r4") _,
                    out("r5") _,
                    out("r8") _,
                    out("r9") _,
                    out("r10") _,
                    out("r11") _,
                    out("r12") _,
                    in("r0") 0xE000ED04u32,
                    in("r1") (1 << 28),
                    in("r2") values.as_mut_ptr(),
                );

                // Verify values
                let correct = [0, 0, 0, 3, 4, 5, 0, 0, 8, 9, 10, 11, 12];
                for i in 3..=12 {
                    if values[i] != correct[i] {
                        result = false;
                        hprintln!("r{} = {}", i, values[i]);
                    }
                }
        }

        if result {
            debug::exit(debug::EXIT_SUCCESS);
        } else {
            debug::exit(debug::EXIT_FAILURE);
        }
    }, task2_stack, TaskConfig::default()).unwrap();

    scheduler.start();
}
