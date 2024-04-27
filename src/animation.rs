use crate::display::ObegraensadDisplay;

use fugit::MicrosDurationU32;

pub trait Animation {
    fn render_frame(&mut self, display: &mut ObegraensadDisplay) -> MicrosDurationU32;
}
