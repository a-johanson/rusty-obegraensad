use core::convert::Infallible;

use embedded_hal::digital::{InputPin, OutputPin};
use obegraensad_core::{
    hardware::{AnimationSelect, DisplayDriver},
    ObegraensadDisplay,
};

/// Pico-specific adapter for the display transport and control pins.
///
/// The frame writer is supplied by `main` because it owns the concrete RP2040
/// DMA transfer type. Keeping it behind this adapter lets the firmware use the
/// portable `DisplayDriver` boundary without coupling core to the RP2040 HAL.
pub struct PicoDisplay<FrameWriter, Latch, NotEnable> {
    frame_writer: FrameWriter,
    latch: Latch,
    not_enable: NotEnable,
}

impl<FrameWriter, Latch, NotEnable> PicoDisplay<FrameWriter, Latch, NotEnable> {
    pub fn new(frame_writer: FrameWriter, latch: Latch, not_enable: NotEnable) -> Self {
        Self {
            frame_writer,
            latch,
            not_enable,
        }
    }
}

impl<FrameWriter, Latch, NotEnable> DisplayDriver for PicoDisplay<FrameWriter, Latch, NotEnable>
where
    FrameWriter: FnMut(&ObegraensadDisplay) -> Result<(), Infallible>,
    Latch: OutputPin<Error = Infallible>,
    NotEnable: OutputPin<Error = Infallible>,
{
    type Error = Infallible;

    fn write_frame(&mut self, display: &ObegraensadDisplay) -> Result<(), Self::Error> {
        (self.frame_writer)(display)
    }

    fn latch(&mut self) -> Result<(), Self::Error> {
        self.latch.set_high()?;
        // The SCT2024 requires a latch pulse of at least 20 ns. The Pico runs
        // at 125 MHz (8 ns/cycle), so keep the pin high for four full cycles
        // (32 ns) rather than relying on surrounding instructions for timing.
        cortex_m::asm::nop();
        cortex_m::asm::nop();
        cortex_m::asm::nop();
        cortex_m::asm::nop();
        self.latch.set_low()
    }

    fn set_enabled(&mut self, enabled: bool) -> Result<(), Self::Error> {
        if enabled {
            self.not_enable.set_low()
        } else {
            self.not_enable.set_high()
        }
    }
}

/// Active-low Pico input implementing core's portable animation selector.
pub struct ActiveLowButton<Input> {
    input: Input,
}

impl<Input> ActiveLowButton<Input> {
    pub fn new(input: Input) -> Self {
        Self { input }
    }
}

impl<Input> AnimationSelect for ActiveLowButton<Input>
where
    Input: InputPin<Error = Infallible>,
{
    type Error = Infallible;

    fn is_selected(&mut self) -> Result<bool, Self::Error> {
        self.input.is_low()
    }
}
