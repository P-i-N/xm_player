use xmplay::CallbackPosition;
use xmplay::Module;
use xmplay::Row;
use xmplay::SongState;

use std::time::Instant;

pub struct RowPrinter {
    // Instant when the current row tick began
    tick_begin_instant: Instant,

    // Accumulated time spent during row ticks in microseconds
    tick_time_acc: u32,
}

impl RowPrinter {
    pub fn new() -> RowPrinter {
        RowPrinter {
            tick_begin_instant: Instant::now(),
            tick_time_acc: 0,
        }
    }

    pub fn print_row(&self, row: &Row) -> String {
        let mut result = String::new();

        // Note
        if row.has_valid_note() {
            static NOTES: &'static str = "CCDDEFFGGAAB";
            static SHARP: &'static str = "-#-#--#-#-#-";
            let note_index = ((row.note - 1) % 12) as usize;
            let octave = 1 + ((row.note - 1) / 12) as usize;

            result += format!(
                "\x1b[37;1m{}{}{}",
                NOTES.chars().nth(note_index).unwrap(),
                SHARP.chars().nth(note_index).unwrap(),
                octave
            )
            .as_str();
        } else if row.is_note_off() {
            result += "== ";
        } else {
            result += "...";
        }

        // Instrument
        if row.instrument > 0 {
            result += format!("\x1b[34m{:02}", row.instrument).as_str();
        } else {
            result += "  ";
        }

        // Volume effect
        if row.volume >= 0x10 && row.volume <= 0x50 {
            result += format!("\x1b[32mv{:02}", row.volume - 16).as_str();
        }
        // Volume slide down
        else if (row.volume & 0xF0) == 0x60 {
            result += format!("\x1b[32md{:02}", row.volume & 0x0F).as_str();
        }
        // Volume slide up
        else if (row.volume & 0xF0) == 0x70 {
            result += format!("\x1b[32mc{:02}", row.volume & 0x0F).as_str();
        }
        // Fine slide down
        else if (row.volume & 0xF0) == 0x80 {
            result += format!("\x1b[32mb{:02}", row.volume & 0x0F).as_str();
        }
        // Fine slide up
        else if (row.volume & 0xF0) == 0x90 {
            result += format!("\x1b[32ma{:02}", row.volume & 0x0F).as_str();
        }
        // Portamento
        else if (row.volume & 0xF0) == 0xF0 {
            result += format!("\x1b[35mg{:02}", row.volume & 0x0F).as_str();
        } else {
            result += "   ";
        }

        // Arpeggio
        if row.effect_type == 0x00 && row.effect_param > 0 {
            result += format!("\x1b[31;1m0{:02X}", row.effect_param).as_str();
        }
        // Tone portamento
        else if row.effect_type == 0x03 {
            result += format!("\x1b[35;1m3{:02X}", row.effect_param).as_str();
        }
        // Set panning
        else if row.effect_type == 0x08 {
            result += format!("\x1b[33;1m3{:02X}", row.effect_param).as_str();
        }
        // Set volume
        else if row.effect_type == 0x0C {
            result += format!("\x1b[32mv{:02X}", row.effect_param).as_str();
        } else {
            result += "\x1b[30;1m...";
        }

        result
    }

    pub fn print<'a>(&mut self, song_state: &SongState, module: &Module<'a>) -> String {
        let mut s = String::new();

        s += format!("\x1b[0m{:02}", song_state.row_index + 1).as_str();

        for channel_index in 0..module.num_channels {
            let row = module.get_channel_row_ordered(
                song_state.pattern_order_index,
                channel_index,
                song_state.row_index,
            );

            if song_state.row_index == 0 {
                s += "\x1b[0m+";
            } else {
                s += "\x1b[0m|";
            }

            if row.is_empty() && song_state.row_index == 0 {
                s += format!("-----------").as_str();
            } else {
                s += self.print_row(&row).as_str();
            }
        }

        s += format!("\x1b[0m|CPU: {}us", self.tick_time_acc).as_str();

        s
    }

    pub fn tick<'a>(
        &mut self,
        cb_pos: CallbackPosition,
        song_state: &SongState,
        module: &Module<'a>,
    ) {
        match cb_pos {
            CallbackPosition::TickBegin => self.tick_begin_instant = Instant::now(),
            CallbackPosition::TickEnd => {
                self.tick_time_acc += self.tick_begin_instant.elapsed().as_micros() as u32;

                if song_state.row_tick == 0 {
                    println!("{}", self.print(song_state, module));
                    self.tick_time_acc = 0;
                }
            }
            _ => {}
        }
    }
}
