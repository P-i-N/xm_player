use super::{NibbleTest, Vec};

#[derive(Clone, Copy, PartialEq, Hash)]
pub struct Row {
    pub note: u8,
    pub instrument: u8,
    pub volume: u8,
    pub effect_type: u8,
    pub effect_param: u8,
}

impl Row {
    pub fn new() -> Self {
        Row {
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

    pub fn is_empty(&self) -> bool {
        self.note == 0
            && self.instrument == 0
            && self.volume == 0
            && self.effect_type == 0
            && self.effect_param == 0
    }

    pub fn is_full(&self) -> bool {
        self.note != 0
            && self.instrument != 0
            && self.volume != 0
            && self.effect_type != 0
            && self.effect_param != 0
    }

    pub fn get_encoding_size(&self) -> usize {
        // Dictionary index
        if self.note >= 98 && self.note < 128 {
            return 1;
        }
        // Back reference
        else if (self.note & 0b_1010_0000) == 0b_1010_0000 {
            return 3;
        }
        // RLE
        else if (self.note & 0b_1100_0000) == 0b_1100_0000 {
            return 1;
        }

        let mut num_non_zeros = 1_usize;

        if self.instrument != 0 {
            num_non_zeros += 1;
        }

        if self.volume != 0 {
            num_non_zeros += 1;
        }

        if self.effect_type != 0 {
            num_non_zeros += 1;
        }

        if self.effect_param != 0 {
            num_non_zeros += 1;
        }

        num_non_zeros
    }

    pub fn write_packed(&self, packed_data: &mut Vec<u8>) {
        // Dictionary index
        if self.note >= 98 && self.note < 128 {
            packed_data.push(self.note);
            return;
        }
        // Back reference
        else if (self.note & 0b_1010_0000) == 0b_1010_0000 {
            packed_data.push(self.note);
            packed_data.push(self.instrument);
            packed_data.push(self.volume);
            return;
        }
        // RLE
        else if (self.note & 0b_1100_0000) == 0b_1100_0000 {
            packed_data.push(self.note);
            return;
        }

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
