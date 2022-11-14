use std::{collections::HashMap, hash::Hash};

use super::*;
use xm_player::BinaryWriter;
use xm_player::Row;
use xm_player::Symbol;

#[derive(Default)]
pub struct Channel {
    pub index: usize,
    pub symbols: Vec<Symbol>,
    pub byte_offsets: Vec<usize>,
    pub dict: Vec<Row>,
    pub slices: Vec<(u16, u8)>,
}

impl Channel {
    fn drain_rle_range(&mut self, begin: usize, num_repeats: usize) {
        self.symbols.drain(begin + 1..begin + num_repeats);
        self.symbols[begin] = Symbol::RLE(num_repeats as u8);
    }

    pub fn compress_rows_rle(&mut self, compress_references: bool) {
        let mut prev_symbol = Symbol::Unknown;
        let mut begin: usize = 0;
        let mut num_repeats: usize = 0;
        let mut i = 0;
        let mut total_num_repeats = 0;

        while i < self.symbols.len() {
            let symbol = self.symbols[i];

            if symbol == prev_symbol && (compress_references == true || !symbol.is_reference()) {
                if num_repeats == 32 {
                    self.drain_rle_range(begin, num_repeats);
                    i = begin + 1;
                    num_repeats = 0;
                }

                if num_repeats == 0 {
                    begin = i;
                }

                num_repeats += 1;
                total_num_repeats += 1;
            } else {
                if num_repeats > 0 {
                    self.drain_rle_range(begin, num_repeats);
                    i = begin + 1;
                    num_repeats = 0;
                }
            }

            prev_symbol = symbol;
            i += 1;
        }

        if num_repeats > 0 {
            self.drain_rle_range(begin, num_repeats);
        }

        if total_num_repeats > 0 {
            println!(
                "- after run-length encoding: {} bytes",
                self.get_total_encoding_size()
            );
        }
    }

    fn find_number_of_repeats(&self, mut start: usize, length: usize) -> usize {
        if start + length * 2 > self.symbols.len() {
            return 0;
        }

        let slice = &self.symbols[start..start + length];
        let mut count = 0;

        loop {
            if let Some(pos) = self.symbols[start + length..]
                .windows(length)
                .position(|w| w == slice)
            {
                count += 1;

                if start + pos + length < self.symbols.len() {
                    start += pos + length;
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        count
    }

    fn rebuild_byte_offsets(&mut self) {
        self.byte_offsets.clear();
        let mut offset = 0;

        for row in &self.symbols {
            self.byte_offsets.push(offset);
            offset += row.get_encoding_size();
        }
    }

    pub fn get_total_encoding_size(&self) -> usize {
        let mut result = 0;

        for row in &self.symbols {
            result += row.get_encoding_size();
        }

        result
    }

    fn only_row_events_or_dictionary(slice: &[Symbol]) -> bool {
        for symbol in slice {
            if symbol.is_reference() {
                return false;
            }
        }

        true
    }

    pub fn compress_repeated_parts(&mut self) {
        while self.slices.len() < 64 {
            self.rebuild_byte_offsets();

            let mut best_start = 0;
            let mut best_length = 0;
            let mut best_count = 0;
            let mut best_total_saved_bytes = 0;

            for length in (4..259).rev() {
                let end = (self.symbols.len() as i32) - ((length * 2) as i32);
                if end <= 0 {
                    continue;
                }

                for start in 0..end as usize {
                    if !Channel::only_row_events_or_dictionary(&self.symbols[start..start + length])
                    {
                        continue;
                    }

                    let count = self.find_number_of_repeats(start, length);
                    if count > 0 {
                        let total_saved_bytes = length * count;
                        if total_saved_bytes > best_total_saved_bytes {
                            best_start = start;
                            best_length = length;
                            best_count = count;
                            best_total_saved_bytes = total_saved_bytes;
                        }
                    }
                }
            }

            if best_count > 0 {
                self.slices
                    .push((best_start as u16, (best_length - 4) as u8));

                let slice = self.symbols[best_start..best_start + best_length].to_vec();

                // Search offset
                let mut offset = best_start + best_length;
                let mut ref_index = 0;

                // Erase all repeated occurences except for the first one. Replace erased
                // parts with back-reference to the first part.
                while ref_index < best_count {
                    if let Some(pos) = self.symbols[offset..]
                        .windows(best_length)
                        .position(|w| w == slice)
                    {
                        // Symbols with this pattern removed
                        let mut new_symbols = Vec::<Symbol>::new();
                        new_symbols.extend_from_slice(&self.symbols[0..offset + pos]);

                        // Insert reference instead of subslice
                        new_symbols.push(Symbol::Reference((self.slices.len() - 1) as u8));

                        // Append rest of the symbols
                        new_symbols.extend_from_slice(&self.symbols[offset + pos + best_length..]);

                        self.symbols = new_symbols;
                        offset += 1;
                    }

                    ref_index += 1;
                }
            } else {
                break;
            }
        }

        println!(
            "- after pattern matching: {} bytes",
            self.get_total_encoding_size()
        );
    }

    pub fn compress_with_dict(&mut self) {
        let mut dict = HashMap::<u64, usize>::new();
        let mut hash_rows = HashMap::<u64, Row>::new();

        for symbol in &self.symbols {
            match symbol {
                Symbol::RowEvent(row) => {
                    let mut key: u64 = row.note as u64;
                    key |= (row.instrument as u64) << 8;
                    key |= (row.volume as u64) << 16;
                    key |= (row.effect_type as u64) << 24;
                    key |= (row.effect_param as u64) << 32;

                    if let Some(count) = dict.get(&key) {
                        dict.insert(key, count + 1);
                    } else {
                        dict.insert(key, 1);
                        hash_rows.insert(key, *row);
                    }
                }
                _ => {}
            }
        }

        let mut notes =
            Vec::<(Row, usize)>::from_iter(dict.iter().filter(|&(_, v)| *v > 1).map(|(k, v)| {
                let row = *hash_rows.get(k).unwrap();
                (row, row.get_encoding_size() * (*v))
            }));

        notes.sort_by(|item1, item2| item2.1.cmp(&item1.1));

        if notes.len() > 128 {
            notes.resize(128, (Row::new(), 0));
        }

        for symbol in &mut self.symbols {
            match symbol {
                Symbol::RowEvent(row) => {
                    if let Some(pos) = notes.iter().position(|n| *row == n.0) {
                        *symbol = Symbol::Dictionary(pos as u8);
                    }
                }
                _ => {}
            }
        }

        println!(
            "- after applied dictionary: {} bytes, dict. size={}",
            self.get_total_encoding_size(),
            dict.len()
        );
    }

    pub fn write(&self, bw: &mut BinaryWriter) {
        bw.write_u8(self.dict.len() as u8);

        for row in &self.dict {
            let symbol = Symbol::RowEvent(*row);
            symbol.write(bw);
        }

        bw.write_u8(self.slices.len() as u8);

        for slice in &self.slices {
            bw.write_u16(slice.0);
            bw.write_u8(slice.1);
        }

        let mut offsets = Vec::<usize>::new();
        let base_pos = bw.pos();

        for i in 0..self.symbols.len() {
            let mut symbol = self.symbols[i];
            let offset = bw.pos() - base_pos;
            offsets.push(offset);

            symbol.write(bw);
        }
    }
}
