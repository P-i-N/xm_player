#![warn(unused_imports)]

use core::mem::transmute;
use std::intrinsics::needs_drop;

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

fn find_repeated_subsequence<T>(mut haystack: &[T], needle: &[T]) -> Option<(usize, usize)>
where
    for<'a> &'a [T]: PartialEq,
{
    let mut first_position = usize::MAX;
    let mut count = 0;

    loop {
        if let Some(pos) = haystack.windows(needle.len()).position(|w| w == needle) {
            if first_position == usize::MAX {
                first_position = pos;
            }

            count += 1;

            if pos + needle.len() < haystack.len() {
                haystack = &haystack[pos + needle.len()..];
            } else {
                break;
            }
        } else {
            break;
        }
    }

    if first_position == usize::MAX {
        None
    } else {
        Some((first_position, count))
    }
}

fn erase_repeated_subsequence<T>(mut haystack: &[T], needle: &[T], marker: &[T]) -> Vec<T>
where
    for<'a> &'a [T]: PartialEq,
    T: Clone,
{
    let mut result = Vec::new();
    result.extend_from_slice(haystack);

    let mut is_first = true;
    let mut offset = 0;

    loop {
        if let Some(pos) = (&result[offset..])
            .windows(needle.len())
            .position(|w| w == needle)
        {
            if is_first {
                is_first = false;
                offset = pos + needle.len();
            } else {
                let mut new_result = Vec::<T>::new();
                new_result.extend_from_slice(&result[0..offset + pos]);
                new_result.extend_from_slice(&marker);
                new_result.extend_from_slice(&result[offset + pos + needle.len()..]);

                result = new_result;
            }
        } else {
            break;
        }
    }

    return result;
}

fn compress_channel_stream(input: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    result.extend_from_slice(&input);

    println!("Compressing channel stream of {} bytes...", result.len());

    loop {
        let mut best_substr_len = 0;
        let mut best_substr_offset = 0;
        let mut best_substr_bytes = 0;

        for offset in 0..result.len() - 4 {
            for len in 4..result.len() {
                if (offset + len >= result.len()) || (offset + len + len > result.len()) {
                    break;
                }

                let end_index = (offset + len).min(result.len());

                let haystack = &result[offset + len..];
                let needle = &result[offset..end_index];

                if needle.len() >= haystack.len() {
                    break;
                }

                if let Some((pos, count)) = find_repeated_subsequence(haystack, needle) {
                    let mut bytes = len * (count - 1);
                    if bytes <= 4 * (count - 1) {
                        bytes = 0;
                    } else {
                        bytes -= 4 * (count - 1);
                    }

                    if bytes > best_substr_bytes {
                        best_substr_offset = offset;
                        best_substr_len = len;
                        best_substr_bytes = bytes;
                    }
                } else {
                    break;
                }
            }
        }

        /*
        println!("Offset: {}", best_substr_offset);
        println!("Length: {}", best_substr_len);
        println!(" Bytes: {}", best_substr_bytes);
        */

        if best_substr_bytes > 0 {
            let marker: [u8; 3] = [64, 64, 64];

            result = erase_repeated_subsequence(
                &result,
                &result[best_substr_offset..best_substr_offset + best_substr_len],
                &marker,
            );
        } else {
            break;
        }
    }

    println!("Compressed to {} bytes!", result.len());

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

        let mut orig_channel_stream_size = 0;
        let mut comp_channel_stream_size = 0;

        for i in 0..module.num_channels {
            println!("Channel {} of {}:", i + 1, module.num_channels);
            //write_marker("[CHANNEL]", packed_data);

            let mut channel_stream = Vec::<u8>::new();
            write_channel_stream(module, i, &mut channel_stream);
            orig_channel_stream_size += channel_stream.len();

            let comp_channel_stream = compress_channel_stream(&channel_stream);
            comp_channel_stream_size += comp_channel_stream.len();
            packed_data.extend_from_slice(&comp_channel_stream);
        }

        println!(
            "Orig channel stream size: {} bytes",
            orig_channel_stream_size
        );
        println!(
            "Comp channel stream size: {} bytes",
            comp_channel_stream_size
        );

        PackedModule { data: packed_data }
    }
}
