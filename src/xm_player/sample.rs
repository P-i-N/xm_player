use std::error;

use super::BinaryReader;
use super::FormatError;

#[derive(Clone, Default)]
pub enum LoopType {
    #[default]
    None,
    Forward,
    PingPong,
}

#[derive(Clone, Default)]
pub struct Sample {
    pub name: String,
    pub data: Vec<i16>,
    pub is_16bit: bool,
    pub adpcm: bool,
    pub loop_type: LoopType,
    pub loop_start: usize,
    pub loop_length: usize,
    pub volume: u8,
    pub panning: u8,
    pub relative_note: i8,
    pub finetune: i8,
}

impl Sample {
    pub fn new(br: &mut BinaryReader, data_pos: usize) -> Result<Sample, Box<dyn error::Error>> {
        let mut result = Sample::default();

        let mut sample_length = br.read_u32() as usize;

        result.loop_start = br.read_u32() as usize;
        result.loop_length = br.read_u32() as usize;
        result.volume = br.read_u8();
        result.finetune = br.read_u8() as i8;

        let flags = br.read_u8();

        result.is_16bit = (flags & 0b10000) != 0;
        if result.is_16bit {
            sample_length >>= 1;
        }

        result.loop_type = match flags & 0x3 {
            0 => Ok(LoopType::None),
            1 => Ok(LoopType::Forward),
            2 => Ok(LoopType::PingPong),
            _ => Err(Box::new(FormatError::new("Invalid sample loop type"))),
        }?;

        result.data.resize(sample_length, 0);

        result.panning = br.read_u8();
        result.relative_note = br.read_u8() as i8;

        let compression_type = br.read_u8();
        result.adpcm = compression_type == 0xAD;

        result.name = br.read_string_segment(22).trim().to_string();

        br.pos = data_pos;
        result.read_samples(br)?;

        Ok(result)
    }

    pub fn read_samples(&mut self, br: &mut BinaryReader) -> Result<(), Box<dyn error::Error>> {
        if !self.adpcm {
            if self.is_16bit {
                for _ in 0..self.data.len() {
                    //
                }

                return Err(Box::new(FormatError::new("Not implemented!")));
            } else {
                let mut acc: u8 = 0;
                for i in 0..self.data.len() {
                    (acc, _) = acc.overflowing_add(br.read_u8());
                    let mut sample = acc as i16;
                    if (sample & 128) != 0 {
                        sample = sample - 256;
                    }

                    self.data[i] = sample * 16;
                }
            }
        } else {
            // ADPCM compression
        }

        Ok(())
    }
}
