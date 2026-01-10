//! To prevent conflict between the `panic-handler` feature of `semihosting` and custom handler in `stack_canary` test,
//! We use a custom-made handler that is linked if the test has `mod panic_handler;`.

use core::panic::PanicInfo;

use semihosting::{println, process::ExitCode};

#[panic_handler]
fn panic_handler(info: &PanicInfo<'_>) -> ! {
    println!("{:?}", info);
    ExitCode::FAILURE.exit_process();
}