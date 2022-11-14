//#![no_std]
#![feature(unchecked_math)]
#![feature(stdsimd)]
#![feature(error_in_core)]
#![feature(core_intrinsics)]
#![warn(dead_code)]

extern crate alloc;
extern crate core;

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::error::Error;
use core::fmt::Display;

mod fixed;

mod utils;
use utils::ButterworthFilter;

mod symbol;
pub use symbol::Symbol;

mod module;
pub use module::Module;

mod row;
pub use row::Row;

mod pattern;
pub use pattern::Pattern;

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
pub use binary_reader::BinaryReader;

mod binary_writer;
pub use binary_writer::BinaryWriter;

mod platform;
pub use platform::DummyInterface;
pub use platform::PlatformInterface;

mod packed_module;
pub use packed_module::PackedModule;
pub use packed_module::PackingParams;

///////////////////////////////////////////////////////////////////////////////

mod math {
    use micromath::F32Ext;

    pub fn fract(value: f32) -> f32 {
        value.fract()
    }

    pub fn floor(value: f32) -> f32 {
        value.floor()
    }

    pub fn pow(value: f32, exponent: f32) -> f32 {
        value.powf(exponent)
    }

    pub fn sin(value: f32) -> f32 {
        value.sin()
    }

    pub fn cos(value: f32) -> f32 {
        value.cos()
    }

    pub fn sqrt(value: f32) -> f32 {
        value.sqrt()
    }

    pub fn exp(value: f32) -> f32 {
        value.exp()
    }

    pub fn sign_u8(value: u8) -> u8 {
        if value == 0 {
            0
        } else {
            1
        }
    }
}

type Fixed = fixed::FixedU32x;

#[derive(Debug)]
pub struct FormatError {
    details: String,
}

impl FormatError {
    pub fn new(_details: &str) -> FormatError {
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
    fn fmt(&self, _: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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

///////////////////////////////////////////////////////////////////////////////

#[repr(C, packed)]
pub struct SampleDesc {
    //
}

#[repr(C, packed)]
pub struct InstrumentDesc {
    //
}
