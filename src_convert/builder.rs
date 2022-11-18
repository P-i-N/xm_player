use super::*;
use xm_player::*;

pub struct Pattern {
    pub num_rows: usize,
    pub channel_rows: Vec<Vec<Row>>,
}

#[derive(Default)]
pub struct Vibrato {
    pub func: u32,
    pub sweep: u32,
    pub depth: u32,
    pub rate: u32,
}

#[derive(Default)]
pub struct Sample {
    pub index: usize,
    pub data: Vec<u8>,
    pub desc: SampleDesc,
}

pub struct Instrument {
    pub index: usize,
    pub desc: InstrumentDesc,
}

// Universal Module Builder
pub struct Builder {
    // Number of channels
    pub num_channels: usize,

    // Patterns (even unused ones)
    pub patterns: Vec<Pattern>,

    // Song pattern order
    pub pattern_order: Vec<usize>,

    // Restart position index (in pattern_order vector)
    pub restart_position: usize,

    // Initial song tempo
    pub tempo: Range<usize>,

    // Initial song BPM
    pub bpm: Range<usize>,

    // Envelopes (referenced by instruments)
    pub envelopes: Vec<BEnvelope>,

    // Samples (referenced by instruments)
    pub samples: Vec<Sample>,

    // Instruments
    pub instruments: Vec<Instrument>,

    // Rows of note events for individual channels
    channels: Vec<EventStream>,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            num_channels: 0,
            patterns: Vec::new(),
            pattern_order: Vec::new(),
            restart_position: 0,
            tempo: Range::new(6),
            bpm: Range::new(120),
            envelopes: Vec::new(),
            samples: Vec::new(),
            instruments: Vec::new(),
            channels: Vec::new(),
        }
    }

    pub fn build(&mut self) -> Vec<u8> {
        let mut result = Vec::new();
        let mut bw = BinaryWriter::new(&mut result);

        bw.write_u8('U' as u8); // Universal
        bw.write_u8('M' as u8); // Module
        bw.write_u8('0' as u8); // Major version
        bw.write_u8('1' as u8); // Minor version

        self.channels.clear();

        for channel_index in 0..self.num_channels {
            let mut channel = self.extract_channel(channel_index);

            channel.desc.data_offset = bw.pos() as u32;
            channel.write(&mut bw);

            self.channels.push(channel);
        }

        // Write samples
        {
            bw.write_u32(self.samples.len() as u32);
        }

        // Write offsets to channels, instruments and samples at the end of data block
        {
            let offset = bw.pos() as u32;

            //bw.write_u32(self.instruments.len() as u32);
            //bw.write_aligned_slice(&self.instruments);

            bw.write_u32(self.channels.len() as u32);
            for channel in &self.channels {
                bw.write_u32(channel.desc.data_offset as u32);
            }

            bw.write_u32(offset);
        }

        result
    }

    // Separate individual pattern rows into individual channel row streams
    fn extract_channel(&self, channel_index: usize) -> EventStream {
        let mut channel = EventStream::default();
        channel.index = channel_index;

        for poi in 0..self.pattern_order.len() {
            let pattern_index = self.pattern_order[poi];
            let pattern = &self.patterns[pattern_index];

            for row_index in 0..pattern.num_rows {
                if row_index < pattern.channel_rows[channel_index].len() {
                    let row = &pattern.channel_rows[channel_index][row_index];
                    channel.symbols.push(Symbol::RowEvent(*row));
                } else {
                    channel.symbols.push(Symbol::RowEvent(Row::new()));
                }
            }
        }

        let orig_encoding_size = channel.get_total_encoding_size();

        println!(
            "Processing channel {}: {} bytes",
            channel_index, orig_encoding_size
        );

        // Unpacked symbols
        let orig_symbols = channel.symbols.clone();

        channel.compress_rows_rle();
        channel.compress_with_dict();
        channel.compress_repeated_parts();

        let unpacked_symbols = channel.unpack_symbols();
        assert!(orig_symbols == unpacked_symbols);

        let current_encoding_size = channel.get_total_encoding_size();

        println!("- final size: {} bytes", current_encoding_size);

        channel
    }
}
