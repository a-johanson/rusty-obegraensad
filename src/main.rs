#![no_std]
#![no_main]

mod animation;
mod animation_leaves;
mod animation_drops;
mod display;

use core::cell::RefCell;

use rp_pico::entry; // rp_pico = Board Support Package (BSP; https://github.com/rp-rs/rp-hal-boards/)
use panic_halt as _;
use rp_pico::hal; // Hardware Abstraction Layer for Raspberry Silicon (higher-level drivers; https://github.com/rp-rs/rp-hal/)
use rp_pico::hal::pac; // Peripheral Access Crate (low-level register access; https://github.com/rp-rs/rp2040-pac)
use rp_pico::hal::pac::interrupt;
use rp_pico::hal::timer::{Alarm, Alarm0};
use rp_pico::hal::gpio::{FunctionSpi, PinState};
use rp_pico::hal::dma::{single_buffer, DMAExt};
use rp_pico::hal::Clock;
use embedded_hal::digital::{InputPin, OutputPin}; // General Hardware Abstraction Layer for embedded systems (https://github.com/rust-embedded/embedded-hal)
use cortex_m::interrupt::Mutex;
use cortex_m::singleton;

use portable_atomic::{AtomicU8, Ordering};
use fugit::{MicrosDurationU32, RateExtU32};

use animation::Animation;

// TODO: look at https://github.com/knurling-rs/flip-link

static ALARM0: Mutex<RefCell<Option<Alarm0>>> = Mutex::new(RefCell::new(None));
static TRANSMIT_NEXT_FRAME: AtomicU8 = AtomicU8::new(0);

