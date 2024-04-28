use crate::animation::Animation;
use crate::display::ObegraensadDisplay;

use rand::{RngCore, SeedableRng};
use rand_xoshiro::Xoroshiro128StarStar;
use fugit::MicrosDurationU32;

#[derive(Clone, Copy)]
struct Drop {
    x: u8,
    y: u8,
    r: u8,
}

impl Drop {
    const fn new() -> Self {
        Self {
            x: 0,
            y: 0,
            r: 0xFF,
        }
    }

    fn is_active(&self) -> bool {
        self.r < 5
    }

    fn init(&mut self, r: u32) {
        self.x = (r & 0xF) as u8;
        self.y = ((r >> 4) & 0xF) as u8;
        self.r = 0;
    }

    fn step(&mut self) {
        self.r += 1;
    }

    fn draw(&self, display: &mut ObegraensadDisplay) {
        // A more general approach would be to store the slow-changing axis' increments in only one octant and to exploit symmetry
        if self.r == 0 {
            display.set_pixel(self.x, self.y);
        } else if self.r == 1 {
            display.set_pixel(self.x + 1, self.y);
            display.set_pixel(self.x - 1, self.y);
            display.set_pixel(self.x, self.y + 1);
            display.set_pixel(self.x, self.y - 1);
        }
        else if self.r == 2 {
            display.set_pixel(self.x - 1, self.y + 2);
            display.set_pixel(self.x    , self.y + 2);
            display.set_pixel(self.x + 1, self.y + 2);
            display.set_pixel(self.x - 1, self.y - 2);
            display.set_pixel(self.x    , self.y - 2);
            display.set_pixel(self.x + 1, self.y - 2);
            display.set_pixel(self.x + 2, self.y - 1);
            display.set_pixel(self.x + 2, self.y);
            display.set_pixel(self.x + 2, self.y + 1);
            display.set_pixel(self.x - 2, self.y - 1);
            display.set_pixel(self.x - 2, self.y);
            display.set_pixel(self.x - 2, self.y + 1);
        }
        else if self.r == 3 {
            display.set_pixel(self.x - 1, self.y + 3);
            display.set_pixel(self.x    , self.y + 3);
            display.set_pixel(self.x + 1, self.y + 3);
            display.set_pixel(self.x - 1, self.y - 3);
            display.set_pixel(self.x    , self.y - 3);
            display.set_pixel(self.x + 1, self.y - 3);
            display.set_pixel(self.x + 3, self.y - 1);
            display.set_pixel(self.x + 3, self.y);
            display.set_pixel(self.x + 3, self.y + 1);
            display.set_pixel(self.x - 3, self.y - 1);
            display.set_pixel(self.x - 3, self.y);
            display.set_pixel(self.x - 3, self.y + 1);

            display.set_pixel(self.x + 2, self.y + 2);
            display.set_pixel(self.x + 2, self.y - 2);
            display.set_pixel(self.x - 2, self.y + 2);
            display.set_pixel(self.x - 2, self.y - 2);
        }
        else if self.r == 4 {
            display.set_pixel(self.x - 2, self.y + 4);
            display.set_pixel(self.x - 1, self.y + 4);
            display.set_pixel(self.x    , self.y + 4);
            display.set_pixel(self.x + 1, self.y + 4);
            display.set_pixel(self.x + 2, self.y + 4);

            display.set_pixel(self.x - 2, self.y - 4);
            display.set_pixel(self.x - 1, self.y - 4);
            display.set_pixel(self.x    , self.y - 4);
            display.set_pixel(self.x + 1, self.y - 4);
            display.set_pixel(self.x + 2, self.y - 4);

            display.set_pixel(self.x + 4, self.y - 2);
            display.set_pixel(self.x + 4, self.y - 1);
            display.set_pixel(self.x + 4, self.y);
            display.set_pixel(self.x + 4, self.y + 1);
            display.set_pixel(self.x + 4, self.y + 2);

            display.set_pixel(self.x - 4, self.y - 2);
            display.set_pixel(self.x - 4, self.y - 1);
            display.set_pixel(self.x - 4, self.y);
            display.set_pixel(self.x - 4, self.y + 1);
            display.set_pixel(self.x - 4, self.y + 2);

            display.set_pixel(self.x + 3, self.y + 3);
            display.set_pixel(self.x + 3, self.y - 3);
            display.set_pixel(self.x - 3, self.y + 3);
            display.set_pixel(self.x - 3, self.y - 3);
        }
    }
}

const MAX_DROPS: usize = 6;

pub struct DropAnimation {
    rng: Xoroshiro128StarStar,
    drops: [Drop; MAX_DROPS],
}

impl DropAnimation {
    pub fn new() -> Self {
        Self {
            rng: Xoroshiro128StarStar::seed_from_u64(0xC063_7BB4_8326_CD62),
            drops: [Drop::new(); MAX_DROPS]
        }
    }
}

impl Animation for DropAnimation {
    fn render_frame(&mut self, display: &mut ObegraensadDisplay) -> MicrosDurationU32 {
        // Step all existing drops
        for drop in self.drops.iter_mut() {
            if drop.is_active() {
                drop.step();
            }
        }

        // Spawn new drop in 1/2 of cases
        if (self.rng.next_u32() & 0b1) == 0 {
            for drop in self.drops.iter_mut() {
                if !drop.is_active() {
                    drop.init(self.rng.next_u32());
                    break;
                }
            }
        }

        // Draw active drops on display
        display.clear();
        for drop in self.drops.iter() {
            if drop.is_active() {
                drop.draw(display);
            }
        }

        MicrosDurationU32::millis(100)
    }
}