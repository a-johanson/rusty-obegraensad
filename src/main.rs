#![no_std]
#![no_main]

mod animation;
mod animation_leaves;
mod display;
mod global_state;

use rp_pico::entry; // rp_pico = Board Support Package (BSP; https://github.com/rp-rs/rp-hal-boards/)
use panic_halt as _;
use rp_pico::hal; // Hardware Abstraction Layer for Raspberry Silicon (higher-level drivers; https://github.com/rp-rs/rp-hal/)
use rp_pico::hal::pac; // Peripheral Access Crate (low-level register access; https://github.com/rp-rs/rp2040-pac)
use rp_pico::hal::pac::interrupt;
use rp_pico::hal::timer::Alarm;
use rp_pico::hal::gpio::{FunctionSpi, PinState};
use rp_pico::hal::dma::{single_buffer, DMAExt};
use rp_pico::hal::Clock;
use embedded_hal::digital::{InputPin, OutputPin}; // General Hardware Abstraction Layer for embedded systems (https://github.com/rust-embedded/embedded-hal)
use cortex_m::singleton;

use portable_atomic::Ordering;
use fugit::{MicrosDurationU32, RateExtU32};

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
    let mut pin_button = pins.gpio22.into_pull_up_input();
    let mut pin_led = pins.led.into_push_pull_output_in_state(PinState::High);
    let mut pin_latch = pins.gpio21.into_push_pull_output_in_state(PinState::Low);

    let pin_clock = pins.gpio18.into_function::<FunctionSpi>();
    let pin_data = pins.gpio19.into_function::<FunctionSpi>();
    let spi = hal::spi::Spi::<_, _, _, 8>::new(pac.SPI0, (pin_data, pin_clock));
    let spi = spi.init(
        &mut pac.RESETS,
        clocks.peripheral_clock.freq(),
        8.MHz(),
        embedded_hal::spi::MODE_0,
    );

    let dma = pac.DMA.split(&mut pac.RESETS);
    let dma_channel = dma.ch0;

    // transmit an empty buffer
    let dma_buffer = singleton!(: [u8; display::BYTE_COUNT] = [0; display::BYTE_COUNT]).unwrap();
    let dma_spi_transfer = single_buffer::Config::new(dma_channel, dma_buffer, spi).start();

    let core = pac::CorePeripherals::take().unwrap();
    let mut delay = cortex_m::delay::Delay::new(core.SYST, clocks.system_clock.freq().to_Hz());
    // Delaying here for a bit helps with spurious button presses at startup since the cap connected to
    // the button pin on the PCB is only slowly charged via the internal pull-up resistor
    delay.delay_ms(10);

    // Latch the display content
    pin_latch.set_high().unwrap();

    let layer_buffer = singleton!(: [u8; display::LAYER_COUNT * display::BYTE_COUNT] = [0; display::LAYER_COUNT * display::BYTE_COUNT]).unwrap();

    let mut timer = hal::Timer::new(pac.TIMER, &mut pac.RESETS, &clocks);
    let mut alarm0 = timer.alarm_0().unwrap();
    alarm0.enable_interrupt();
    alarm0.schedule(MicrosDurationU32::millis(10)).unwrap();
    let mut alarm1 = timer.alarm_1().unwrap();
    alarm1.enable_interrupt();
    alarm1.schedule(MicrosDurationU32::millis(1)).unwrap();

    // Finish latch pulse and enable the display
    pin_latch.set_low().unwrap();
    pin_not_enable.set_low().unwrap();

    // setup global shared state
    cortex_m::interrupt::free(|cs| {
        global_state::SHARED_STATE.borrow(cs).replace(Some(global_state::SharedState {
            alarm0,
            alarm1,
            layer_buffer,
            dma_spi_transfer: Some(dma_spi_transfer),
            pin_latch
        }));
    });

    unsafe {
        pac::NVIC::unmask(pac::Interrupt::TIMER_IRQ_0);
        pac::NVIC::unmask(pac::Interrupt::TIMER_IRQ_1);
    }

    let mut display = display::ObegraensadDisplay::new();
    let mut current_animation: u8 = 0;
    const ANIMATION_COUNT: u8 = 1;
    let mut animation_leaves = animation_leaves::FallingLeaves::new();
    let mut current_frame_duration = MicrosDurationU32::millis(10);
    loop {
        // if the button is pressed...
        if pin_button.is_low().unwrap() {
            // clear the display, set a short frame duration, change animation index
            display.clear();
            current_frame_duration = MicrosDurationU32::millis(10);
            current_animation = (current_animation + 1) % ANIMATION_COUNT;

            // wait until the button is released
            loop {
                delay.delay_ms(20);
                if pin_button.is_high().unwrap() {
                    break;
                }
            }

            // re-schedule the alarm
            cortex_m::interrupt::free(|cs| {
                global_state::SHARED_STATE.borrow(cs).borrow_mut().as_mut().map(|s| s.alarm0_schedule(current_frame_duration)).unwrap();
            });
            global_state::ATOMIC_STATE.show_next_frame.store(0, Ordering::Relaxed);
        }

        // transfer display content to global layer buffer
        cortex_m::interrupt::free(|cs| {
            display.to_layer_buffer(global_state::SHARED_STATE.borrow(cs).borrow_mut().as_mut().map(|s| &mut s.layer_buffer).unwrap());
        });

        // compute next frame
        let next_frame_duration = match current_animation {
            _ => animation_leaves.render_frame(&mut display)
        };

        // Disable the activity LED and sleep until it's time to show the next frame
        pin_led.set_low().unwrap();
        while global_state::ATOMIC_STATE.show_next_frame.load(Ordering::Relaxed) == 0 {
            cortex_m::asm::wfi();
        }

        // Reset frame transmission status and re-schedule the timer to determine how long the current frame should be shown
        global_state::ATOMIC_STATE.show_next_frame.store(0, Ordering::Relaxed);
        cortex_m::interrupt::free(|cs| {
            global_state::SHARED_STATE.borrow(cs).borrow_mut().as_mut().map(|s| s.alarm0_schedule(current_frame_duration)).unwrap();
        });

        // Enable the activity LED
        pin_led.set_high().unwrap();

        // Ensure that upon the next display cycle, we display the next frame for the designated amount of time
        current_frame_duration = next_frame_duration;
    }
}

