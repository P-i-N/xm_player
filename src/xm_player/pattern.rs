use std::error;

use super::{BinaryReader, BitTest, NibbleTest};

#[derive(Clone, Copy, Default)]
pub struct Row {
    pub note: u8,
    pub instrument: u8,
    pub volume: u8,
    pub effect_type: u8,
    pub effect_param: u8,
}

impl Row {
    pub fn to_colored_string(&self) -> String {
        let mut result = String::new();

        // Note
        if self.has_valid_note() {
            static NOTES: &'static str = "CCDDEFFGGAAB";
            static SHARP: &'static str = "-#-#--#-#-#-";
            let note_index = ((self.note - 1) % 12) as usize;
            let octave = 1 + ((self.note - 1) / 12) as usize;

            result += format!(
                "\x1b[37;1m{}{}{}",
                NOTES.chars().nth(note_index).unwrap(),
                SHARP.chars().nth(note_index).unwrap(),
                octave
            )
            .as_str();
        } else if self.is_note_off() {
            result += "== ";
        } else {
            result += "...";
        }

        // Instrument
        if self.instrument > 0 {
            result += format!("\x1b[34m{:02}", self.instrument).as_str();
        } else {
            result += "  ";
        }

        // Volume effect
        if self.volume >= 0x10 && self.volume <= 0x50 {
            result += format!("\x1b[32mv{:02}", self.volume - 16).as_str();
        }
        // Volume slide down
        else if self.volume.test_high_nibble(0x60) {
            result += format!("\x1b[32md{:02}", self.volume & 0x0F).as_str();
        }
        // Volume slide up
        else if self.volume.test_high_nibble(0x70) {
            result += format!("\x1b[32mc{:02}", self.volume & 0x0F).as_str();
        }
        // Fine slide down
        else if self.volume.test_high_nibble(0x80) {
            result += format!("\x1b[32mb{:02}", self.volume & 0x0F).as_str();
        }
        // Fine slide up
        else if self.volume.test_high_nibble(0x90) {
            result += format!("\x1b[32ma{:02}", self.volume & 0x0F).as_str();
        }
        // Portamento
        else if self.volume.test_high_nibble(0xF0) {
            result += format!("\x1b[35mg{:02}", self.volume & 0x0F).as_str();
        } else {
            result += "   ";
        }

        // Arpeggio
        if self.effect_type == 0x00 && self.effect_param > 0 {
            result += format!("\x1b[31;1m0{:02X}", self.effect_param).as_str();
        }
        // Tone portamento
        else if self.effect_type == 0x03 {
            result += format!("\x1b[35;1m3{:02X}", self.effect_param).as_str();
        }
        // Set panning
        else if self.effect_type == 0x08 {
            result += format!("\x1b[33;1m3{:02X}", self.effect_param).as_str();
        }
        // Set volume
        else if self.effect_type == 0x0C {
            result += format!("\x1b[32mv{:02X}", self.effect_param).as_str();
        } else {
            result += "\x1b[30;1m...";
        }

        result
    }

    pub fn has_valid_note(&self) -> bool {
        self.note > 0 && self.note < 97
    }

    pub fn has_portamento(&self) -> bool {
        self.effect_type == 3 || self.effect_type == 5 || self.volume.test_high_nibble(0xF0)
    }

    pub fn is_note_off(&self) -> bool {
        self.note == 97
    }
}

#[derive(Clone, Default)]
pub struct Pattern {
    pub num_rows: usize,
    pub channels: Vec<Vec<Row>>,
}

impl Pattern {
    pub fn parse(&mut self, br: &mut BinaryReader) -> Result<(), Box<dyn error::Error>> {
        let _pattern_header_len = br.read_u32();

        // Packing type, not used
        br.read_u8();

        self.num_rows = br.read_u16() as usize;
        for i in 0..self.channels.len() {
            self.channels[i].resize(self.num_rows, Row::default());
        }

        let packed_data_size = br.read_u16() as usize;

        let mut i: usize = 0;
        let mut line: usize = 0;
        let mut channel: usize = 0;
        let mut last_effect_type = 0xFFu8;

        while i < packed_data_size {
            let note = br.read_u8();
            i += 1;

            let row = &mut self.channels[channel][line];
            *row = Row::default();

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

    pub fn get_channel_row(&self, channel_index: usize, row_index: usize) -> Row {
        if channel_index >= self.channels.len() {
            return Row::default();
        }

        let rows = &self.channels[channel_index];
        if row_index < rows.len() {
            rows[row_index]
        } else {
            Row::default()
        }
    }
}
