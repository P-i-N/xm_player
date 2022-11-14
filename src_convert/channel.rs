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

        if num_repeats <= 32 {
            self.symbols[begin] = Symbol::RLE(num_repeats as u8);
        }

        rle_symbols
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
                    i = begin + self.replace_rle_range(begin, num_repeats);
                    num_repeats = 0;
                }

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
                    for length in (4..259) {
                        if start + length * 2 > self.symbols.len() {
                            break;
                        }

                        let slice = &self.symbols[start..start + length];
                        if !Channel::only_row_events_or_dictionary(&slice) {
                            continue;
                        }

                        let mut count = 0;
                        let mut search_offset = start;
                        repeated_positions.clear();

                        for &pos in search_positions {
                            if pos <= search_offset || pos + length > self.symbols.len() {
                                continue;
                            }

                            if &self.symbols[pos..pos + length] == slice {
                                count += 1;
                                search_offset = pos + length - 1;
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

                let slice = self.symbols[best_start..best_start + best_length].to_vec();

                // Search offset
                let mut offset = best_start + best_length;

                let mut new_symbols = Vec::<Symbol>::new();
                let mut prev_pos = 0;

                // Erase all repeated occurences except for the first one. Replace erased
                // parts with back-reference to the first part.
                for &pos in &best_repeated_positions {
                    new_symbols.extend_from_slice(&self.symbols[prev_pos..pos]);

                    // Insert reference instead of subslice
                    new_symbols.push(Symbol::Reference((self.slices.len() - 1) as u8));

                    prev_pos = pos + best_length;
                }

                // Append rest of the symbols
                new_symbols.extend_from_slice(&self.symbols[prev_pos..]);
                self.symbols = new_symbols;

                /*
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
                */
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
        // Event dictionary
        {
            bw.write_u8(self.dict.len() as u8);
            for row in &self.dict {
                let symbol = Symbol::RowEvent(*row);
                symbol.write(bw);
            }
        }

        // References
        {
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
}

#[cfg(test)]
mod tests {
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
