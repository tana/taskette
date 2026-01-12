// This file is released in the public domain.

//! Taskette demo for ESP32C3/C6

#![no_std]
#![no_main]

use defmt::info;
use embedded_hal::delay::DelayNs;
use esp_backtrace as _;
use esp_hal::{
    interrupt::software::SoftwareInterruptControl,
    peripherals::RMT,
    rmt::Rmt,
    time::Rate,
};
use esp_hal_smartled::{SmartLedsAdapter, smart_led_buffer};
use esp_println as _;
use smart_leds::{SmartLedsWrite, brightness};
use static_cell::ConstStaticCell;
use taskette::{
    scheduler::{SchedulerConfig, spawn},
    task::TaskConfig,
};
use taskette_esp_riscv::{Stack, init_scheduler};
use taskette_utils::delay::Delay;

#[cfg(feature = "rgb-gpio2")]
use esp_hal::peripherals::GPIO2 as RgbGpio;
#[cfg(feature = "rgb-gpio8")]
use esp_hal::peripherals::GPIO8 as RgbGpio;
#[cfg(feature = "rgb-gpio20")]
use esp_hal::peripherals::GPIO20 as RgbGpio;

static BLINK_TASK_STACK: ConstStaticCell<Stack<8192>> = ConstStaticCell::new(Stack::new());

const TICK_FREQ: u32 = 1000;

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_hal::main]
fn main() -> ! {
    info!("Started");

    let clock = esp_hal::clock::CpuClock::max();
    let peripherals = esp_hal::init(esp_hal::Config::default().with_cpu_clock(clock));

    info!("CPU Running at {} MHz", clock as u32);

    // Init scheduler
    let swint = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    let scheduler = init_scheduler(
        peripherals.SYSTIMER,
        swint.software_interrupt0,
        1_000_000 * clock as u32,
        SchedulerConfig::default().with_tick_freq(TICK_FREQ),
    )
    .unwrap();

    #[cfg(feature = "rgb-gpio2")]
    let rgb_gpio = peripherals.GPIO2;
    #[cfg(feature = "rgb-gpio8")]
    let rgb_gpio = peripherals.GPIO8;
    #[cfg(feature = "rgb-gpio20")]
    let rgb_gpio = peripherals.GPIO20;

    #[cfg(feature = "rgbpwr-gpio19")]
    {
        use esp_hal::gpio::{Level, Output, OutputConfig};
        let _rgbpwr = Output::new(peripherals.GPIO19, Level::High, OutputConfig::default());
    }

    // Start LED blinking task
    let blink_task_stack = BLINK_TASK_STACK.take();
    let _blink_task = spawn(
        move || blink_task_func(peripherals.RMT, rgb_gpio),
        blink_task_stack,
        TaskConfig::default(),
    )
    .unwrap();

    scheduler.start()
}

fn blink_task_func(rmt: RMT, rgb_gpio: RgbGpio) {
    info!("Blink task started");

    // Init smart LED
    let rmt = Rmt::new(rmt, Rate::from_mhz(80)).unwrap();
    let mut led_buf = smart_led_buffer!(1);
    let mut led = SmartLedsAdapter::new(rmt.channel0, rgb_gpio, &mut led_buf);

    let mut delay = Delay::new().unwrap();

    loop {
        info!("LED on");
        led.write(brightness([smart_leds::colors::RED].into_iter(), 10))
            .unwrap();
        delay.delay_ms(500);

        info!("LED off");
        led.write([smart_leds::colors::BLACK].into_iter()).unwrap();
        delay.delay_ms(500);
    }
}
