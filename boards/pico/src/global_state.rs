use core::cell::RefCell;

use cortex_m::interrupt::Mutex;
use hal::timer::{Alarm, Alarm0};
use rp_pico::hal;

use fugit::MicrosDurationU32;
use portable_atomic::AtomicU8;

pub static SHARED_STATE: Mutex<RefCell<Option<SharedState>>> = Mutex::new(RefCell::new(None));
pub static ATOMIC_STATE: AtomicState = AtomicState::new();

pub fn shared_state_interrupt_free<F>(f: F)
where
    F: FnOnce(&mut SharedState),
{
    cortex_m::interrupt::free(|cs| {
        SHARED_STATE
            .borrow(cs)
            .borrow_mut()
            .as_mut()
            .map(f)
            .unwrap();
    });
}

pub struct AtomicState {
    pub transmit_next_frame: AtomicU8,
}

impl AtomicState {
    pub const fn new() -> Self {
        Self {
            transmit_next_frame: AtomicU8::new(0),
        }
    }
}

pub struct SharedState {
    pub alarm0: Alarm0,
}

impl SharedState {
    pub fn alarm0_schedule(&mut self, duration: MicrosDurationU32) {
        self.alarm0.schedule(duration).unwrap();
    }

    pub fn alarm0_clear_interrupt(&mut self) {
        self.alarm0.clear_interrupt();
    }
}
