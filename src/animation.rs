use crate::display::ObegraensadDisplay;

use fugit::MicrosDurationU32;

pub trait Animation {
    /// Renders the next frame of the animation to the display and returns the duration this frame should be displayed for.
    /// 
    /// When implementing this method, you typically want to use `display.clear()` to erase the current contents of the display and then draw your frame using `display.set_pixel()`.
    fn render_frame(&mut self, display: &mut ObegraensadDisplay) -> MicrosDurationU32;
}
