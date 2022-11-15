use std::hash::Hasher;
use std::{collections::HashMap, hash::Hash};

use super::*;
use xm_player::BinaryWriter;
use xm_player::Row;
use xm_player::Symbol;

#[derive(Default)]
pub struct Channel {
    pub index: usize,
    pub symbols: Vec<Symbol>,
    pub dict: Vec<Row>,
    pub slices: Vec<(u16, u8)>,
    pub search_map: HashMap<u64, Vec<usize>>,
}

impl Channel {
    fn replace_rle_range(&mut self, begin: usize, num_repeats: usize) -> usize {
        let rle_symbols = 1;
        self.symbols.drain(begin + 1..begin + num_repeats);
        self.symbols[begin] = Symbol::RLE(num_repeats as u16);
        rle_symbols
    }

    pub fn compress_rows_rle(&mut self) {
        let mut prev_symbol = Symbol::Unknown;
        let mut begin: usize = 0;
        let mut num_repeats: usize = 0;
        let mut i = 0;
        let mut total_num_repeats = 0;

        while i < self.symbols.len() {
            let symbol = self.symbols[i];

            if symbol == prev_symbol && symbol.is_row_event_or_dictionary() {
                if num_repeats == 0 {
                    begin = i;
                }

                num_repeats += 1;
                total_num_repeats += 1;
            } else {
                if num_repeats > 0 {
                    i = begin + self.replace_rle_range(begin, num_repeats);
                    num_repeats = 0;
                }
            }

            prev_symbol = symbol;
            i += 1;
        }

        if num_repeats > 0 {
            self.replace_rle_range(begin, num_repeats);
        }

        if total_num_repeats > 0 {
            println!(
                "- after run-length encoding: {} bytes",
                self.get_total_encoding_size()
            );
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

    fn symbol_slice_hash(slice: &[Symbol]) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        slice.hash(&mut hasher);
        hasher.finish()
    }

    fn rebuild_search_map(&mut self) {
        if self.symbols.len() < 4 {
            self.search_map.clear();
            return;
        }

        let mut search_map = HashMap::<u64, Vec<usize>>::new();

        for pos in 0..self.symbols.len() - 4 {
            let hash = Channel::symbol_slice_hash(&self.symbols[pos..pos + 4]);

            if let Some(v) = search_map.get_mut(&hash) {
                v.push(pos);
            } else {
                search_map.insert(hash, vec![pos]);
            }
        }

        self.search_map = search_map;
    }

    pub fn compress_repeated_parts(&mut self) {
        let mut repeated_positions = Vec::new();
        let mut best_repeated_positions = Vec::new();

        while self.slices.len() < 64 && self.symbols.len() >= 8 {
            self.rebuild_search_map();

            let mut best_start = 0;
            let mut best_length = 0;
            let mut best_count = 0;
            let mut best_total_saved_bytes = 0;

            best_repeated_positions.clear();

            for start in 0..self.symbols.len() - 4 {
                // Use hash of first 4 symbols as a lookup to search_map to get a vector
                // of potential matching positions
                let slice_header = &self.symbols[start..start + 4];
                if !Channel::only_row_events_or_dictionary(&slice_header) {
                    continue;
                }

                let hash = Channel::symbol_slice_hash(&slice_header);

                if let Some(search_positions) = self.search_map.get(&hash) {
                    for length in (4..260) {
                        if start + length * 2 > self.symbols.len() {
                            break;
                        }

                        // Should not lay inside existing slice
                        for slice in &self.slices {}

                        let slice_footer = &self.symbols[start + 4..start + length];
                        if !Channel::only_row_events_or_dictionary(&slice_footer) {
                            continue;
                        }

                        let mut count = 0;
                        let mut search_offset = start + length;
                        repeated_positions.clear();

                        for &pos in search_positions {
                            if pos < search_offset || pos + length > self.symbols.len() {
                                continue;
                            }

                            if &self.symbols[pos + 4..pos + length] == slice_footer {
                                count += 1;
                                search_offset = pos + length;
                                repeated_positions.push(pos);
                            }
                        }

                        if count > 0 {
                            let total_saved_bytes = (length - 1) * count;
                            if total_saved_bytes > best_total_saved_bytes {
                                best_start = start;
                                best_length = length;
                                best_count = count;
                                best_total_saved_bytes = total_saved_bytes;

                                best_repeated_positions.clear();
                                best_repeated_positions.extend_from_slice(&repeated_positions);
                            }
                        }
                    }
                }
            }

            if best_count > 0 {
                self.slices
                    .push((best_start as u16, (best_length - 4) as u8));

                let mut new_symbols = Vec::<Symbol>::new();
                let mut new_slices = self.slices.clone();
                let mut prev_pos = 0;

                // Erase all repeated occurences except for the first one. Replace erased
                // parts with back-reference to the first part.
                for &pos in &best_repeated_positions {
                    new_symbols.extend_from_slice(&self.symbols[prev_pos..pos]);

                    // Insert reference instead of subslice
                    new_symbols.push(Symbol::Reference((self.slices.len() - 1) as u8));

                    for slice in &mut new_slices {
                        //
                    }

                    prev_pos = pos + best_length;
                }

                // Append rest of the symbols
                new_symbols.extend_from_slice(&self.symbols[prev_pos..]);
                self.symbols = new_symbols;
                self.slices = new_slices;
            } else {
                break;
            }

            break;
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

        self.dict = Vec::from_iter(notes.iter().map(|&(row, _)| row));

        println!(
            "- after applied dictionary: {} bytes, dict. size={}",
            self.get_total_encoding_size(),
            self.dict.len()
        );
    }

    pub fn write(&self, bw: &mut BinaryWriter) {
        // Event dictionary
        {
            println!("- dictionary events: {}", self.dict.len());

            bw.write_u8(self.dict.len() as u8);
            for row in &self.dict {
                let symbol = Symbol::RowEvent(*row);
                symbol.write(bw);
            }
        }

        // References
        {
            println!("- references: {}", self.slices.len());

            bw.write_u8(self.slices.len() as u8);
            for slice in &self.slices {
                bw.write_u16(slice.0);
                bw.write_u8(slice.1);
            }
        }

        // Symbols
        for symbol in &self.symbols {
            symbol.write(bw);
        }
    }

    pub fn unpack_symbols(&self) -> Vec<Symbol> {
        let mut result = Vec::new();
        //let mut stack = Vec::new();

        let mut i = 0;
        while i < self.symbols.len() {
            match &self.symbols[i] {
                Symbol::Dictionary(index) => {
                    result.push(Symbol::RowEvent(self.dict[*index as usize]));
                }
                Symbol::RowEvent(row) => {
                    result.push(Symbol::RowEvent(*row));
                }
                Symbol::RLE(length) => {
                    let repeated_symbol = result.last().unwrap().clone();
                    for _ in 0..*length {
                        result.push(repeated_symbol);
                    }
                }
                Symbol::Reference(index) => {
                    let slice = self.slices[*index as usize];
                    for i in 0..(slice.1 as usize) + 4 {
                        let ref_symbol = self.symbols[(slice.0 as usize) + i].clone();
                        result.push(ref_symbol);
                    }
                }
                _ => {}
            }
            //
            i = i + 1;
        }

        result
    }
}

pub mod tests {
    use xm_player::Symbol;

    use super::Channel;

    fn notes_from_string(channel: &mut Channel, notes: &str) {
        for ch in notes.chars() {
            //
        }
    }

    #[test]
    pub fn compress_decompress_eq() {
        //
    }
}
