use std::ops::{BitAnd, Shr};

pub struct BitWriter<'a> {
    output: &'a mut Vec<u8>,
    bit_pos: u8,
}

impl<'a> BitWriter<'a> {
    pub fn new(output: &'a mut Vec<u8>) -> Self {
        Self { output, bit_pos: 0 }
    }

    pub fn write<T>(&mut self, mut data: T, mut num_bits: u8)
    where
        T: Copy + BitAnd<u8, Output = u8> + Shr<u8, Output = T>,
    {
        while num_bits > 0 {
            let bits_left_in_byte = 8 - self.bit_pos;
            let bits_to_write = core::cmp::min(num_bits, bits_left_in_byte);

            let data_mask = ((1u16 << bits_to_write) - 1) as u8;
            let data_to_write = data.bitand(0xFF as u8) & data_mask;

            if self.bit_pos == 0 {
                self.output.push(data_to_write);
            } else {
                let last_byte = self.output.last_mut().unwrap();
                *last_byte |= data_to_write << self.bit_pos;
            }

            self.bit_pos = (self.bit_pos + bits_to_write) % 8;

            num_bits -= bits_to_write;
            data = data.shr(bits_to_write);
        }
    }

    pub fn write_bit(&mut self, bit: bool) {
        if bit {
            if self.bit_pos == 0 {
                self.output.push(1);
            } else {
                let last_byte = self.output.last_mut().unwrap();
                *last_byte |= 1 << self.bit_pos;
            }
        }

        self.bit_pos = (self.bit_pos + 1) % 8;
    }

    pub fn write_u8(&mut self, value: u8) {
        self.write(value, 8);
    }
}
