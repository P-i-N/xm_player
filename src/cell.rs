use super::NibbleTest;

#[derive(Clone, Copy)]
pub struct Cell {
    pub note: u8,
    pub instrument: u8,
    pub volume: u8,
    pub effect_type: u8,
    pub effect_param: u8,
}

impl Cell {
    pub fn new() -> Self {
        Cell {
            note: 0,
            instrument: 0,
            volume: 0,
            effect_type: 0,
            effect_param: 0,
        }
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
