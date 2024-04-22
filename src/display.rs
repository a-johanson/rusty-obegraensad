
use embedded_hal::digital::{OutputPin, PinState};

const DISPLAY_SIZE: usize = 16;
const BYTE_COUNT: usize = 2 * DISPLAY_SIZE;

pub struct ObegraensadDisplay<CLK: OutputPin, DI: OutputPin, LA: OutputPin> {
    pixels: [u8; BYTE_COUNT],
    pin_clock: CLK,
    pin_data: DI,
    pin_latch: LA,
    delay: cortex_m::delay::Delay,
}

impl<CLK: OutputPin, DI: OutputPin, LA: OutputPin> ObegraensadDisplay<CLK, DI, LA> {
    pub fn new(pin_clock: CLK, pin_data: DI, pin_latch: LA, delay: cortex_m::delay::Delay) -> Self {
        Self {
            pixels: [0; BYTE_COUNT],
            pin_clock,
            pin_data,
            pin_latch,
            delay,
        }
    }

    pub fn clear(&mut self) {
        self.pixels.fill(0);
    }

    pub fn set_pixel(&mut self, x: u8, y: u8) {
        if x >= DISPLAY_SIZE as u8 || y >= DISPLAY_SIZE as u8 {
            return;
        }
        let shift_reg_index = (y & 0b1111_1110) + ((if y & 0b0000_0010 == 0 { 15 - x } else { x }) >> 3);
        let (byte_index, bit_index) = if y & 0x01 == 0 {
            (shift_reg_index << 1, x & 0b0000_0111)
        } else {
            ((shift_reg_index << 1) + 1, 7 - (x & 0b0000_0111))
        };
        self.pixels[byte_index as usize] |= 1 << bit_index;
    }

    pub fn show(&mut self) {
        for byte in self.pixels {
            self.output_byte(byte);
        }
        self.pin_latch.set_high().unwrap();
        // Delay for at least 20 ns
        self.delay.delay_us(10);
        self.pin_latch.set_low().unwrap();
    }

    fn output_byte(&mut self, mut byte: u8) { // Replace bit banging with SPI peripheral in the future
        for _ in 0..8 {
            let value = byte & 0b1000_0000 != 0;
            self.pin_data.set_state(match value {
                true => PinState::High,
                _ => PinState::Low,
            }).unwrap();
            // Delay for at least 5 ns
            self.delay.delay_us(5);
            self.pin_clock.set_high().unwrap();
            // Delay for at least 20 n
            self.delay.delay_us(5);
            self.pin_clock.set_low().unwrap();
            byte = byte << 1;
        }
    }
}

