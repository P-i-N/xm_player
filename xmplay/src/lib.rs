//#![no_std]
#![feature(unchecked_math)]
#![feature(stdsimd)]
#![feature(error_in_core)]
#![feature(core_intrinsics)]
#![feature(hash_drain_filter)]
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

mod entropy;
pub use entropy::RangeEncoder;

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

mod bit_reader;

mod bit_writer;
pub use bit_writer::BitWriter;

mod platform;
pub use platform::DummyInterface;
pub use platform::PlatformInterface;

///////////////////////////////////////////////////////////////////////////////

mod math {
    #[cfg(target = "thumbv7em-none-eabihf")]
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

pub struct SampleFlags;

impl SampleFlags {
    pub const NONE: u32 = 0;
    pub const IS_16_BITS: u32 = 0b_0000_0001;
    pub const PING_PONG: u32 = 0b_0000_0010;
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct SampleDesc {
    pub data_offset: u32,
    pub data_length: u32,
    pub flags: u32,
    pub loop_start: u32,
    pub loop_end: u32,
    pub volume: u8,
    pub panning: u8,
    pub relative_note: i8,
    pub finetune: i8,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct EnvelopeDesc {
    pub data_offset: u32,
    pub data_length: u32,
    pub sustain: u16,
    pub loop_start: u16,
    pub loop_end: u16,
    pub fadeout: u16,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VibratoDesc {
    pub waveform: u8,
    pub sweep: u8,
    pub depth: u8,
    pub rate: u8,
}

#[repr(C, packed)]
pub struct InstrumentDesc {
    pub sample_keymap: [u8; 96],
    pub volume_envelope_index: u8,
    pub panning_envelope_index: u8,
    pub vibrato: VibratoDesc,
}

impl Default for InstrumentDesc {
    fn default() -> Self {
        InstrumentDesc {
            sample_keymap: [0; 96],
            volume_envelope_index: u8::MAX,
            panning_envelope_index: u8::MAX,
            vibrato: VibratoDesc::default(),
        }
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, Debug, Default)]
pub struct ChannelDesc {
    pub data_offset: u32,
    pub data_length: u32,
}

#[repr(C, packed)]
pub struct Range<T> {
    pub min: T,
    pub max: T,
    pub default: T,
}

impl<T: Copy> Range<T> {
    pub fn new(value: T) -> Self {
        Self {
            min: value,
            max: value,
            default: value,
        }
    }

    pub fn set_all(&mut self, value: T) {
        self.min = value;
        self.max = value;
        self.default = value;
    }
}

impl Range<u8> {
    pub fn write(&self, writer: &mut BinaryWriter) {
        writer.write_u8(self.min);
        writer.write_u8(self.max);
        writer.write_u8(self.default);
    }

    pub fn read(reader: &mut BinaryReader) -> Self {
        Self {
            min: reader.read_u8(),
            max: reader.read_u8(),
            default: reader.read_u8(),
        }
    }
}

impl Range<u16> {
    pub fn write(&self, writer: &mut BinaryWriter) {
        writer.write_u16(self.min);
        writer.write_u16(self.max);
        writer.write_u16(self.default);
    }

    pub fn read(reader: &mut BinaryReader) -> Self {
        Self {
            min: reader.read_u16(),
            max: reader.read_u16(),
            default: reader.read_u16(),
        }
    }
}

#[repr(C, packed)]
pub struct ModuleDesc<'a> {
    pub data: &'a [u8],
    pub samples: &'a [SampleDesc],
    pub envelopes: &'a [EnvelopeDesc],
    pub instruments: &'a [InstrumentDesc],
    pub channels: &'a [ChannelDesc],
    pub tempo: Range<u8>,
    pub bpm: Range<u16>,
}

impl<'a> ModuleDesc<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self, Box<dyn Error>> {
        let mut br = BinaryReader::new(data);

        br.pos = data.len() - 20; // Seek back to last 20 bytes containing 5 data offsets

        let first_sample_offset = br.read_u32() as usize;
        let first_envelope_offset = br.read_u32() as usize;
        let first_instrument_offset = br.read_u32() as usize;
        let first_channel_offset = br.read_u32() as usize;
        let module_desc_offset = br.read_u32() as usize;

        // Get samples
        br.pos = first_sample_offset;
        let num_samples = br.read_u8() as usize;
        br.align_to_struct::<SampleDesc>();
        let (_, samples, _) = unsafe {
            data[br.pos..br.pos + size_of::<SampleDesc>() * num_samples].align_to::<SampleDesc>()
        };

        // Get envelopes
        br.pos = first_envelope_offset;
        let num_envelopes = br.read_u8() as usize;
        br.align_to_struct::<EnvelopeDesc>();
        let (_, envelopes, _) = unsafe {
            data[br.pos..br.pos + size_of::<EnvelopeDesc>() * num_envelopes]
                .align_to::<EnvelopeDesc>()
        };

        // Get instruments
        br.pos = first_instrument_offset;
        let num_instruments = br.read_u8() as usize;
        br.align_to_struct::<InstrumentDesc>();
        let (_, instruments, _) = unsafe {
            data[br.pos..br.pos + size_of::<InstrumentDesc>() * num_instruments]
                .align_to::<InstrumentDesc>()
        };

        // Get channel event streams
        br.pos = first_channel_offset;
        let num_channels = br.read_u8() as usize;
        br.align_to_struct::<ChannelDesc>();
        let (_, channels, _) = unsafe {
            data[br.pos..br.pos + size_of::<ChannelDesc>() * num_channels].align_to::<ChannelDesc>()
        };

        // Get module descriptor data
        br.pos = module_desc_offset;

        Ok(ModuleDesc {
            data,
            samples,
            envelopes,
            instruments,
            channels,
            tempo: Range::<u8>::read(&mut br),
            bpm: Range::<u16>::read(&mut br),
        })
    }
}
