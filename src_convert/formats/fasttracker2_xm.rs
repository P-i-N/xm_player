use std::error;
use std::mem::transmute;

use super::*;
use benvelope::BEnvelope;
use xm_player::*;

#[derive(Default)]
struct ItemCounts {
    num_channels: usize,
    num_patterns: usize,
    num_instruments: usize,
}

pub fn convert_xm(build: &mut Builder, data: &[u8]) -> Result<(), Box<dyn error::Error>> {
    let mut br = BinaryReader::new(&data);

    // Skip ID text (17B) + module name (20B)
    br.skip(37);

    // 0x1A separator
    if br.read_u8() != 0x1A {
        return Err(Box::new(FormatError::new("Invalid XM header")));
    }

    // Skip tracker name (20B) + module version (2B)
    br.pos += 22;

    let item_counts = parse_header(build, &mut br)?;

    // Remember channel count
    build.num_channels = item_counts.num_channels;

    for _ in 0..item_counts.num_patterns {
        parse_pattern(build, &mut br)?;
    }

    for _ in 0..item_counts.num_instruments {
        parse_instrument(build, &mut br)?;
    }

    Ok(())
}

fn parse_header(
    build: &mut Builder,
    br: &mut BinaryReader,
) -> Result<ItemCounts, Box<dyn error::Error>> {
    let skip_pos = 60 + br.read_u32() as usize;

    let song_length = br.read_u16() as usize;

    build.restart_position = br.read_u16() as usize;

    let result = ItemCounts {
        num_channels: br.read_u16() as usize,
        num_patterns: br.read_u16() as usize,
        num_instruments: br.read_u16() as usize,
    };

    // Skip linear frequency table flag
    br.pos += 2;

    // Tempo and BPM
    build.tempo.set_all(br.read_u16() as usize);
    build.bpm.set_all(br.read_u16() as usize);

    // Read pattern order
    for _ in 0..song_length {
        build.pattern_order.push(br.read_u8() as usize);
    }

    br.pos = skip_pos;
    Ok(result)
}

fn parse_pattern(build: &mut Builder, br: &mut BinaryReader) -> Result<(), Box<dyn error::Error>> {
    // Skip pattern header length (4B) + packing type (1B)
    br.skip(5);

    let mut pattern = builder::Pattern {
        num_rows: br.read_u16() as usize,
        channel_rows: Vec::new(),
    };

    for _ in 0..build.num_channels {
        pattern.channel_rows.push(Vec::new());
    }

    let pattern_data_len = br.read_u16() as usize;
    let end_pos = br.pos + pattern_data_len;

    let mut channel_index: usize = 0;

    while br.pos < end_pos {
        let note = br.read_u8();

        let mut row = Row::new();

        // Packed row item
        if (note & 0x80) == 0x80 {
            if (note & 0b00001) != 0 {
                row.note = br.read_u8();
            }

            if (note & 0b00010) != 0 {
                row.instrument = br.read_u8();
            }

            if (note & 0b00100) != 0 {
                row.volume = br.read_u8();
            }

            if (note & 0b01000) != 0 {
                row.effect_type = br.read_u8();
            }

            if (note & 0b10000) != 0 {
                row.effect_param = br.read_u8();
            }
        }
        // Full row item
        else {
            row.note = note;
            row.instrument = br.read_u8();
            row.volume = br.read_u8();
            row.effect_type = br.read_u8();
            row.effect_param = br.read_u8();
        }

        pattern.channel_rows[channel_index].push(row);

        channel_index += 1;
        if channel_index >= build.num_channels {
            channel_index = 0;
        }
    }

    build.patterns.push(pattern);
    Ok(())
}

