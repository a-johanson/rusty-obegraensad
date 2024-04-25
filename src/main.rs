#![no_std]
#![no_main]

mod display;

use core::cell::RefCell;

use rp_pico::entry;
use panic_halt as _;
// use rp_pico::hal::prelude::*;
use rp_pico::hal; // Hardware Abstraction Layer (higher-level drivers)
use rp_pico::hal::pac; // Peripheral Access Crate (low-level register access)
use rp_pico::hal::pac::interrupt;
use rp_pico::hal::timer::{Alarm, Alarm0};
use rp_pico::hal::gpio::{FunctionSpi, PinState};
use rp_pico::hal::dma::{single_buffer, SingleChannel, DMAExt};
use rp_pico::hal::Clock;
use embedded_hal::digital::OutputPin;
use cortex_m::interrupt::Mutex;
use cortex_m::singleton;

use portable_atomic::{AtomicU32, AtomicU8, Ordering};
use fugit::MicrosDurationU32;
use fugit::RateExtU32;

// TODO: look at https://github.com/knurling-rs/flip-link

static SHARED_STATE: Mutex<RefCell<Option<SharedState>>> = Mutex::new(RefCell::new(None));
static ATOMIC_STATE: AtomicState = AtomicState::new();

type DisplaySpi = hal::Spi<hal::spi::Enabled, pac::SPI0, (hal::gpio::Pin<hal::gpio::bank0::Gpio19, hal::gpio::FunctionSpi, hal::gpio::PullDown>, hal::gpio::Pin<hal::gpio::bank0::Gpio18, hal::gpio::FunctionSpi, hal::gpio::PullDown>)>;
type SpiDmaChannel =  hal::dma::Channel<hal::dma::CH0>;
type SpiDmaTransfer = single_buffer::Transfer<hal::dma::Channel<hal::dma::CH0>, &'static mut [u8; 32], hal::Spi<hal::spi::Enabled, pac::SPI0, (hal::gpio::Pin<hal::gpio::bank0::Gpio19, FunctionSpi, hal::gpio::PullDown>, hal::gpio::Pin<hal::gpio::bank0::Gpio18, FunctionSpi, hal::gpio::PullDown>)>>;
type LatchPin = hal::gpio::Pin<hal::gpio::bank0::Gpio21, hal::gpio::FunctionSio<hal::gpio::SioOutput>, hal::gpio::PullDown>;

struct SharedState {
    alarm0: Alarm0,
    spi: Option<DisplaySpi>,
    dma_channel: Option<SpiDmaChannel>,
    dma_buffer: Option<&'static mut [u8; 32]>,
    spi_dma_transfer: Option<SpiDmaTransfer>,
    pin_latch: LatchPin,
}

struct AtomicState {
    frame_duration: AtomicU32,
    transmit_next_frame: AtomicU8,
}

