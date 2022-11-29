use std::fmt::Binary;
use std::hash::Hasher;
use std::{collections::HashMap, hash::Hash};

use super::*;
use xm_player::BinaryWriter;
use xm_player::BitWriter;
use xm_player::RangeEncoder;
use xm_player::Row;
use xm_player::{ChannelDesc, Symbol, SymbolEncodingSize};

#[derive(Default)]
pub struct EventStream {
    pub index: usize,
    pub desc: ChannelDesc,
    pub symbols: Vec<Symbol>,
    pub row_dict: Vec<(Row, u16)>,
    pub eom_count: u16,
    pub slice_dict: Vec<Symbol>,
    pub slices: Vec<(u16, u16)>,
    pub search_map: HashMap<u64, Vec<usize>>,
}

impl EventStream {
    fn replace_rle_range(&mut self, begin: usize, num_repeats: usize) -> usize {
        let rle_symbols = 1;
        self.symbols.drain(begin + 1..begin + num_repeats);
        self.symbols[begin] = Symbol::RLE(num_repeats as u16);
        rle_symbols
    }

    fn replace_rle(&mut self, begin: usize, length: usize, num_repeats: usize) -> usize {
        self.symbols
            .drain(begin + length..begin + length + num_repeats - 1);

        self.symbols[begin + length] = Symbol::RLE(num_repeats as u16);
        begin + length + 1
    }

    fn get_or_create_dict_slice(&mut self, begin: usize, length: usize) -> usize {
        let mut result = self.slices.len();
        let slice = &self.symbols[begin..begin + length];

        // Find position of slice in slice dictionary
        if let Some(pos) = self
            .slice_dict
            .windows(slice.len())
            .position(|w| w == slice)
        {
            if let Some(slice_pos) = self
                .slices
                .iter()
                .position(|s| s.0 == (pos as u16) && s.1 == (length as u16))
            {
                result = slice_pos;
            } else {
                self.slices.push((pos as u16, length as u16));
            }
        } else {
            self.slices
                .push((self.slice_dict.len() as u16, slice.len() as u16));

            self.slice_dict.extend_from_slice(slice);
        }

        result
    }

    pub fn compress_rle(&mut self, min: usize, max: usize) {
        let mut slice_copy = Vec::new();

        for segment_length in (min..max + 1) {
            if segment_length * 2 >= self.symbols.len() {
                continue;
            }

            let mut num_repeats = 0;
            let mut i = 0;
            let mut begin = 0;

            while i < self.symbols.len() - segment_length * 2 {
                let current_slice = &self.symbols[i + segment_length..i + 2 * segment_length];

                if num_repeats == 0 {
                    if current_slice == &self.symbols[i..i + segment_length] {
                        num_repeats = 1;
                        begin = i;
                        i += segment_length;

                        slice_copy.clear();
                        slice_copy.extend_from_slice(current_slice);
                    } else {
                        i += 1;
                    }
                } else {
                    if current_slice == &slice_copy {
                        num_repeats += 1;
                        i += segment_length;
                    } else {
                        if num_repeats > 0 {
                            i = self.replace_rle(begin, segment_length, num_repeats);
                        }

                        num_repeats = 0;
                    }
                }
            }

            if num_repeats > 0 {
                self.replace_rle(begin, segment_length, num_repeats);
            }
        }
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

        let mut removed_rle_spaces = 0;

        prev_symbol = self.symbols[0];
        i = 1;
        while i < self.symbols.len() {
            let mut symbol = self.symbols[i];
            match symbol {
                Symbol::RLE(length) => {
                    if length < 8 && prev_symbol.is_empty_row() {
                        self.symbols[i - 1] = Symbol::Dictionary(length as u8);
                        self.symbols.remove(i);
                        symbol = self.symbols[i - 1];
                        i -= 1;
                        removed_rle_spaces += 1;
                    }
                }
                _ => {}
            }

            prev_symbol = symbol;
            i += 1;
        }

        if total_num_repeats > 0 {
            println!(
                "- after run-length encoding: {} bytes, {} spaces removed",
                self.get_total_encoding_size(),
                removed_rle_spaces
            );
        }
    }

