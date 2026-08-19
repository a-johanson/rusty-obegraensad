use crate::animation::Animation;
use crate::display;
use display::ObegraensadDisplay;

use fugit::MicrosDurationU32;

pub struct EmptyAnimation {}

impl EmptyAnimation {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for EmptyAnimation {
    fn default() -> Self {
        Self::new()
    }
}

impl Animation for EmptyAnimation {
    fn render_frame(&mut self, display: &mut ObegraensadDisplay) -> MicrosDurationU32 {
        display.clear();
        MicrosDurationU32::millis(500)
    }
}
