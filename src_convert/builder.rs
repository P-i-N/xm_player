use super::Channel;
use xm_player::BinaryWriter;
use xm_player::Row;
use xm_player::Symbol;

pub struct Pattern {
    pub num_rows: usize,
    pub channel_rows: Vec<Vec<Row>>,
}

#[derive(Default)]
pub struct Envelope {
    pub points: Vec<(u32, u32)>,
    pub sustain: usize,
    pub loop_start: usize,
    pub loop_end: usize,
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
    pub offset: usize,
    pub data: Vec<u8>,
    pub is_16bit: bool,
    pub loop_start: u32,
    pub loop_end: u32,
    pub loop_pingpong: bool,
    pub volume: u8,
    pub panning: u8,
    pub relative_note: i8,
    pub finetune: i32,
}

pub struct Instrument {
    pub offset: usize,
    pub sample_keymap: [usize; 96],
    pub volume_envelope: Envelope,
    pub panning_envelope: Envelope,
    pub vibrato: Vibrato,
    pub fadeout: u32,
    pub samples: Vec<Sample>,
}

#[derive(Clone, Copy, Default)]
pub struct Range<T> {
    pub min: T,
    pub max: T,
    pub default: T,
}

impl<T: Copy> Range<T> {
    pub fn set_all(&mut self, value: T) {
        self.min = value;
        self.max = value;
        self.default = value;
    }
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

    // Instruments
    pub instruments: Vec<Instrument>,

    // Rows of note events for individual channels
    channels: Vec<Channel>,
}

impl Builder {
    pub fn new() -> Self {
        Self {
            num_channels: 0,
            patterns: Vec::new(),
            pattern_order: Vec::new(),
            restart_position: 0,
            tempo: Range {
                min: 6,
                max: 6,
                default: 6,
            },
            bpm: Range {
                min: 120,
                max: 120,
                default: 120,
            },
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

            channel.offset = bw.pos();
            channel.write(&mut bw);

            self.channels.push(channel);
        }

        // Write offsets to channels, instruments and samples at the end of data block
        {
            let offset = bw.pos() as u32;

            bw.write_u32(self.instruments.len() as u32);
            for instrument in &self.instruments {
                bw.write_u32(instrument.offset as u32);
            }

            bw.write_u32(self.channels.len() as u32);
            for channel in &self.channels {
                bw.write_u32(channel.offset as u32);
            }

            bw.write_u32(offset);
        }

        result
    }

    // Separate individual pattern rows into individual channel row streams
    fn extract_channel(&self, channel_index: usize) -> Channel {
        let mut channel = Channel::default();
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