    pub fn get_total_encoding_size(&self) -> usize {
        let mut result = 2 + self.slices.len() * 3;

        for (row, _) in &self.row_dict {
            result += Symbol::RowEvent(*row).encoding_size();
        }

        for symbol in &self.slice_dict {
            result += symbol.encoding_size();
        }

        for row in &self.symbols {
            result += row.encoding_size();
        }

        result
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
            let hash = EventStream::symbol_slice_hash(&self.symbols[pos..pos + 4]);

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

            let mut best_length = 0;
            let mut best_count = 0;
            let mut best_total_saved_bytes = 0;

            best_repeated_positions.clear();

            for start in 0..self.symbols.len() - 8 {
                // Use hash of first 4 symbols as a lookup to search_map to get a vector
                // of potential matching positions
                let slice_header = &self.symbols[start..start + 4];
                let hash = EventStream::symbol_slice_hash(&slice_header);

                if let Some(search_positions) = self.search_map.get(&hash) {
                    for length in (4..37) {
                        if start + length > self.symbols.len() {
                            break;
                        }

                        let slice_footer = &self.symbols[start + 4..start + length];

                        let mut count = 0;
                        let mut search_offset = start;
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

                        if count > 1 {
                            let slice_size = self.symbols[start..start + length].encoding_size();

                            let saved_bytes = length * (count - 1);

                            if saved_bytes > best_total_saved_bytes {
                                best_length = length;
                                best_count = count;
                                best_total_saved_bytes = saved_bytes;

                                best_repeated_positions.clear();
                                best_repeated_positions.extend_from_slice(&repeated_positions);
                            }
                        }
                    }
                }
            }

            if best_count > 0 {
                let start = best_repeated_positions[0];
                let slice_index = self.get_or_create_dict_slice(start, best_length);

                let mut new_symbols = Vec::<Symbol>::new();
                let mut prev_pos = 0;

                // Erase all repeated occurences except for the first one. Replace erased
                // parts with back-reference to the first part.
                for &pos in &best_repeated_positions {
                    new_symbols.extend_from_slice(&self.symbols[prev_pos..pos]);

                    // Insert reference instead of subslice
                    new_symbols.push(Symbol::Reference(slice_index as u8));

                    prev_pos = pos + best_length;
                }

                // Append rest of the symbols
                new_symbols.extend_from_slice(&self.symbols[prev_pos..]);
                self.symbols = new_symbols;
            } else {
                break;
            }

            //self.compress_rows_rle();
        }

