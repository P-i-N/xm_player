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
use std::intrinsics::size_of;

mod fixed;

mod utils;
use utils::ButterworthFilter;

mod symbol;
pub use symbol::Symbol;
pub use symbol::SymbolEncodingSize;
pub use symbol::SymbolPrefixBits;

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
    pub data_offset: u32,
    pub data_length: u32,
    pub is_16bit: u8,
    pub loop_start: u32,
    pub loop_end: u32,
    pub volume: u8,
    pub panning: u8,
    pub relative_note: i8,
    pub finetune: i8,
}

#[repr(C, packed)]
pub struct InstrumentDesc {
    pub sample_keymap: [u16; 96],
}

#[repr(C, packed)]
pub struct ChannelDesc {
    pub data_offset: u32,
}

#[repr(C, packed)]
pub struct ModuleDesc<'a> {
    pub data: &'a [u8],
    pub samples: &'a [SampleDesc],
    pub instruments: &'a [InstrumentDesc],
    pub channels: &'a [ChannelDesc],
}

impl<'a> ModuleDesc<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, Box<dyn Error>> {
        let mut br = BinaryReader::new(data);

        br.pos = data.len() - 4; // Seek back to last 4 bytes containing ending header offset
        let header_offset = br.read_u32() as usize;
        br.pos = header_offset;

        let num_instruments = br.read_u32() as usize;
        let (_, instruments, _) = unsafe {
            data[0..size_of::<InstrumentDesc>() * num_instruments].align_to::<InstrumentDesc>()
        };

        Ok(ModuleDesc {
            data,
            samples: &[],
            instruments,
            channels: &[],
        })
    }
}
