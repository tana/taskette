#![no_std]
#![no_main]

use defmt::{error, info};
use defmt_rtt as _;
use embedded_hal::{delay::DelayNs, digital::OutputPin};
use panic_halt as _;
use rp2040_hal::{
    Clock,
    gpio::{FunctionSio, Pin, PullDown, SioOutput, bank0::Gpio25},
};
use static_cell::StaticCell;
use taskette::{
    scheduler::{SchedulerConfig, spawn},
    task::TaskConfig,
};
use taskette_cortex_m::{Stack, init_scheduler};
use taskette_utils::delay::Delay;
use usb_device::{
    UsbError,
    bus::{UsbBus, UsbBusAllocator},
    device::{StringDescriptors, UsbDeviceBuilder, UsbVidPid},
};
use usbd_serial::{SerialPort, USB_CLASS_CDC};

static BLINK_TASK_STACK: StaticCell<Stack<8192>> = StaticCell::new();
static USB_TASK_STACK: StaticCell<Stack<8192>> = StaticCell::new();

// This is necessary when directly using HAL without BSP
// Reference: https://github.com/rp-rs/rp-hal/blob/50a77826533f759b331076712d151e93650cc2bc/rp2040-hal-examples/src/bin/blinky.rs#L27-L33
#[unsafe(link_section = ".boot2")]
#[used]
pub static BOOT2: [u8; 256] = rp2040_boot2::BOOT_LOADER_W25Q080;

const XTAL_FREQ: u32 = 12_000_000;
const TICK_FREQ: u32 = 1000;

#[rp2040_hal::entry]
fn main() -> ! {
    info!("Started");

    let mut peripherals = rp2040_hal::pac::Peripherals::take().unwrap();

    // Init RP2040 system
    let mut watchdog = rp2040_hal::Watchdog::new(peripherals.WATCHDOG);
    let clocks = rp2040_hal::clocks::init_clocks_and_plls(
        XTAL_FREQ,
        peripherals.XOSC,
        peripherals.CLOCKS,
        peripherals.PLL_SYS,
        peripherals.PLL_USB,
        &mut peripherals.RESETS,
        &mut watchdog,
    )
    .unwrap();

    // Init peripherals for blinking
    let sio = rp2040_hal::Sio::new(peripherals.SIO);
    let pins = rp2040_hal::gpio::Pins::new(
        peripherals.IO_BANK0,
        peripherals.PADS_BANK0,
        sio.gpio_bank0,
        &mut peripherals.RESETS,
    );
    let led_pin = pins.gpio25.into_push_pull_output();

    // Init USB bus
    let usb_bus = UsbBusAllocator::new(rp2040_hal::usb::UsbBus::new(
        peripherals.USBCTRL_REGS,
        peripherals.USBCTRL_DPRAM,
        clocks.usb_clock,
        true,
        &mut peripherals.RESETS,
    ));

    // Init scheduler
    let core_peripherals = cortex_m::Peripherals::take().unwrap();
    let scheduler = init_scheduler(
        core_peripherals.SYST,
        core_peripherals.SCB,
        clocks.system_clock.freq().to_Hz(),
        SchedulerConfig::default().with_tick_freq(TICK_FREQ),
    )
    .unwrap();

    // Start LED blinking task
    let blink_task_stack = BLINK_TASK_STACK.init(Stack::new());
    let _blink_task = spawn(
        move || blink_task_func(led_pin),
        blink_task_stack,
        TaskConfig::default(),
    )
    .unwrap();

    // Start USB task
    let usb_task_stack = USB_TASK_STACK.init(Stack::new());
    let _usb_task = spawn(
        move || usb_task_func(usb_bus),
        usb_task_stack,
        TaskConfig::default(),
    )
    .unwrap();

    scheduler.start();
}

fn blink_task_func(mut led_pin: Pin<Gpio25, FunctionSio<SioOutput>, PullDown>) {
    info!("Blink task started");

    let mut delay = Delay::new().unwrap();

    loop {
        led_pin.set_high().unwrap();
        delay.delay_ms(500);

        led_pin.set_low().unwrap();
        delay.delay_ms(500);
    }
}

fn usb_task_func(usb_bus: UsbBusAllocator<rp2040_hal::usb::UsbBus>) {
    info!("USB task started");

    let mut serial_port = SerialPort::new(&usb_bus);

    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16C0, 0x27DD))
        .strings(&[StringDescriptors::default()
            .manufacturer("tana_ash")
            .product("taskette demo")
            .serial_number("TEST")])
        .unwrap()
        .device_class(USB_CLASS_CDC)
        .build();

    let mut last_dev_state = usb_dev.state();

    loop {
        if !usb_dev.poll(&mut [&mut serial_port]) {
            continue;
        }

        let dev_state = usb_dev.state();
        if dev_state != last_dev_state {
            info!("USB state changed: {}", dev_state);
            last_dev_state = dev_state;
        }

        let mut buf = [0u8; 64];
        match serial_port.read(&mut buf) {
            Ok(count) => {
                info!("CDC Received: {}", buf[..count]);

                // Echo the received data back
                match write_all(&mut serial_port, &buf[..count]) {
                    Ok(_) => (),
                    Err(err) => error!("{}", err),
                }
            }
            Err(UsbError::WouldBlock) => (),
            Err(err) => error!("{}", err),
        }
    }
}

fn write_all(serial_port: &mut SerialPort<'_, impl UsbBus>, data: &[u8]) -> Result<(), UsbError> {
    let mut idx = 0;

    while idx < data.len() {
        match serial_port.write(data) {
            Ok(n) => idx += n,
            Err(UsbError::WouldBlock) => (),
            Err(err) => return Err(err),
        }
    }

    Ok(())
}
