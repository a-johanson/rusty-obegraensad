use crate::display::ObegraensadDisplay;

/// Board-specific output for the OBEGRÄNSAD serial LED-driver chain.
///
/// Core animation code only produces an `ObegraensadDisplay` frame. Board crates
/// own the concrete transport, latch timing, and enable pin polarity.
pub trait DisplayDriver {
    type Error;

    fn write_frame(&mut self, display: &ObegraensadDisplay) -> Result<(), Self::Error>;

    fn latch(&mut self) -> Result<(), Self::Error>;

    fn set_enabled(&mut self, enabled: bool) -> Result<(), Self::Error>;
}

/// Board-specific user input used to select the active animation.
pub trait AnimationSelect {
    type Error;

    fn is_selected(&mut self) -> Result<bool, Self::Error>;
}
