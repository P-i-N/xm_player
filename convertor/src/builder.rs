use super::*;
use std::collections::HashMap;
use xmplay::*;

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
    pub tempo: Range<u8>,

    // Initial song BPM
    pub bpm: Range<u16>,

    // Samples (referenced by instruments)
    pub samples: Vec<Sample>,

    // Envelopes (referenced by instruments)
    pub envelopes: Vec<BEnvelope>,

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
            samples: Vec::new(),
            envelopes: Vec::new(),
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

        // Write samples
        if false {
            for sample in &mut self.samples {
                sample.desc.data_offset = bw.pos() as u32;
                sample.desc.data_length = sample.data.len() as u32;
                bw.write_slice(&sample.data);
            }
        }

        // Write envelopes
        if false {
            for envelope in &mut self.envelopes {
                envelope.desc.data_offset = bw.pos() as u32;
                envelope.desc.data_length = (envelope.tick_values.len() * 2) as u32;
                bw.write_slice(&envelope.tick_values);
            }
        }

        let mut byte_freqs = [0usize; 256];
        let mut shared_dict_rows: HashMap<Row, usize> = HashMap::new();

        // Build channel event streams
        {
            self.channels.clear();
            for channel_index in 0..self.num_channels {
                let channel = self.extract_channel(channel_index);
                channel.count_byte_freqs(&mut byte_freqs);

                for (row, _) in &channel.row_dict {
                    if let Some(count) = shared_dict_rows.get_mut(row) {
                        *count += 1;
                    } else {
                        shared_dict_rows.insert(*row, 1);
                    }
                }

                self.channels.push(channel);
            }
        }

        // Write byte frequencies
        if false {
            for &f in &byte_freqs {
                if f < 255 {
                    bw.write_u8(f as u8);
                } else {
                    bw.write_u8(255);
                }
            }
        }

        println!("Byte freqs: {:?}", byte_freqs);

        shared_dict_rows.drain_filter(|_, count| *count <= 1);
        println!("Num shared dict. rows: {}", shared_dict_rows.len());

        // Write channel event streams
        {
            for channel in &mut self.channels {
                channel.desc.data_offset = bw.pos() as u32;
                //channel.desc.data_length = channel.write(&mut bw, &byte_freqs) as u32;
            }

            self.channels[1].write(&mut bw, &byte_freqs);
        }

        // Write offsets to channels, instruments and samples at the end of data block
        if false {
            let first_sample_offset = bw.pos() as u32;

            bw.write_u8(self.samples.len() as u8);
            for sample in &self.samples {
                bw.write_aligned_struct(&sample.desc);
            }

            let first_envelope_offset = bw.pos() as u32;

            bw.write_u8(self.envelopes.len() as u8);
            for envelope in &self.envelopes {
                bw.write_aligned_struct(&envelope.desc);
            }

            let first_instrument_offset = bw.pos() as u32;

            bw.write_u8(self.instruments.len() as u8);
            for instrument in &self.instruments {
                bw.write_aligned_struct(&instrument.desc);
            }

            let first_channel_offset = bw.pos() as u32;

            bw.write_u8(self.channels.len() as u8);
            for channel in &self.channels {
                bw.write_aligned_struct(&channel.desc);
            }

            let module_desc_offset = bw.pos() as u32;

            self.tempo.write(&mut bw);
            self.bpm.write(&mut bw);

            bw.write_u32(first_sample_offset);
            bw.write_u32(first_envelope_offset);
            bw.write_u32(first_instrument_offset);
            bw.write_u32(first_channel_offset);
            bw.write_u32(module_desc_offset);
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
        //channel.compress_repeated_parts();

        //let unpacked_symbols = channel.unpack_symbols();
        //assert!(orig_symbols == unpacked_symbols);

        let current_encoding_size = channel.get_total_encoding_size();

        println!("- final size: {} bytes", current_encoding_size);

        channel
    }
}
