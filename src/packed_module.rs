#![warn(unused_imports)]

use core::mem::transmute;

use super::{Cell, Instrument, Module, Pattern, Sample, Vec};

pub struct PackedModule<'a> {
    data: &'a [u8],
}

pub struct PackingParams {
    pub convert_to_8bits: bool,

    // =0: no downsampling
    // >0: any sample larger than this value will be downsampled by half
    pub downsample_threshold: u32,
}

impl Default for PackingParams {
    fn default() -> Self {
        PackingParams {
            convert_to_8bits: false,
            downsample_threshold: 0,
        }
    }
}

fn write_sample(sample: &Sample, params: &PackingParams, packed_data: &mut Vec<u8>) {
    unsafe {
        if params.downsample_threshold > 0
            && (sample.data.len() as u32) > params.downsample_threshold
        {
            for i in (0..sample.data.len()).step_by(2) {
                let s1 = *sample.data.get_unchecked(i);
                let s2 = *sample.data.get_unchecked(i + 1);
                let avg = (((s1 as i32) + (s2 as i32)) / 2) as i16;

                let mut s_u16: u16 = transmute(avg);
                if s_u16 >= 0x8000 {
                    s_u16 -= 0x8000;
                } else {
                    s_u16 += 0x8000;
                }

                if params.convert_to_8bits {
                    packed_data.push((s_u16 >> 8) as u8);
                } else {
                    packed_data.push((s_u16 >> 8) as u8);
                    packed_data.push((s_u16 & 0xFF) as u8);
                }
            }
        } else {
            for s in &sample.data {
                let mut s_u16: u16 = transmute(*s);
                if s_u16 >= 0x8000 {
                    s_u16 -= 0x8000;
                } else {
                    s_u16 += 0x8000;
                }

                if params.convert_to_8bits {
                    packed_data.push((s_u16 >> 8) as u8);
                } else {
                    packed_data.push((s_u16 >> 8) as u8);
                    packed_data.push((s_u16 & 0xFF) as u8);
                }
            }
        }
    }
}

fn write_pattern(p: &Pattern, packed_data: &mut Vec<u8>) {
    let num_channels = p.channels.len();

    let mut channel_index = 0;
    let mut row_index = 0;
    let mut prev_cell = Cell::new();
    let mut num_repeat: u8 = 0;

    while channel_index < num_channels {
        let cell = p.channels[channel_index][row_index];

        if cell == prev_cell {
            if num_repeat == 64 {
                packed_data.push((num_repeat - 1) | 0b_1100_0000);
                num_repeat = 0;
            }

            num_repeat += 1;
        } else {
            if num_repeat > 0 {
                packed_data.push((num_repeat - 1) | 0b_1100_0000);
                num_repeat = 0;
            }

            cell.write_packed(packed_data);
        }

        prev_cell = cell;

        row_index += 1;
        if row_index >= p.channels[channel_index].len() {
            row_index = 0;
            channel_index += 1;
            prev_cell = Cell::new();

            if num_repeat > 0 {
                packed_data.push((num_repeat - 1) | 0b_1100_0000);
                num_repeat = 0;
            }
        }
    }
}

impl<'a> PackedModule<'a> {
    pub fn from_module(
        module: &Module,
        params: PackingParams,
        packed_data: &'a mut Vec<u8>,
    ) -> Self {
        // Convert & store all instrument samples
        for instrument in &module.instruments {
            for sample in &instrument.samples {
                write_sample(sample.as_ref(), &params, packed_data);
            }
        }

        // Pack all patterns
        for pattern in &module.patterns {
            write_pattern(pattern, packed_data);
        }

        PackedModule { data: packed_data }
    }
}
