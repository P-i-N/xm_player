use super::{NibbleTest, Vec};

#[derive(Clone, Copy, PartialEq)]
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

    pub fn write_packed(&self, packed_data: &mut Vec<u8>) {
        let mut num_non_zeros = 0_usize;
        let mut packing_mask: u8 = 0b_1000_0000;

        if self.note != 0 {
            num_non_zeros += 1;
            packing_mask |= 0b_00001;
        }

        if self.instrument != 0 {
            num_non_zeros += 1;
            packing_mask |= 0b_00010;
        }

        if self.volume != 0 {
            num_non_zeros += 1;
            packing_mask |= 0b_00100;
        }

        if self.effect_type != 0 {
            num_non_zeros += 1;
            packing_mask |= 0b_01000;
        }

        if self.effect_param != 0 {
            num_non_zeros += 1;
            packing_mask |= 0b_10000;
        }

        if num_non_zeros < 4 {
            packed_data.push(packing_mask);

            if self.note != 0 {
                packed_data.push(self.note);
            }

            if self.instrument != 0 {
                packed_data.push(self.instrument);
            }

            if self.volume != 0 {
                packed_data.push(self.volume);
            }

            if self.effect_type != 0 {
                packed_data.push(self.effect_type);
            }

            if self.effect_param != 0 {
                packed_data.push(self.effect_param);
            }
        } else {
            packed_data.push(self.note);
            packed_data.push(self.instrument);
            packed_data.push(self.volume);
            packed_data.push(self.effect_type);
            packed_data.push(self.effect_param);
        }
    }
}
