use std::cell::Ref;
use std::cell::RefCell;
use std::error;
use std::fs::File;
use std::io::Read;

use super::BinaryReader;
use super::FormatError;
use super::Instrument;
use super::Pattern;

#[derive(Default)]
pub struct Module {
    pub name: String,
    pub tracker: String,
    pub version: i32,
    pub patterns: Vec<Pattern>,
    pub pattern_order: Vec<usize>,
    pub instruments: Vec<RefCell<Instrument>>,
    pub restart_position: usize,
    pub num_instruments: usize,
    pub num_channels: usize,
    pub linear_freq_table: bool,
    pub tempo: usize,
    pub bpm: usize,
}

impl Module {
    pub fn load(path: &str) -> Result<Module, Box<dyn error::Error>> {
        let mut file = File::open(path)?;
        let mut _data = Vec::new();
        file.read_to_end(&mut _data)?;

        let data = _data;
        let mut br = BinaryReader::new(&data);

        // ID text
        if br.read_string_segment(17) != "Extended Module: " {
            return Err(Box::new(FormatError::new("Invalid ID text")));
        }

        let mut result = Module::default();

        result.parse_header(&mut br)?;

        for pattern in &mut result.patterns {
            (*pattern).parse(&mut br)?;
        }

        for instrument in &result.instruments {
            let mut i = instrument.borrow_mut();
            i.parse(&mut br)?;
        }

        Ok(result)
    }

    pub fn get_instrument<'a>(&'a self, index: usize) -> Option<Ref<'a, Instrument>> {
        if index < self.instruments.len() {
            Some(self.instruments[index].borrow())
        } else {
            None
        }
    }

    fn parse_header(&mut self, br: &mut BinaryReader) -> Result<(), Box<dyn error::Error>> {
        // Module name
        self.name = br.read_string_segment(20).trim().to_string();

        // 0x1A separator
        if br.read_u8() != 0x1A {
            return Err(Box::new(FormatError::new("Invalid XM header")));
        }

        // Tracker name
        self.tracker = br.read_string_segment(20).trim().to_string();

        // Module version
        self.version = br.read_u16() as i32;

        let header_size = br.read_u32() as usize;

        let song_length = br.read_u16() as usize;
        self.pattern_order.resize(song_length, 0);

        // Restart position index
        self.restart_position = br.read_u16() as usize;

        // Number of channels
        self.num_channels = br.read_u16() as usize;

        let num_patterns = br.read_u16() as usize;
        self.patterns.resize(num_patterns, Pattern::default());
        for i in 0..num_patterns {
            self.patterns[i]
                .channels
                .resize(self.num_channels, Vec::new());
        }

        self.num_instruments = br.read_u16() as usize;
        for _ in 0..self.num_instruments {
            self.instruments.push(RefCell::new(Instrument::default()));
        }

        // Usage of linear frequency table
        self.linear_freq_table = br.read_u16() == 1;

        // Tempo and BPM
        self.tempo = br.read_u16() as usize;
        self.bpm = br.read_u16() as usize;

        for i in 0..song_length {
            self.pattern_order[i] = br.read_u8() as usize;
        }

        br.pos = header_size + 60;
        Ok(())
    }
}
