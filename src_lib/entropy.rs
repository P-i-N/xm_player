use super::BitWriter;

pub struct RangeEncoder<'a> {
    pub bit_writer: &'a mut BitWriter<'a>,
    pub symbol_counts: Vec<usize>,
}

impl<'a> RangeEncoder<'a> {
    pub fn new(bit_writer: &'a mut BitWriter<'a>) -> Self {
        Self {
            bit_writer,
            symbol_counts: Vec::new(),
        }
    }

    pub fn encode(&mut self, symbol_index: usize) {
        self.bit_writer.write(symbol_index as u8, 8);
    }
}