        println!(
            "- after pattern matching: {} bytes, {} repeating slices",
            self.get_total_encoding_size(),
            self.slices.len()
        );
    }

    pub fn compress_with_dict(&mut self) {
        let mut event_counts = HashMap::<Row, usize>::new();
        let mut eom_count = 0;

        for symbol in &self.symbols {
            match symbol {
                Symbol::RowEvent(row) => {
                    if let Some(count) = event_counts.get(&row) {
                        event_counts.insert(*row, count + 1);
                    } else {
                        event_counts.insert(*row, 1);
                    }
                }
                _ => {
                    eom_count += 1;
                }
            }
        }

        let mut most_used_events =
            Vec::<(Row, u16)>::from_iter(event_counts.iter().filter(|&(_, count)| *count > 1).map(
                |(row, count)| {
                    (
                        *row,
                        *count as u16,
                        //((*row).get_encoding_size() * (*count)) as u16,
                    )
                },
            ));

        most_used_events.sort_by(|item1, item2| item2.1.cmp(&item1.1));

        if most_used_events.len() > 120 {
            most_used_events.resize(120, (Row::new(), 0));
        }

        let mut total_count = 0;
        for (_, count) in &most_used_events {
            total_count += *count;
        }

        print!("Most used event counts:");

        for (_, count) in &most_used_events {
            print!(" {}", count);
        }

        println!(" / total={}, other={}", total_count, eom_count);

        for symbol in &mut self.symbols {
            match symbol {
                Symbol::RowEvent(row) => {
                    if let Some(pos) = most_used_events.iter().position(|n| *row == n.0) {
                        *symbol = Symbol::Dictionary((pos + 8) as u8);
                    }
                }
                _ => {}
            }
        }

        self.row_dict = most_used_events;

        println!(
            "- after applied dictionary: {} bytes, row dict. size={}",
            self.get_total_encoding_size(),
            self.row_dict.len()
        );
    }

    pub fn compress_entropy(&self) -> Vec<u8> {
        let mut data = Vec::new();
        let mut bw = BinaryWriter::new(&mut data);

        for symbol in &self.symbols {
            symbol.write(&mut bw);
        }

        // Data byte probabilities
        let mut counts = Vec::<usize>::with_capacity(256);
        counts.resize(256, 0);

        for b in &data {
            counts[*b as usize] += 1;
        }

        let mut bit_output = Vec::<u8>::new();
        let mut bit_writer = BitWriter::new(&mut bit_output);
        let mut rc = RangeEncoder::new(&mut bit_writer);
        rc.symbol_counts = counts;

        for b in &data {
            rc.encode(*b as usize);
        }

        bit_output
    }

    pub fn write(&self, bw: &mut BinaryWriter) -> usize {
        let start_pos = bw.pos();

        // Event dictionary
        {
            bw.write_u8(self.row_dict.len() as u8);
            for (row, prob) in &self.row_dict {
                let symbol = Symbol::RowEvent(*row);
                symbol.write(bw);
            }
        }

        // References
        {
            bw.write_u8(self.slice_dict.len() as u8);
            for symbol in &self.slice_dict {
                symbol.write(bw);
            }

            bw.write_u8(self.slices.len() as u8);
            for slice in &self.slices {
                bw.write_u16(slice.0);
                bw.write_u8((slice.1 - 4) as u8);
            }
        }

        let symbol_data = self.compress_entropy();
        bw.write_slice(&symbol_data);

        /*
        // Symbols
        for symbol in &self.symbols {
            symbol.write(bw);
        }
        */

        bw.pos() - start_pos
    }

    pub fn unpack_symbols(&self) -> Vec<Symbol> {
        let mut result = Vec::new();
        let mut ref_stack = Vec::<(u16, u16)>::new();
        let mut max_depth = 0;

        let mut i = 0;
        while i < self.symbols.len() {
            let mut move_to_next = true;

            let symbol = if let Some(slice) = ref_stack.last_mut() {
                self.slice_dict[slice.0 as usize].clone()
            } else {
                self.symbols[i].clone()
            };

            match symbol {
                Symbol::Dictionary(index) => {
                    result.push(Symbol::RowEvent(self.row_dict[index as usize].0));
                }
                Symbol::RowEvent(row) => {
                    result.push(Symbol::RowEvent(row));
                }
                Symbol::RLE(length) => {
                    let repeated_symbol = result.last().unwrap().clone();
                    for _ in 0..length {
                        result.push(repeated_symbol);
                    }
                }
                Symbol::Reference(index) => {
                    let slice = self.slices[index as usize];
                    ref_stack.push(slice);
                    move_to_next = false;
                    max_depth = max_depth.max(ref_stack.len());
                }
                _ => {}
            }

            if move_to_next {
                loop {
                    if let Some(mut slice) = ref_stack.pop() {
                        if slice.1 > 1 {
                            slice.0 += 1;
                            slice.1 -= 1;
                            ref_stack.push(slice);
                            break;
                        }
                    } else {
                        i = i + 1;
                        break;
                    }
                }
            }
        }

        println!("Max. unpack depth: {}", max_depth);

        result
    }
}

pub mod tests {
    use xm_player::Symbol;

    use super::EventStream;

    fn notes_from_string(channel: &mut EventStream, notes: &str) {
        for ch in notes.chars() {
            //
        }
    }

    #[test]
    pub fn compress_decompress_eq() {
        //
    }
}
