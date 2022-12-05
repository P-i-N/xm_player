extern crate core;

use core::mem::transmute;

pub struct BinaryReader<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> BinaryReader<'a> {
    pub fn new(data: &'a [u8]) -> BinaryReader {
        BinaryReader { data, pos: 0 }
    }

    pub fn skip(&mut self, num_bytes: usize) {
        self.pos = (self.pos + num_bytes).min(self.data.len());
    }

    pub fn align_to(&mut self, alignment: usize) {
        let padding = (alignment - (self.pos % alignment)) % alignment;
        self.pos += padding;
    }

    pub fn align_to_struct<T>(&mut self) {
        self.align_to(core::mem::align_of::<T>());
    }

    pub fn peek_u8(&self) -> u8 {
        if self.pos >= self.data.len() {
            return 0;
        }

        self.data[self.pos]
    }

    pub fn read_u8(&mut self) -> u8 {
        if self.pos >= self.data.len() {
            return 0;
        }

        let value = self.data[self.pos];
        self.pos += 1;
        value
    }

    pub fn read_i8(&mut self) -> i8 {
        unsafe { transmute::<u8, i8>(self.read_u8()) }
    }

    pub fn read_u16(&mut self) -> u16 {
        if self.pos + 1 >= self.data.len() {
            return 0;
        }

        let value = u16::from(self.data[self.pos + 1]) << 8 | u16::from(self.data[self.pos]);
        self.pos += 2;
        value
    }

    pub fn read_i16(&mut self) -> i16 {
        unsafe { transmute::<u16, i16>(self.read_u16()) }
    }

    pub fn read_u32(&mut self) -> u32 {
        if self.pos + 3 >= self.data.len() {
            return 0;
        }

        let value = ((self.data[self.pos + 3] as u32) << 24)
            | ((self.data[self.pos + 2] as u32) << 16)
            | ((self.data[self.pos + 1] as u32) << 8)
            | (self.data[self.pos] as u32);

        self.pos += 4;
        value
    }
}
