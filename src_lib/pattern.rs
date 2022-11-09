use super::{BinaryReader, BitTest, Box, Cell, Error, Vec};

#[derive(Clone, Default)]
pub struct Pattern {
    pub num_rows: usize,
    pub channels: Vec<Vec<Cell>>,
}

impl Pattern {
    pub fn parse(&mut self, br: &mut BinaryReader) -> Result<(), Box<dyn Error>> {
        let _pattern_header_len = br.read_u32();

        // Packing type, not used
        br.read_u8();

        self.num_rows = br.read_u16() as usize;
        for i in 0..self.channels.len() {
            self.channels[i].resize(self.num_rows, Cell::new());
        }

        let packed_data_size = br.read_u16() as usize;

        let mut i: usize = 0;
        let mut line: usize = 0;
        let mut channel: usize = 0;

        while i < packed_data_size {
            let note = br.read_u8();
            i += 1;

            let row = &mut self.channels[channel][line];
            *row = Cell::new();

            // Packed row item
            if note.test_bitmask(0x80) {
                if note.test_bitmask(0b00001) {
                    row.note = br.read_u8();
                    i += 1;
                }

                if note.test_bitmask(0b00010) {
                    row.instrument = br.read_u8();
                    i += 1;
                }

                if note.test_bitmask(0b00100) {
                    row.volume = br.read_u8();
                    i += 1;
                }

                if note.test_bitmask(0b01000) {
                    row.effect_type = br.read_u8();
                    i += 1;
                }

                if note.test_bitmask(0b10000) {
                    row.effect_param = br.read_u8();
                    i += 1;
                }
            }
            // Full row item
            else {
                row.note = note;
                row.instrument = br.read_u8();
                row.volume = br.read_u8();
                row.effect_type = br.read_u8();
                row.effect_param = br.read_u8();
                i += 4;
            }

            channel += 1;
            if channel == self.channels.len() {
                channel = 0;
                line += 1;
            }
        }

        Ok(())
    }

    pub fn get_channel_row(&self, channel_index: usize, row_index: usize) -> Cell {
        if channel_index >= self.channels.len() {
            return Cell::new();
        }

        let rows = &self.channels[channel_index];
        if row_index < rows.len() {
            rows[row_index]
        } else {
            Cell::new()
        }
    }
}
