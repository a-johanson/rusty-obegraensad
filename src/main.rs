#![no_std]
#![no_main]

mod display;

use core::cell::{Cell, RefCell};

use rp_pico::entry;
use panic_halt as _;
// use rp_pico::hal::prelude::*;
use rp_pico::hal; // Hardware Abstraction Layer (higher-level drivers)
use rp_pico::hal::pac; // Peripheral Access Crate (low-level register access)
use rp_pico::hal::pac::interrupt;
use rp_pico::hal::timer::{Alarm, Alarm0};
use rp_pico::hal::gpio::PinState;
use rp_pico::hal::Clock;
use embedded_hal::digital::OutputPin;
use cortex_m::interrupt::{CriticalSection, Mutex};

use fugit::MicrosDurationU32;
// use fugit::RateExtU32;

// TODO: look at https://github.com/knurling-rs/flip-link

static ALARM0: Mutex<RefCell<Option<Alarm0>>> = Mutex::new(RefCell::new(None));
static FRAME_DURATION: Mutex<Cell<MicrosDurationU32>> = Mutex::new(Cell::new(MicrosDurationU32::millis(500)));
static TRANSMIT_FRAME: Mutex<Cell<bool>> = Mutex::new(Cell::new(false));

#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();
    let core = pac::CorePeripherals::take().unwrap();

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

    let delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    // Set up peripherals (GPIO, Timer/Alarm, and maybe SPI for data transmission)
    let sio = hal::Sio::new(pac.SIO); // single-cycle IO
    let pins = rp_pico::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );
    let mut pin_led = pins.led.into_push_pull_output_in_state(PinState::High);
    let mut pin_not_enable = pins.gpio20.into_push_pull_output_in_state(PinState::High);
    let pin_latch = pins.gpio21.into_push_pull_output_in_state(PinState::Low);
    let pin_clock = pins.gpio18.into_push_pull_output_in_state(PinState::Low);
    let pin_data = pins.gpio19.into_push_pull_output();

    let mut display = display::ObegraensadDisplay::new(pin_clock, pin_data, pin_latch, delay);

    // let spi_clk: hal::gpio::Pin<_, hal::gpio::FunctionSpi, hal::gpio::PullNone> = pins.gpio18.reconfigure();
    // let spi_tx: hal::gpio::Pin<_, hal::gpio::FunctionSpi, hal::gpio::PullNone> = pins.gpio19.reconfigure();
    // let spi_rx: hal::gpio::Pin<_, hal::gpio::FunctionSpi, hal::gpio::PullUp> = pins.gpio16.reconfigure();
    // let spi_cs = pins.gpio17.into_push_pull_output();
    // let spi = hal::spi::Spi::<_, _, _, 8>::new(pac.SPI0, (spi_tx, spi_clk)); // TODO: omit spi_rx in (spi_tx, spi_rx, spi_clk)?
    // let spi = spi.init(
    //     &mut pac.RESETS,
    //     clocks.peripheral_clock.freq(),
    //     500.kHz(),
    //     embedded_hal::spi::MODE_0,
    // );
    // spi.write(words);

    // pin_led.set_high().unwrap();
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

    // transmit empty frame
    display.show();
    pin_not_enable.set_low().unwrap();
    let mut frame_count: u32 = 0;
    loop {
        // compute next frame
        display.clear();
        let pixel_idx: u8 = (frame_count & 0xFF) as u8;
        let x = pixel_idx & 0x0F;
        let y = pixel_idx >> 4;
        display.set_pixel(x, y);
        frame_count += 1;

        // sleep until next frame should be transmitted
        while !cortex_m::interrupt::free(|cs| TRANSMIT_FRAME.borrow(cs).get()) {
            pin_led.set_low().unwrap();
            cortex_m::asm::wfi();
        }
        cortex_m::interrupt::free(|cs| TRANSMIT_FRAME.borrow(cs).set(false));
        pin_led.set_high().unwrap();

        // transmit frame
        display.show();
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
