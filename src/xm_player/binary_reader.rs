//use std::str::from_utf8;

pub struct BinaryReader<'a> {
    pub data: &'a [u8],
    pub pos: usize,
}

impl<'a> BinaryReader<'a> {
    pub fn new(data: &'a [u8]) -> BinaryReader {
        BinaryReader { data, pos: 0 }
    }

    pub fn read_u8(&mut self) -> u8 {
        if self.pos >= self.data.len() {
            return 0;
        }

        let value = self.data[self.pos];
        self.pos += 1;
        value
    }

    pub fn read_u16(&mut self) -> u16 {
        if self.pos + 1 >= self.data.len() {
            return 0;
        }

        let value = u16::from(self.data[self.pos + 1]) << 8 | u16::from(self.data[self.pos]);
        self.pos += 2;
        value
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

    pub fn read_string_segment(&mut self, segment_length: usize) -> String {
        if self.pos + segment_length - 1 >= self.data.len() {
            String::default()
        } else {
            let slice = &self.data[self.pos..self.pos + segment_length];

            // We have to iterate every byte and strictly convert to ASCII, because
            // some modules have f*cked up sample & instrument names, that do not
            // map nicely to UTF-8 strings
            let value =
                String::from_iter(slice.iter().map(
                    |&ch| {
                        if ch < 128 {
                            ch as char
                        } else {
                            32 as char
                        }
                    },
                ));

            self.pos += segment_length;
            value
        }
    }
}
