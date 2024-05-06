use crate::animation::Animation;
use crate::display;
use display::ObegraensadDisplay;

use fugit::MicrosDurationU32;
use rand::{RngCore, SeedableRng};
use rand_xoshiro::Xoshiro128StarStar;

#[derive(Clone, Copy)]
struct Leaf {
    x: u8,
    y: u8,
}

impl Leaf {
    const fn new() -> Self {
        Self { x: 0, y: 0xFF }
    }

    fn is_active(&self) -> bool {
        self.y < display::DISPLAY_SIZE as u8
    }

    fn init(&mut self, r: u32) {
        self.x = (r & 0xF) as u8;
        self.y = 0;
    }

    fn step(&mut self, r: u32) {
        let t = (r & 0b111) as u8;
        match t {
            0..=4 => self.x += 1, // 5/8 chance to go right
            5 => self.x -= 1,     // 1/8 chance to go left
            _ => (),              // 2/8 chance to not move horizontally
        }
        self.x &= 0x0F;
        self.y += 1;
    }
}

const MAX_LEAVES: usize = 10;

pub struct FallingLeaves {
    rng: Xoshiro128StarStar,
    leaves: [Leaf; MAX_LEAVES],
}

impl FallingLeaves {
    pub fn new() -> Self {
        Self {
            rng: Xoshiro128StarStar::seed_from_u64(0x9C63_EA21_046B_F751),
            leaves: [Leaf::new(); MAX_LEAVES],
        }
    }
}

impl Animation for FallingLeaves {
    fn render_frame(&mut self, display: &mut ObegraensadDisplay) -> MicrosDurationU32 {
        display.clear();

        // Move and draw all existing leaves
        for leaf in self.leaves.iter_mut() {
            if leaf.is_active() {
                leaf.step(self.rng.next_u32());
                display.set_pixel(leaf.x, leaf.y);
            }
        }

        // Spawn and draw new leaf in 1/2 of cases
        if (self.rng.next_u32() & 0b1) == 0 {
            for leaf in self.leaves.iter_mut() {
                if !leaf.is_active() {
                    leaf.init(self.rng.next_u32());
                    display.set_pixel(leaf.x, leaf.y);
                    break;
                }
            }
        }

        MicrosDurationU32::millis(400)
    }
}
