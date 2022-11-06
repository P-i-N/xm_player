#![no_std]
#![feature(unchecked_math)]
#![feature(stdsimd)]
#![feature(error_in_core)]
#![feature(core_intrinsics)]

extern crate alloc;
extern crate core;

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::error::Error;
use core::fmt::Display;
use core::intrinsics::*;
use core::time::Duration;

mod fixed;

mod module;
pub use module::Module;

mod pattern;
pub use pattern::Pattern;
use pattern::Row;

mod envelope;
pub use envelope::Envelope;

mod instrument;
pub use instrument::Instrument;

mod sample;
use sample::LoopType;
use sample::Sample;

mod channel;
pub use channel::Channel;

mod player;
pub use player::Player;

mod binary_reader;
use binary_reader::BinaryReader;

mod platform;
pub use platform::PlatformInterface;

///////////////////////////////////////////////////////////////////////////////

mod math {
    use core::intrinsics::{floorf32, powf32, sinf32};

    pub fn fract(value: f32) -> f32 {
        unsafe { value - floorf32(value) }
    }

    pub fn floor(value: f32) -> f32 {
        unsafe { floorf32(value) }
    }

    pub fn pow(value: f32, exponent: f32) -> f32 {
        unsafe { powf32(value, exponent) }
    }

    pub fn sin(value: f32) -> f32 {
        unsafe { sinf32(value) }
    }
}

type Fixed = fixed::FixedU32x<16>;

#[derive(Debug)]
pub struct FormatError {
    details: String,
}

impl FormatError {
    pub fn new(details: &str) -> FormatError {
        FormatError {
            details: String::new(),
        }
    }
}

impl Error for FormatError {
    fn description(&self) -> &str {
        &self.details
    }
}

impl Display for FormatError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        Ok(())
    }
}

trait BitTest<T> {
    fn test_bitmask(&self, mask: T) -> bool;
}

trait NibbleTest<T> {
    fn test_high_nibble(&self, value: T) -> bool;
}

impl BitTest<u8> for u8 {
    fn test_bitmask(&self, mask: Self) -> bool {
        (self & mask) == mask
    }
}

impl NibbleTest<u8> for u8 {
    fn test_high_nibble(&self, value: Self) -> bool {
        (self & 0xF0) == value
    }
}
