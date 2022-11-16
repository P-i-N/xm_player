use super::math::*;
use super::BinaryReader;
use super::BinaryWriter;
use super::Row;

pub enum SymbolPrefix {
    Dictionary,
    Reference,
    RLE,
    RowEvent,
}

pub trait SymbolPrefixBits {
    fn symbol_prefix(&self) -> SymbolPrefix;
}

impl SymbolPrefixBits for u8 {
    fn symbol_prefix(&self) -> SymbolPrefix {
        if (self & 0b_1000_0000) == 0 {
            return SymbolPrefix::Dictionary;
        } else if (self & 0b_1100_0000) == 0b_1000_0000 {
            return SymbolPrefix::Reference;
        } else if (self & 0b_1110_0000) == 0b_1100_0000 {
            return SymbolPrefix::RLE;
        }

        SymbolPrefix::RowEvent
    }
}

pub trait SymbolEncodingSize {
    fn encoding_size(&self) -> usize;
}

impl SymbolEncodingSize for [Symbol] {
    fn encoding_size(&self) -> usize {
        let mut size = 0;

        for symbol in self {
            size += symbol.encoding_size();
        }

        size
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum Symbol {
    Unknown,
    Dictionary(u8),
    Reference(u8),
    RLE(u16),
    RowEvent(Row),
}

impl Symbol {
    pub fn is_row_event_or_dictionary(&self) -> bool {
        match self {
            Symbol::RowEvent(_) | Symbol::Dictionary(_) => true,
            _ => false,
        }
    }

    pub fn is_rle(&self) -> bool {
        match self {
            Symbol::RLE(_) => true,
            _ => false,
        }
    }

    pub fn is_reference(&self) -> bool {
        match self {
            Symbol::Reference(_) => true,
            _ => false,
        }
    }

    pub fn read(&mut self, br: &mut BinaryReader) {
        let mut b = br.read_u8();

        match b.symbol_prefix() {
            SymbolPrefix::Dictionary => {
                *self = Symbol::Dictionary(b);
            }
            SymbolPrefix::Reference => {
                *self = Symbol::Reference(b & 0b_0011_1111);
            }
            SymbolPrefix::RLE => {
                let mut length = 0;
                let mut num_rle_symbols = 0;

                loop {
                    length |= ((b & 0b_0001_1111) as u16) << (num_rle_symbols * 5);
                    match br.peek_u8().symbol_prefix() {
                        SymbolPrefix::RLE => {}
                        _ => break,
                    }

                    b = br.read_u8();
                    num_rle_symbols += 1;
                }

                *self = Symbol::RLE(length);
            }
            SymbolPrefix::RowEvent => {
                let mut row = Row::new();

                if (b & 0b_0000_0001) == 0b_0000_0001 {
                    row.note = br.read_u8();
                }

                if (b & 0b_0000_0010) == 0b_0000_0010 {
                    row.instrument = br.read_u8();
                }

                if (b & 0b_0000_0100) == 0b_0000_0100 {
                    row.volume = br.read_u8();
                }

                if (b & 0b_0000_1000) == 0b_0000_1000 {
                    row.effect_type = br.read_u8();
                }

                if (b & 0b_0001_0000) == 0b_0001_0000 {
                    row.effect_param = br.read_u8();
                }

                *self = Symbol::RowEvent(row);
            }
        }
    }

    pub fn write(&self, bw: &mut BinaryWriter) {
        match self {
            Symbol::Dictionary(dict) => {
                assert!(*dict < 128);
                bw.write_u8(*dict);
            }
            Symbol::Reference(index) => {
                assert!(*index < 64);
                bw.write_u8(0b_1000_0000 | (*index));
            }
            Symbol::RLE(length) => {
                let mut l = *length;
                while l > 0 {
                    bw.write_u8(0b_1100_0000 | ((l & 0b_0001_1111) as u8));
                    l >>= 5;
                }
            }
            Symbol::RowEvent(row) => {
                let mut packing_mask: u8 = 0b_1110_0000;

                if row.note != 0 {
                    packing_mask |= 0b_0000_0001;
                }

                if row.instrument != 0 {
                    packing_mask |= 0b_0000_0010;
                }

                if row.volume != 0 {
                    packing_mask |= 0b_0000_0100;
                }

                if row.effect_type != 0 {
                    packing_mask |= 0b_0000_1000;
                }

                if row.effect_param != 0 {
                    packing_mask |= 0b_0001_0000;
                }

                bw.write_u8(packing_mask);

                if row.note != 0 {
                    bw.write_u8(row.note);
                }

                if row.instrument != 0 {
                    bw.write_u8(row.instrument);
                }

                if row.volume != 0 {
                    bw.write_u8(row.volume);
                }

                if row.effect_type != 0 {
                    bw.write_u8(row.effect_type);
                }

                if row.effect_param != 0 {
                    bw.write_u8(row.effect_param);
                }
            }

            Symbol::Unknown => {}
        }
    }
}

impl SymbolEncodingSize for Symbol {
    fn encoding_size(&self) -> usize {
        match self {
            Symbol::Dictionary(_) => 1,
            Symbol::Reference(_) => 1,
            Symbol::RLE(length) => {
                if *length <= 32 {
                    1
                } else if *length <= 1024 {
                    2
                } else {
                    3
                }
            }
            Symbol::RowEvent(row) => {
                let mut num_non_zeros = 1_u8;

                num_non_zeros += sign_u8(row.note);
                num_non_zeros += sign_u8(row.instrument);
                num_non_zeros += sign_u8(row.volume);
                num_non_zeros += sign_u8(row.effect_type);
                num_non_zeros += sign_u8(row.effect_param);

                num_non_zeros as usize
            }
            Symbol::Unknown => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Row;
    use super::Symbol;
    use crate::BinaryReader;
    use crate::BinaryWriter;

    fn test_write_read_eq(x: &Symbol) -> bool {
        let mut data = Vec::new();
        let mut y = Symbol::RowEvent(Row::new());

        // Write
        {
            let mut bw = BinaryWriter::new(&mut data);
            x.write(&mut bw);
        }

        // Read
        {
            let mut br = BinaryReader::new(&data);
            y.read(&mut br);
        }

        *x == y
    }

    #[test]
    pub fn write_read_eq() {
        assert!(test_write_read_eq(&Symbol::Dictionary(16)));
        assert!(test_write_read_eq(&Symbol::Reference(33)));
        assert!(test_write_read_eq(&Symbol::RLE(5)));
        assert!(test_write_read_eq(&Symbol::RowEvent(Row::new())));
    }
}
