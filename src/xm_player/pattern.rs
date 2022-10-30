use std::error;

use super::BinaryReader;

#[derive(Clone, Copy)]
pub struct Row {
    pub note: u8,
    pub instrument: u8,
    pub volume: u8,
    pub effect_type: u8,
    pub effect_param: u8,
}

impl Default for Row {
    fn default() -> Self {
        Row {
            note: 0x80,
            instrument: 0,
            volume: 0,
            effect_type: 0,
            effect_param: 0,
        }
    }
}

impl Row {
    pub fn to_colored_string(&self) -> String {
        if self.note >= 0x80 {
            return format!("\x1b[30m...     ");
        } else if self.note == 96 {
            return format!("\x1b[0;37m== .....");
        }

        static NOTES: &'static str = "CCDDEFFGGAAB";
        static SHARP: &'static str = "-#-#--#-#-#-";
        let note_index = (self.note % 12) as usize;
        let octave = 1 + (self.note / 12) as usize;

        let mut result = String::new();

        // Note
        result += format!(
            "\x1b[37;1m{}{}{}",
            NOTES.chars().nth(note_index).unwrap(),
            SHARP.chars().nth(note_index).unwrap(),
            octave
        )
        .as_str();

        // Instrument
        result += format!("\x1b[34m{:02}", self.instrument).as_str();

        // Volume effect
        if self.volume >= 0x10 && self.volume <= 0x50 {
            result += format!("\x1b[32mv{:02}", self.volume - 16).as_str();
        } else {
            result += "   ";
        }

        result
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

        while i < packed_data_size {
            let note = br.read_u8();
            i += 1;

            let row = &mut self.channels[channel][line];

            // Packed row item
            if (note & 0x80) == 0x80 {
                if (note & 0b00001) != 0 {
                    row.note = br.read_u8() - 1;
                    i += 1;
                }

                if (note & 0b00010) != 0 {
                    row.instrument = br.read_u8();
                    i += 1;
                }

                if (note & 0b00100) != 0 {
                    row.volume = br.read_u8();
                    i += 1;
                }

                if (note & 0b01000) != 0 {
                    row.effect_type = br.read_u8();
                    i += 1;
                }

                if (note & 0b10000) != 0 {
                    row.effect_param = br.read_u8();
                    i += 1;
                }
            }
            // Full row item
            else {
                row.note = note - 1;
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
}
