#[cfg(target_os = "windows")]
use win32::Win32;

use std::error;

use ::xm_player::platform::*;
use ::xm_player::Module;
use ::xm_player::Player;

fn row_to_colored_string(row: &Row) -> String {
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

fn main() -> Result<(), Box<dyn error::Error>> {
    const SAMPLE_RATE: usize = 48000;
    let platform = Win32::new(SAMPLE_RATE);

    let file_data = std::fs::read("../../unreal.xm")?;

    let module = Module::from_memory(&file_data)?;

    let mut player = Player::new(&module, platform.as_ref(), SAMPLE_RATE, 1);

    //println!("Benchmarking...");
    //println!("Elapsed time: {}ms", player.benchmark().as_millis());

    //return Ok(());

    player.print_rows = true;

    let mut buffer = [0 as i16; SAMPLE_RATE * 2];

    while platform.audio_wait() {
        let samples_to_render = platform.get_available_samples();
        player.render(&mut buffer[0..samples_to_render]);
        platform.audio_render(&buffer[0..samples_to_render]);
    }

    Ok(())
}
