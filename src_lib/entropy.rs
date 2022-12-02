pub struct RangeEncoder<'a> {
    output: &'a mut Vec<u8>,
    symbols_freq: Vec<usize>,
    symbol_lows: Vec<usize>,
    total_freq: usize,
    low: u32,
    range: u32,
}

const RANGE_TOP: u32 = 1 << 24;
const RANGE_BOTTOM: u32 = 1 << 16;

impl<'a> RangeEncoder<'a> {
    pub fn new(output: &'a mut Vec<u8>, freqs: &[usize]) -> Self {
        let mut result = Self {
            output,
            symbols_freq: freqs.to_vec(),
            symbol_lows: Vec::with_capacity(freqs.len()),
            total_freq: 0,
            low: 0,
            range: u32::MAX,
        };

        for &f in freqs {
            result.symbol_lows.push(result.total_freq);
            result.total_freq += f;
        }

        if false {
            let mut num_zeros: u8 = 0;
            for &f in freqs {
                if f == 0 {
                    num_zeros += 1;

                    if num_zeros == 127 {
                        result.output.push(num_zeros | 0b1000_0000);
                        num_zeros = 0;
                    }
                } else {
                    if num_zeros > 0 {
                        result.output.push(num_zeros | 0b1000_0000);
                        num_zeros = 0;
                    }

                    result.output.push(f as u8);
                }
            }

            if num_zeros > 0 {
                result.output.push(num_zeros | 0b1000_0000);
            }
        }

        result
    }

    pub fn encode(&mut self, symbol_index: usize) {
        let symbol_freq = self.symbols_freq[symbol_index];
        let symbol_low = self.symbol_lows[symbol_index];

        self.range /= self.total_freq as u32;
        self.low += (symbol_low * self.range as usize) as u32;
        self.range *= symbol_freq as u32;

        loop {
            let mut emit_byte = (self.low ^ (self.low.overflowing_add(self.range).0)) < RANGE_TOP;

            if !emit_byte && self.range < RANGE_BOTTOM {
                self.range = RANGE_BOTTOM - (self.low & (RANGE_BOTTOM - 1));
                emit_byte = true;
            }

            if emit_byte {
                self.output.push((self.low >> 24) as u8);
                self.low <<= 8;
                self.range <<= 8;
            } else {
                break;
            }
        }
    }
}