#[interrupt]
fn TIMER_IRQ_0() {
    global_state::ATOMIC_STATE.show_next_frame.store(1, Ordering::Relaxed);
    cortex_m::interrupt::free(|cs| {
        global_state::SHARED_STATE.borrow(cs).borrow_mut().as_mut().map(|s| s.alarm0_clear_interrupt()).unwrap();
    });
}

#[interrupt]
fn TIMER_IRQ_1() {
    static mut LAYER_INDEX: usize = 0;

    cortex_m::interrupt::free(|cs| {
        let mut state = global_state::SHARED_STATE.borrow(cs).borrow_mut();

        // Start latch pulse
        state.as_mut().map(|s| s.pin_latch_high()).unwrap();

        // Re-schedule Alarm1
        state.as_mut().map(|s| s.alarm1_schedule(match *LAYER_INDEX {
            // total time for 2 kHZ PWM frequency is 500 micros
            0 => MicrosDurationU32::micros(289), // we are transmitting layer 0 and display layer 2 for this duration (should be the rest of the total time)
            1 => MicrosDurationU32::micros(48), // we are transmitting layer 1 and display layer 0 for this duration (should be 9.6% of the total time)
            _ => MicrosDurationU32::micros(163), // we are transmitting layer 2 and display layer 1 for this duration (should be roughly 42.2% - 9.6% = 32.6% of the total time)
        })).unwrap();
        state.as_mut().map(|s| s.alarm1_clear_interrupt()).unwrap();

        // Finish DMA transfer
        let (dma_channel, dma_buffer, spi) = state.as_mut().map(|s| s.dma_spi_transfer_take()).unwrap().wait();

        // Copy next layer to dma_buffer
        let layer_buffer = state.as_mut().map(|s| &mut s.layer_buffer).unwrap();
        let start_index = *LAYER_INDEX * display::BYTE_COUNT;
        dma_buffer.copy_from_slice(&layer_buffer[start_index..(start_index + display::BYTE_COUNT)]);

        // Finish latch pulse
        state.as_mut().map(|s| s.pin_latch_low()).unwrap();

        // Start next DMA transfer
        state.as_mut().map(|s: &mut global_state::SharedState| s.dma_spi_transfer_replace(single_buffer::Config::new(dma_channel, dma_buffer, spi).start())).unwrap();
    });

    *LAYER_INDEX += 1;
    if *LAYER_INDEX >= display::LAYER_COUNT {
        *LAYER_INDEX = 0;
    }
}