#[entry]
fn main() -> ! {
    let mut pac = pac::Peripherals::take().unwrap();

    let mut watchdog = hal::Watchdog::new(pac.WATCHDOG);

    // Configure the clocks (125 MHz system clock)
    let clocks = hal::clocks::init_clocks_and_plls(
        rp_pico::XOSC_CRYSTAL_FREQ,
        pac.XOSC,
        pac.CLOCKS,
        pac.PLL_SYS,
        pac.PLL_USB,
        &mut pac.RESETS,
        &mut watchdog,
    ).unwrap();

    // To make WFI more energy-efficient, we could pruning the clock tree.
    // This can be done by selecting the respective bits in a rp_pico::hal::clocks::ClockGate
    // and by applying this config via clocks.configure_sleep_enable(cg_config);

    // Set up peripherals (GPIO, SPI, DMA, Timer/Alarm)
    let sio = hal::Sio::new(pac.SIO); // single-cycle IO
    let pins = rp_pico::Pins::new(
        pac.IO_BANK0,
        pac.PADS_BANK0,
        sio.gpio_bank0,
        &mut pac.RESETS,
    );
    let mut pin_not_enable = pins.gpio20.into_push_pull_output_in_state(PinState::High);
    let mut pin_led = pins.led.into_push_pull_output_in_state(PinState::High);
    let mut pin_latch = pins.gpio21.into_push_pull_output_in_state(PinState::Low);
    let mut pin_button = pins.gpio22.into_pull_up_input();

    let pin_clock = pins.gpio18.into_function::<FunctionSpi>();
    let pin_data = pins.gpio19.into_function::<FunctionSpi>();
    let spi = hal::spi::Spi::<_, _, _, 8>::new(pac.SPI0, (pin_data, pin_clock));
    let spi = spi.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        2.MHz(),
        embedded_hal::spi::MODE_0,
    );

    let dma = pac.DMA.split(&mut pac.RESETS);
    let dma_channel = dma.ch0;

    let mut timer = hal::Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);
    let mut alarm0 = timer.alarm_0().unwrap();
    alarm0.enable_interrupt();
    // Delay to transmit the initial blank frame and to activate the display
    alarm0.schedule(MicrosDurationU32::millis(10)).unwrap();
    cortex_m::interrupt::free(|cs| {
        ALARM0.borrow(cs).replace(Some(alarm0));
    });

    unsafe {
        pac::NVIC::unmask(pac::Interrupt::TIMER_IRQ_0);
    }

    let mut display = display::ObegraensadDisplay::new();
    let mut current_animation: u8 = 0;
    const ANIMATION_COUNT: u8 = 2;
    let mut animation_leaves = animation_leaves::FallingLeaves::new();
    let mut animation_drops = animation_drops::DropAnimation::new();
    let dma_buffer = singleton!(: [u8; 32] = [0; 32]).unwrap();
    let mut dma_ch_opt = Some(dma_channel);
    let mut spi_opt = Some(spi);
    let mut dma_buffer_opt = Some(dma_buffer);
    let mut current_frame_duration = MicrosDurationU32::millis(10);
    loop {
        // if the button is pressed...
        if pin_button.is_low().unwrap() {
            // clear the display, set a short frame duration, change animation index
            display.clear();
            current_frame_duration = MicrosDurationU32::millis(10);
            current_animation = (current_animation + 1) % ANIMATION_COUNT;

            // wait until the button is released (TODO: debounce)
            while pin_button.is_low().unwrap() {
                cortex_m::asm::nop();
            }

            // re-schedule the alarm
            cortex_m::interrupt::free(|cs| {
                ALARM0.borrow(cs).borrow_mut().as_mut().unwrap().schedule(current_frame_duration).unwrap();
            });
            TRANSMIT_NEXT_FRAME.store(0, Ordering::Relaxed);
        }

        // start to transmit display content (current frame) via SPI fed via DMA
        let dma_channel = dma_ch_opt.take().unwrap();
        let spi = spi_opt.take().unwrap();
        let dma_buffer = dma_buffer_opt.take().unwrap();
        display.to_output_buffer(dma_buffer);
        let spi_dma_transfer = single_buffer::Config::new(dma_channel, dma_buffer, spi).start();

        // compute next frame
        let next_frame_duration = if current_animation == 0 {
            animation_leaves.render_frame(&mut display)
        } else {
            animation_drops.render_frame(&mut display)
        };

        // Disable the activity LED and sleep until it's time to transmit the next frame
        pin_led.set_low().unwrap();
        while TRANSMIT_NEXT_FRAME.load(Ordering::Relaxed) == 0 {
            cortex_m::asm::wfi();
        }

        // Start pulsing the latch pin to show the current frame
        pin_latch.set_high().unwrap();

        // Reset frame transmission status and re-schedule the timer to determine how long the current frame should be shown
        TRANSMIT_NEXT_FRAME.store(0, Ordering::Relaxed);
        cortex_m::interrupt::free(|cs| {
            ALARM0.borrow(cs).borrow_mut().as_mut().unwrap().schedule(current_frame_duration).unwrap();
        });

        // Enable the activity LED
        pin_led.set_high().unwrap();

        // Ensure that upon the next transmission cycle, we display the next frame for the designated amount of time
        current_frame_duration = next_frame_duration;

        // Finish the latch pulse and enable display (in case it was disable before)
        cortex_m::asm::nop();
        cortex_m::asm::nop();
        cortex_m::asm::nop();
        pin_latch.set_low().unwrap();
        pin_not_enable.set_low().unwrap();

        // Ensure DMA -> SPI transmission is finished and free the peripherals for the next transmission
        let (dma_channel, dma_buffer, spi) = spi_dma_transfer.wait();
        dma_ch_opt.replace(dma_channel);
        spi_opt.replace(spi);
        dma_buffer_opt.replace(dma_buffer);
    }
}

#[interrupt]
fn TIMER_IRQ_0() {
    TRANSMIT_NEXT_FRAME.store(1, Ordering::Relaxed);
    cortex_m::interrupt::free(|cs| {
        ALARM0.borrow(cs).borrow_mut().as_mut().unwrap().clear_interrupt();
    });
}
