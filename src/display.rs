
const DISPLAY_SIZE: usize = 16;
const BIT_COUNT: usize = DISPLAY_SIZE * DISPLAY_SIZE;
const BYTE_COUNT: usize = BIT_COUNT / 8;

static PIXEL_TO_BIT: [u8; BIT_COUNT] = [
    16, 17, 18, 19, 20, 21, 22, 23, 0, 1, 2, 3, 4, 5, 6, 7,
    31, 30, 29, 28, 27, 26, 25, 24, 15, 14, 13, 12, 11, 10, 9, 8,
    32, 33, 34, 35, 36, 37, 38, 39, 48, 49, 50, 51, 52, 53, 54, 55,
    47, 46, 45, 44, 43, 42, 41, 40, 63, 62, 61, 60, 59, 58, 57, 56,
    80, 81, 82, 83, 84, 85, 86, 87, 64, 65, 66, 67, 68, 69, 70, 71,
    95, 94, 93, 92, 91, 90, 89, 88, 79, 78, 77, 76, 75, 74, 73, 72,
    96, 97, 98, 99, 100, 101, 102, 103, 112, 113, 114, 115, 116, 117, 118, 119,
    111, 110, 109, 108, 107, 106, 105, 104, 127, 126, 125, 124, 123, 122, 121, 120,
    144, 145, 146, 147, 148, 149, 150, 151, 128, 129, 130, 131, 132, 133, 134, 135,
    159, 158, 157, 156, 155, 154, 153, 152, 143, 142, 141, 140, 139, 138, 137, 136,
    160, 161, 162, 163, 164, 165, 166, 167, 176, 177, 178, 179, 180, 181, 182, 183,
    175, 174, 173, 172, 171, 170, 169, 168, 191, 190, 189, 188, 187, 186, 185, 184,
    208, 209, 210, 211, 212, 213, 214, 215, 192, 193, 194, 195, 196, 197, 198, 199,
    223, 222, 221, 220, 219, 218, 217, 216, 207, 206, 205, 204, 203, 202, 201, 200,
    224, 225, 226, 227, 228, 229, 230, 231, 240, 241, 242, 243, 244, 245, 246, 247,
    239, 238, 237, 236, 235, 234, 233, 232, 255, 254, 253, 252, 251, 250, 249, 248,
];

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
        let pixel_index = (y << 4) | x;
        let bit_index = PIXEL_TO_BIT[pixel_index as usize];
        let byte_index = bit_index >> 3;
        let bit_in_byte = bit_index & 0b0000_0111;
        self.pixels[byte_index as usize] |= 1 << bit_in_byte;
    }

    pub fn to_output_buffer(&self, buffer: &mut [u8; BYTE_COUNT]) {
        buffer.copy_from_slice(&self.pixels);
    }
}

