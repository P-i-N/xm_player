use std::error::Error;
use std::fmt;

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

///////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct FormatError {
    details: String,
}

impl FormatError {
    pub fn new(details: &str) -> FormatError {
        FormatError {
            details: details.to_string(),
        }
    }
}

impl Error for FormatError {
    fn description(&self) -> &str {
        &self.details
    }
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.details)
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