fn parse_instrument(
    build: &mut Builder,
    br: &mut BinaryReader,
) -> Result<(), Box<dyn error::Error>> {
    let mut instrument_size = br.read_u32() as usize;
    if instrument_size == 0 || instrument_size > 263 {
        instrument_size = 263;
    }

    let skip_pos = br.pos + instrument_size - 4;

    // Skip instrument name (22B) + type (1B)
    br.skip(23);

    let mut instr = builder::Instrument {
        index: build.instruments.len(),
        desc: InstrumentDesc::default(),
    };

    let num_samples = br.read_u16() as usize;
    if num_samples > 0 {
        // Skip sample header size (4B)
        br.skip(4);

        for i in &mut instr.desc.sample_keymap {
            let sample_index = br.read_u8() as usize;
            *i = if sample_index < num_samples {
                (build.samples.len() + sample_index) as u8
            } else {
                u8::MAX
            };
        }

        let mut volume_env_points = [0 as usize; 24];
        let mut panning_env_points = [0 as usize; 24];

        // Volume envelope points
        for i in 0..24 {
            volume_env_points[i] = br.read_u16() as usize;
        }

        // Panning envelope points
        for i in 0..24 {
            panning_env_points[i] = br.read_u16() as usize;
        }

        let num_volume_points = br.read_u8() as usize;
        let num_panning_points = br.read_u8() as usize;

        let mut volume_envelope = BEnvelope::default();
        let mut panning_envelope = BEnvelope::default();

        for i in (0..num_volume_points * 2).step_by(2) {
            volume_envelope
                .points
                .push((volume_env_points[i] as u32, volume_env_points[i + 1] as u32));
        }

        for i in (0..num_panning_points * 2).step_by(2) {
            panning_envelope.points.push((
                panning_env_points[i] as u32,
                panning_env_points[i + 1] as u32,
            ));
        }

        volume_envelope.desc.sustain = br.read_u8() as u16;
        volume_envelope.desc.loop_start = br.read_u8() as u16;
        volume_envelope.desc.loop_end = br.read_u8() as u16;

        panning_envelope.desc.sustain = br.read_u8() as u16;
        panning_envelope.desc.loop_start = br.read_u8() as u16;
        panning_envelope.desc.loop_end = br.read_u8() as u16;

        let _volume_flags = br.read_u8();
        let _panning_flags = br.read_u8();

        instr.desc.vibrato.waveform = br.read_u8();
        instr.desc.vibrato.sweep = br.read_u8();
        instr.desc.vibrato.depth = br.read_u8();
        instr.desc.vibrato.rate = br.read_u8();

        volume_envelope.desc.fadeout = br.read_u16();

        instr.desc.volume_envelope_index = build.envelopes.len() as u8;
        build.envelopes.push(volume_envelope);

        instr.desc.panning_envelope_index = build.envelopes.len() as u8;
        build.envelopes.push(panning_envelope);

        // Reserved, unused
        br.skip(22);

        let first_sample_header_pos = br.pos;
        let mut sample_data_pos = br.pos + num_samples * 40;

        // Read all samples
        for i in 0..num_samples {
            // Seek binary reader to start of sample header
            br.pos = first_sample_header_pos + i * 40;

            parse_sample(sample_data_pos, build, br)?;

            // Current binary reader position is start of next sample data position
            sample_data_pos = br.pos;
        }
    } else {
        br.pos = skip_pos;
    }

    build.instruments.push(instr);
    Ok(())
}

fn parse_sample(
    data_pos: usize,
    build: &mut Builder,
    br: &mut BinaryReader,
) -> Result<(), Box<dyn error::Error>> {
    let mut sample = Sample::default();
    let mut desc = sample.desc;

    let mut num_samples = br.read_u32() as usize;

    desc.loop_start = br.read_u32();
    desc.loop_end = desc.loop_start + br.read_u32();
    desc.volume = br.read_u8();
    desc.finetune = br.read_i8();

    let flags = br.read_u8();

    if (flags & 0b_0001_0000) != 0 {
        desc.flags |= SampleFlags::IS_16_BITS;
        desc.loop_start >>= 1;
        desc.loop_end >>= 1;
        num_samples >>= 1;
    }

    match flags & 0x3 {
        0 => {
            desc.loop_start = u32::MAX;
            desc.loop_end = u32::MAX;
        }
        1 => {
            //
        }
        2 => {
            desc.flags |= SampleFlags::PING_PONG;
        }
        _ => {
            return Err(Box::new(FormatError::new("Invalid sample loop type")));
        }
    }

    desc.panning = br.read_u8();
    desc.relative_note = br.read_i8();

    let compression_type = br.read_u8();

    // Skip sample name
    br.skip(22);

    // Jump to sample data position
    br.pos = data_pos;

    read_sample_data(
        &mut sample,
        &mut desc,
        num_samples,
        compression_type == 0xAD,
        br,
    )?;

    build.samples.push(sample);
    Ok(())
}

fn i16_to_u16(value: i16) -> u16 {
    let mut result = unsafe { transmute(value) };
    if result >= 0x8000 {
        result -= 0x8000;
    } else {
        result += 0x8000;
    }

    result
}

fn i8_to_u8(value: i8) -> u8 {
    let mut result = unsafe { transmute(value) };
    if result >= 0x80 {
        result -= 0x80;
    } else {
        result += 0x80;
    }

    result
}

fn read_sample_data(
    sample: &mut Sample,
    desc: &mut SampleDesc,
    num_samples: usize,
    adpcm: bool,
    br: &mut BinaryReader,
) -> Result<(), Box<dyn error::Error>> {
    if !adpcm {
        if (desc.flags & SampleFlags::IS_16_BITS) != 0 {
            let mut acc: i16 = 0;
            for i in 0..num_samples {
                (acc, _) = acc.overflowing_add(br.read_i16());
                let s = i16_to_u16(acc);
                sample.data.push((s & 0xFF) as u8);
                sample.data.push((s >> 8) as u8);
            }
        } else {
            let mut acc: i8 = 0;
            for i in 0..num_samples {
                (acc, _) = acc.overflowing_add(br.read_i8());
                sample.data.push(i8_to_u8(acc));
            }
        }
    } else {
        // ADPCM compression
        return Err(Box::new(FormatError::new("ADPCM not supported")));
    }

    Ok(())
}
