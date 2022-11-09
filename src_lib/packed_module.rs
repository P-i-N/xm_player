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

fn write_channel_stream(module: &Module, channel_index: usize, packed_data: &mut Vec<u8>) {
    let mut prev_cell = Cell::new();
    let mut num_repeat: u8 = 0;

    for p in &module.patterns {
        let mut row_index = 0;

        while row_index < p.num_rows {
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
        }
    }

    if num_repeat > 0 {
        packed_data.push((num_repeat - 1) | 0b_1100_0000);
    }
}

fn write_marker(name: &str, packed_data: &mut Vec<u8>) {
    for b in name.as_bytes() {
        packed_data.push(*b);
    }
}

fn find_subsequence<T>(haystack: &[T], needle: &[T]) -> Option<usize>
where
    for<'a> &'a [T]: PartialEq,
{
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn compress_channel_stream(input: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();

    let mut best_substr_len = 0;
    let mut best_substr_offset = 0;

    println!("Looking for longest substring in {} bytes...", input.len());

    for substr_off in 0..input.len() / 2 {
        for substr_len in 3..(input.len() / 2) - substr_off {
            let needle = &input[input.len() - substr_len - substr_off..input.len() - substr_off];

            if let Some(substr_offset) =
                find_subsequence(&input[0..input.len() - substr_len - substr_off], &needle)
            {
                if substr_len > best_substr_len {
                    best_substr_len = substr_len;
                    best_substr_offset = substr_offset;
                }
            }
        }
    }

    println!("Best substr len: {}", best_substr_len);
    println!("Best substr off: {}", best_substr_offset);

    result
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
                //write_marker("[SAMPLE]", packed_data);
                //write_sample(sample.as_ref(), &params, packed_data);
            }
        }

        for i in 0..module.num_channels {
            //write_marker("[CHANNEL]", packed_data);
            //write_channel_stream(module, i, packed_data);
        }

        let mut channel_stream = Vec::<u8>::new();
        write_channel_stream(module, 0, &mut channel_stream);

        packed_data.append(&mut compress_channel_stream(&channel_stream));

        PackedModule { data: packed_data }
    }
}
