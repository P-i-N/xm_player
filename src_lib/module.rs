use super::{Box, Error, Rc, Vec};

use super::BinaryReader;
use super::FormatError;
use super::Instrument;
use super::Pattern;
use super::Row;

pub struct Module<'a> {
    pub data: &'a [u8],
    pub patterns: Vec<Pattern>,
    pub pattern_order: Vec<usize>,
    pub instruments: Vec<Rc<Instrument>>,
    pub restart_position: usize,
    pub num_instruments: usize,
    pub num_channels: usize,
    pub tempo: usize,
    pub bpm: usize,
}

impl<'a> Module<'a> {
    pub fn from_memory(data: &'a [u8]) -> Result<Module, Box<dyn Error>> {
        let mut br = BinaryReader::new(data);

        // Skip ID text
        br.pos += 17;

        let mut result = Module {
            data,
            patterns: Vec::new(),
            pattern_order: Vec::new(),
            instruments: Vec::new(),
            restart_position: 0,
            num_instruments: 0,
            num_channels: 0,
            tempo: 0,
            bpm: 0,
        };

        result.parse_header(&mut br)?;

        for pattern in &mut result.patterns {
            (*pattern).parse(&mut br)?;
        }

        for _ in 0..result.num_instruments {
            let mut instrument = Instrument::default();
            instrument.parse(&mut br)?;

            result.instruments.push(Rc::new(instrument));
        }

        Ok(result)
    }

    pub fn get_instrument(&self, index: usize) -> Option<Rc<Instrument>> {
        if index < self.instruments.len() {
            Some(self.instruments[index].clone())
        } else {
            None
        }
    }

    pub fn get_channel_row_ordered(
        &self,
        pattern_order_index: usize,
        channel_index: usize,
        row_index: usize,
    ) -> Row {
        if pattern_order_index >= self.pattern_order.len() || channel_index >= self.num_channels {
            return Row::new();
        }

        let pattern_index = self.pattern_order[pattern_order_index];
        if pattern_index >= self.patterns.len() {
            return Row::new();
        }

        self.patterns[pattern_index].get_channel_row(channel_index, row_index)
    }

    fn parse_header(&mut self, br: &mut BinaryReader) -> Result<(), Box<dyn Error>> {
        // Skip module name
        br.pos += 20;

        // 0x1A separator
        if br.read_u8() != 0x1A {
            return Err(Box::new(FormatError::new("Invalid XM header")));
        }

        // Skip tracker name
        br.pos += 20;

        // Skip module version
        br.pos += 2;

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

        // Skip linear frequency table flag
        br.pos += 2;

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
