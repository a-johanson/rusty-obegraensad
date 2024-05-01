#![no_std]
#![no_main]

mod animation;
mod animation_leaves;
mod display;
mod global_state;

use cortex_m::singleton;
use embedded_hal::digital::{InputPin, OutputPin}; // General Hardware Abstraction Layer (HAL) for embedded systems (https://github.com/rust-embedded/embedded-hal)
use panic_halt as _;
use rp_pico::entry; // rp_pico = Board Support Package (BSP; https://github.com/rp-rs/rp-hal-boards/)
use rp_pico::hal; // Hardware Abstraction Layer (HAL) for Raspberry Silicon (higher-level drivers; https://github.com/rp-rs/rp-hal/)
use rp_pico::hal::dma::{single_buffer, DMAExt};
use rp_pico::hal::gpio::{FunctionSpi, PinState};
use rp_pico::hal::pac; // Peripheral Access Crate (PAC; low-level register access; https://github.com/rp-rs/rp2040-pac)
use rp_pico::hal::pac::interrupt;
use rp_pico::hal::timer::Alarm;
use rp_pico::hal::Clock;

use fugit::{MicrosDurationU32, RateExtU32};
use portable_atomic::Ordering;

use animation::Animation;

// TODO: look at https://github.com/knurling-rs/flip-link

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
    )
    .unwrap();

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
    let mut pin_button = pins.gpio22.into_pull_up_input();
    let mut pin_led = pins.led.into_push_pull_output_in_state(PinState::High);
    let mut pin_latch = pins.gpio21.into_push_pull_output_in_state(PinState::Low);

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

    // Transmit an empty buffer
    let dma_buffer = singleton!(: [u8; display::BYTE_COUNT] = [0; display::BYTE_COUNT]).unwrap();
    let dma_spi_transfer = single_buffer::Config::new(dma_channel, dma_buffer, spi).start();

    let core = pac::CorePeripherals::take().unwrap();
    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());
    // Delaying here for a bit helps with spurious button presses at startup since the cap connected to
    // the button pin on the PCB is only slowly charged via the internal pull-up resistor
    delay.delay_ms(10);

    // Latch the (now empty) display content
    pin_latch.set_high().unwrap();

    let mut timer = hal::Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);
    let mut alarm0 = timer.alarm_0().unwrap();
    alarm0.enable_interrupt();
    alarm0.schedule(MicrosDurationU32::millis(10)).unwrap();

    // Finish latch pulse and enable the display
    cortex_m::asm::nop();
    cortex_m::asm::nop();
    pin_latch.set_low().unwrap();
    pin_not_enable.set_low().unwrap();

    // Set up global shared state
    cortex_m::interrupt::free(|cs| {
        global_state::SHARED_STATE
            .borrow(cs)
            .replace(Some(global_state::SharedState { alarm0 }));
    });

    unsafe {
        pac::NVIC::unmask(pac::Interrupt::TIMER_IRQ_0);
    }

    let mut display = display::ObegraensadDisplay::new();
    let mut animation_leaves = animation_leaves::FallingLeaves::new();
    let mut current_frame_duration = MicrosDurationU32::millis(10);
    let mut dma_spi_transfer = Some(dma_spi_transfer);
    loop {
        // If the button is pressed...
        if pin_button.is_low().unwrap() {
            // freeze the animation until the button is released
            loop {
                delay.delay_ms(20);
                if pin_button.is_high().unwrap() {
                    break;
                }
            }

            // re-schedule the alarm
            cortex_m::interrupt::free(|cs| {
                global_state::SHARED_STATE
                    .borrow(cs)
                    .borrow_mut()
                    .as_mut()
                    .map(|s| s.alarm0_schedule(current_frame_duration))
                    .unwrap();
            });
            global_state::ATOMIC_STATE
                .transmit_next_frame
                .store(0, Ordering::Relaxed);
        }

        // Start to transmit the display content (current frame) via SPI fed via DMA
        let (dma_channel, dma_buffer, spi) = dma_spi_transfer.take().unwrap().wait();
        display.to_output_buffer(dma_buffer);
        dma_spi_transfer.replace(single_buffer::Config::new(dma_channel, dma_buffer, spi).start());

        // Compute the next frame
        let next_frame_duration = animation_leaves.render_frame(&mut display);

        // Disable the activity LED and sleep until it's time to show the next frame
        pin_led.set_low().unwrap();
        while global_state::ATOMIC_STATE
            .transmit_next_frame
            .load(Ordering::Relaxed)
            == 0
        {
            cortex_m::asm::wfi();
        }

        // Start pulsing the latch pin to show the current frame
        pin_latch.set_high().unwrap();

        // Reset frame transmission status and re-schedule the timer to determine how long the current frame should be shown
        global_state::ATOMIC_STATE
            .transmit_next_frame
            .store(0, Ordering::Relaxed);
        cortex_m::interrupt::free(|cs| {
            global_state::SHARED_STATE
                .borrow(cs)
                .borrow_mut()
                .as_mut()
                .map(|s| s.alarm0_schedule(current_frame_duration))
                .unwrap();
        });

        // Enable the activity LED
        pin_led.set_high().unwrap();

        // Ensure that upon the next transmission cycle, we display the next frame for the designated amount of time
        current_frame_duration = next_frame_duration;

        // Finish the latch pulse
        cortex_m::asm::nop();
        cortex_m::asm::nop();
        pin_latch.set_low().unwrap();
    }
}

#[interrupt]
fn TIMER_IRQ_0() {
    global_state::ATOMIC_STATE
        .transmit_next_frame
        .store(1, Ordering::Relaxed);
    cortex_m::interrupt::free(|cs| {
        global_state::SHARED_STATE
            .borrow(cs)
            .borrow_mut()
            .as_mut()
            .map(|s| s.alarm0_clear_interrupt())
            .unwrap();
    });
}
