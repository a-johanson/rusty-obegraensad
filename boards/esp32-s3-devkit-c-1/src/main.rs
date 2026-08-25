#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp-hal types"
)]
#![deny(clippy::large_stack_frames)]

use core::convert::Infallible;

use esp_hal::{
    clock::CpuClock,
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    main,
    time::{Duration, Instant},
};
use fugit::MicrosDurationU32;
use obegraensad_core::{
    Animation, BYTE_COUNT, EmptyAnimation, FallingLeaves, ObegraensadDisplay,
    hardware::{AnimationSelect, DisplayDriver},
};

esp_bootloader_esp_idf::esp_app_desc!();

const FRAME_AFTER_ANIMATION_SWITCH: MicrosDurationU32 = MicrosDurationU32::from_millis(30);

struct Esp32S3Display<'d> {
    clock: Output<'d>,
    data: Output<'d>,
    latch: Output<'d>,
    not_enable: Output<'d>,
}

impl<'d> Esp32S3Display<'d> {
    fn new(clock: Output<'d>, data: Output<'d>, latch: Output<'d>, not_enable: Output<'d>) -> Self {
        Self {
            clock,
            data,
            latch,
            not_enable,
        }
    }

    fn write_byte(&mut self, byte: u8) -> Result<(), Infallible> {
        for bit in (0..8).rev() {
            if (byte & (1 << bit)) == 0 {
                self.data.set_low();
            } else {
                self.data.set_high();
            }

            self.clock.set_high();
            self.clock.set_low();
        }

        Ok(())
    }
}

impl DisplayDriver for Esp32S3Display<'_> {
    type Error = Infallible;

    fn write_frame(&mut self, display: &ObegraensadDisplay) -> Result<(), Self::Error> {
        let mut buffer = [0; BYTE_COUNT];
        display.to_output_buffer(&mut buffer);

        for byte in buffer {
            self.write_byte(byte)?;
        }

        Ok(())
    }

    fn latch(&mut self) -> Result<(), Self::Error> {
        self.latch.set_high();
        self.latch.set_low();
        Ok(())
    }

    fn set_enabled(&mut self, enabled: bool) -> Result<(), Self::Error> {
        if enabled {
            self.not_enable.set_low();
        } else {
            self.not_enable.set_high();
        }

        Ok(())
    }
}

struct ActiveLowButton<'d> {
    input: Input<'d>,
}

impl<'d> ActiveLowButton<'d> {
    fn new(input: Input<'d>) -> Self {
        Self { input }
    }
}

impl AnimationSelect for ActiveLowButton<'_> {
    type Error = Infallible;

    fn is_selected(&mut self) -> Result<bool, Self::Error> {
        Ok(self.input.is_low())
    }
}

fn delay_micros(us: u32) {
    let delay_start = Instant::now();
    while delay_start.elapsed() < Duration::from_micros(us as u64) {}
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[allow(
    clippy::large_stack_frames,
    reason = "the firmware keeps the display buffer and animation state on the stack"
)]
#[main]
fn main() -> ! {
    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);

    let output_config = OutputConfig::default();
    let mut display_driver = Esp32S3Display::new(
        Output::new(peripherals.GPIO12, Level::Low, output_config),
        Output::new(peripherals.GPIO11, Level::Low, output_config),
        Output::new(peripherals.GPIO10, Level::Low, output_config),
        Output::new(peripherals.GPIO9, Level::High, output_config),
    );
    let mut animation_select = ActiveLowButton::new(Input::new(
        peripherals.GPIO4,
        InputConfig::default().with_pull(Pull::Up),
    ));

    let mut display = ObegraensadDisplay::new();
    let mut animation_leaves = FallingLeaves::new();
    let mut animation_empty = EmptyAnimation::new();
    const ANIMATION_COUNT: usize = 2;
    let animations: [&mut dyn Animation; ANIMATION_COUNT] =
        [&mut animation_leaves, &mut animation_empty];
    let mut current_animation_index = 0;
    let mut current_frame_duration = MicrosDurationU32::from_millis(10);

    display_driver.write_frame(&display).unwrap();
    display_driver.latch().unwrap();
    display_driver.set_enabled(true).unwrap();

    loop {
        if animation_select.is_selected().unwrap() {
            while animation_select.is_selected().unwrap() {
                delay_micros(20_000);
            }

            current_animation_index += 1;
            if current_animation_index >= ANIMATION_COUNT {
                current_animation_index = 0;
            }

            display.clear();
            current_frame_duration = FRAME_AFTER_ANIMATION_SWITCH;
        }

        display_driver.write_frame(&display).unwrap();
        display_driver.latch().unwrap();

        let next_frame_duration = animations[current_animation_index].render_frame(&mut display);
        delay_micros(current_frame_duration.as_micros());
        current_frame_duration = next_frame_duration;
    }
}