impl AtomicState {
    pub const fn new() -> Self {
        Self {
            frame_duration: AtomicU32::new(MicrosDurationU32::millis(500).to_micros()),
            transmit_next_frame: AtomicU8::new(0),
        }
    }
}

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
    let mut pin_not_enable = pins.gpio20.into_push_pull_output_in_state(PinState::High);
    let mut pin_led = pins.led.into_push_pull_output_in_state(PinState::High);
    let pin_latch = pins.gpio21.into_push_pull_output_in_state(PinState::Low);
    // let pin_clock = pins.gpio18.into_push_pull_output_in_state(PinState::Low);
    // let pin_data = pins.gpio19.into_push_pull_output();

    // TODO: set power regulator mode (PFM (low; default) vs PWM (high)) controlled via GPIO23 (pins.b_power_save)

    // let core = pac::CorePeripherals::take().unwrap();
    // let delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());

    let pin_clock = pins.gpio18.into_function::<FunctionSpi>();
    let pin_data = pins.gpio19.into_function::<FunctionSpi>();
    // let spi_rx: hal::gpio::Pin<_, hal::gpio::FunctionSpi, hal::gpio::PullUp> = pins.gpio16.reconfigure();
    // let spi_cs = pins.gpio17.into_push_pull_output();
    let spi = hal::spi::Spi::<_, _, _, 8>::new(pac.SPI0, (pin_data, pin_clock));
    let spi = spi.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        500.kHz(),
        embedded_hal::spi::MODE_0,
    );

    // Initialize DMA
    let dma_buffer = singleton!(: [u8; 32] = [0; 32]).unwrap();
    let dma = pac.DMA.split(&mut pac.RESETS);
    let mut dma_channel = dma.ch0;
    dma_channel.enable_irq0();
    // transmit empty frame
    let spi_dma_transfer = single_buffer::Config::new(dma_channel, dma_buffer, spi).start();

    let mut timer = hal::Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);
    let mut alarm0 = timer.alarm_0().unwrap();
    alarm0.enable_interrupt();

    cortex_m::interrupt::free(|cs| {
        SHARED_STATE.borrow(cs).replace(Some(SharedState {
            alarm0,
            spi: None,
            dma_channel: None,
            dma_buffer: None,
            spi_dma_transfer: Some(spi_dma_transfer),
            pin_latch
        }));
    });

    reschedule_alarm0();

    unsafe {
        pac::NVIC::unmask(pac::Interrupt::TIMER_IRQ_0);
        pac::NVIC::unmask(pac::Interrupt::DMA_IRQ_0);
    }

    let mut display = display::ObegraensadDisplay::new();
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
        pin_led.set_low().unwrap();
        while ATOMIC_STATE.transmit_next_frame.load(Ordering::Relaxed) == 0 {
            cortex_m::asm::wfi();
        }
        ATOMIC_STATE.transmit_next_frame.store(0, Ordering::Relaxed);
        pin_led.set_high().unwrap();
        pin_not_enable.set_low().unwrap();

        // transmit frame
        cortex_m::interrupt::free(|cs| {
            let mut shared_state = SHARED_STATE.borrow(cs).borrow_mut();
            let dma_buffer = shared_state.as_mut().map(|s| s.dma_buffer.take().unwrap()).unwrap();
            display.to_output_buffer(dma_buffer);
            let dma_channel = shared_state.as_mut().map(|s| s.dma_channel.take().unwrap()).unwrap();
            let spi = shared_state.as_mut().map(|s| s.spi.take().unwrap()).unwrap();
            let spi_dma_transfer = single_buffer::Config::new(dma_channel, dma_buffer, spi).start();
            shared_state.as_mut().map(|s| s.spi_dma_transfer.replace(spi_dma_transfer));
        });
    }
}

fn reschedule_alarm0() {
    let frame_mircos = ATOMIC_STATE.frame_duration.load(Ordering::Relaxed);
    cortex_m::interrupt::free(|cs| {
        let mut shared_state = SHARED_STATE.borrow(cs).borrow_mut();
        let alarm = &mut shared_state.as_mut().unwrap().alarm0;
        alarm.schedule(MicrosDurationU32::micros(frame_mircos)).unwrap();
        alarm.clear_interrupt();
    });
}

#[interrupt]
fn TIMER_IRQ_0() {
    ATOMIC_STATE.transmit_next_frame.store(1, Ordering::Relaxed);
    reschedule_alarm0();
}

#[interrupt]
fn DMA_IRQ_0() {
    cortex_m::interrupt::free(|cs| {
        let mut shared_state = SHARED_STATE.borrow(cs).borrow_mut();
        let mut spi_dma_transfer = shared_state.as_mut().map(|s| s.spi_dma_transfer.take().unwrap()).unwrap();
        spi_dma_transfer.check_irq0();
        let (dma_channel, dma_buffer, spi) = spi_dma_transfer.wait();
        shared_state.as_mut().unwrap().pin_latch.set_high().unwrap();
        shared_state.as_mut().map(|s| s.spi.replace(spi));
        shared_state.as_mut().map(|s| s.dma_channel.replace(dma_channel));
        shared_state.as_mut().map(|s| s.dma_buffer.replace(dma_buffer));
        cortex_m::asm::nop();
        cortex_m::asm::nop();
        cortex_m::asm::nop();
        shared_state.as_mut().unwrap().pin_latch.set_low().unwrap();
    });
}
