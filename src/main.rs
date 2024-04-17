#![no_std]
#![no_main]

use core::cell::{Cell, RefCell};

use rp_pico::entry;
use panic_halt as _;
// use rp_pico::hal::prelude::*;
use rp_pico::hal; // Hardware Abstraction Layer (higher-level drivers)
use rp_pico::hal::pac; // Peripheral Access Crate (low-level register access)
use rp_pico::hal::pac::interrupt;
use rp_pico::hal::timer::{Alarm, Alarm0};
use embedded_hal::digital::{OutputPin, StatefulOutputPin};
use cortex_m::interrupt::{CriticalSection, Mutex};

use fugit::MicrosDurationU32;

// TODO: look at https://github.com/knurling-rs/flip-link

static ALARM0: Mutex<RefCell<Option<Alarm0>>> = Mutex::new(RefCell::new(None));
static FRAME_DURATION: Mutex<Cell<MicrosDurationU32>> = Mutex::new(Cell::new(MicrosDurationU32::millis(500)));
static TRANSMIT_FRAME: Mutex<Cell<bool>> = Mutex::new(Cell::new(false));

#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();

    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);

    // Configure the clocks (125 MHz system clock)
    // TODO: do I need to do anything here to make WFI energy-efficient by pruning the clock tree?
    let clocks = hal::clocks::init_clocks_and_plls(
        rp_pico::XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    ).unwrap();

    // Set up peripherals (GPIO, Timer/Alarm, and maybe SPI for data transmission)
    let sio = hal::Sio::new(pac.SIO); // single-cycle IO
    let pins = rp_pico::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );
    let mut led_pin = pins.led.into_push_pull_output();
    led_pin.set_high().unwrap();
    // TODO: set power regulator mode (PFM (low; default) vs PWM (high)) controlled via GPIO23 (pins.b_power_save)

    let mut timer = hal::Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);
    let mut alarm0 = timer.alarm_0().unwrap();
    alarm0.enable_interrupt();
    cortex_m::interrupt::free(|cs| {
        ALARM0.borrow(cs).replace(Some(alarm0));
        reschedule_alarm(cs);
    });

    unsafe {
        pac::NVIC::unmask(pac::Interrupt::TIMER_IRQ_0);
    }

    // submit empty frame
    loop {
        // compute next frame

        // sleep until next frame should be transmitted
        while !cortex_m::interrupt::free(|cs| TRANSMIT_FRAME.borrow(cs).get()) {
            // led_pin.set_low().unwrap();
            cortex_m::asm::wfi();
        }
        cortex_m::interrupt::free(|cs| TRANSMIT_FRAME.borrow(cs).set(false));
        // led_pin.set_high().unwrap();

        // transmit frame (toggle LED to show activity)
        led_pin.toggle().unwrap();
    }
}

fn reschedule_alarm(cs: &CriticalSection) {
    let duration = FRAME_DURATION.borrow(cs).get();
    let mut alarm = ALARM0.borrow(cs).borrow_mut();
    let alarm = alarm.as_mut().unwrap();
    alarm.schedule(duration).unwrap();
    alarm.clear_interrupt();
}

#[interrupt]
fn TIMER_IRQ_0() {
    cortex_m::interrupt::free(|cs| {
        TRANSMIT_FRAME.borrow(cs).set(true);
        reschedule_alarm(cs);
    });
}
