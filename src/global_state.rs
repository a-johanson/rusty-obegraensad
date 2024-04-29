use core::cell::RefCell;

use rp_pico::hal;
use hal::pac;
use hal::timer::{Alarm, Alarm0, Alarm1};
use hal::gpio::FunctionSpi;
use hal::dma::single_buffer;
use embedded_hal::digital::OutputPin;
use cortex_m::interrupt::Mutex;

use fugit::MicrosDurationU32;
use portable_atomic::AtomicU8;

use crate::display;


pub static SHARED_STATE: Mutex<RefCell<Option<SharedState>>> = Mutex::new(RefCell::new(None));
pub static ATOMIC_STATE: AtomicState = AtomicState::new();

pub struct AtomicState {
    pub show_next_frame: AtomicU8,
}

impl AtomicState {
    pub const fn new() -> Self {
        Self {
            show_next_frame: AtomicU8::new(0),
        }
    }
}

type DmaSpiTransfer = single_buffer::Transfer<hal::dma::Channel<hal::dma::CH0>, &'static mut [u8; display::BYTE_COUNT], hal::Spi<hal::spi::Enabled, pac::SPI0, (hal::gpio::Pin<hal::gpio::bank0::Gpio19, FunctionSpi, hal::gpio::PullDown>, hal::gpio::Pin<hal::gpio::bank0::Gpio18, FunctionSpi, hal::gpio::PullDown>)>>;
type LatchPin = hal::gpio::Pin<hal::gpio::bank0::Gpio21, hal::gpio::FunctionSio<hal::gpio::SioOutput>, hal::gpio::PullDown>;

pub struct SharedState {
    pub alarm0: Alarm0,
    pub alarm1: Alarm1,
    pub layer_buffer: &'static mut [u8; display::LAYER_COUNT * display::BYTE_COUNT],
    pub dma_spi_transfer: Option<DmaSpiTransfer>,
    pub pin_latch: LatchPin,
}

impl SharedState {
    pub fn pin_latch_high(&mut self) {
        self.pin_latch.set_high().unwrap();
    }

    pub fn pin_latch_low(&mut self) {
        self.pin_latch.set_low().unwrap();
    }

    pub fn alarm0_schedule(&mut self, duration: MicrosDurationU32) {
        self.alarm0.schedule(duration).unwrap();
    }

    pub fn alarm0_clear_interrupt(&mut self) {
        self.alarm0.clear_interrupt();
    }

    pub fn alarm1_schedule(&mut self, duration: MicrosDurationU32) {
        self.alarm1.schedule(duration).unwrap();
    }

    pub fn alarm1_clear_interrupt(&mut self) {
        self.alarm1.clear_interrupt();
    }

    pub fn dma_spi_transfer_take(&mut self) -> DmaSpiTransfer {
        self.dma_spi_transfer.take().unwrap()
    }

    pub fn dma_spi_transfer_replace(&mut self, transfer: DmaSpiTransfer) {
        self.dma_spi_transfer.replace(transfer);
    }
}
