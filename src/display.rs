
const DISPLAY_SIZE: usize = 16;
const BYTE_COUNT: usize = 2 * DISPLAY_SIZE;

pub struct ObegraensadDisplay {
    pixels: [u8; BYTE_COUNT]
}

impl ObegraensadDisplay {
    pub fn new() -> Self {
        Self {
            pixels: [0; BYTE_COUNT]
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

    pub fn to_output_buffer(&self, buffer: &mut [u8; BYTE_COUNT]) {
        buffer.copy_from_slice(&self.pixels);
    }
}

