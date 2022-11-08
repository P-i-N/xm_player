use super::{math::*, BinaryReader, Box, Error, FormatError, String, Vec};

#[derive(Clone, Default)]
pub enum LoopType {
    #[default]
    None,
    Forward,
    PingPong,
}

#[derive(Default)]
pub struct Sample {
    pub name: String,
    pub data: Vec<i16>,
    pub loop_type: LoopType,
    pub loop_start: u32,
    pub loop_end: u32,
    pub sample_end: u32,
    pub volume: u8,
    pub panning: u8,
    pub relative_note: i8,
    pub finetune: f32,
}

impl Sample {
    pub fn new(br: &mut BinaryReader, data_pos: usize) -> Result<Sample, Box<dyn Error>> {
        let mut result = Sample::default();

        let mut sample_length = br.read_u32() as usize;

        result.loop_start = br.read_u32();
        result.loop_end = result.loop_start + br.read_u32();
        result.volume = br.read_u8();
        result.finetune = (br.read_i8() as f32) / 128.0;

        let flags = br.read_u8();

        let is_16bit = (flags & 0b10000) != 0;
        if is_16bit {
            sample_length >>= 1;
            result.loop_start = result.loop_start / 2;
            result.loop_end = result.loop_end / 2;
        }

        result.loop_type = match flags & 0x3 {
            0 => Ok(LoopType::None),
            1 => Ok(LoopType::Forward),
            2 => Ok(LoopType::PingPong),
            _ => Err(Box::new(FormatError::new("Invalid sample loop type"))),
        }?;

        result.data.resize(sample_length, 0);
        result.sample_end = sample_length as u32;

        result.panning = br.read_u8();
        result.relative_note = br.read_i8();

        let compression_type = br.read_u8();

        // Skip sample name
        br.pos += 22;

        br.pos = data_pos;
        result.read_samples(br, compression_type == 0xAD, is_16bit)?;

        Ok(result)
    }

    pub fn read_samples(
        &mut self,
        br: &mut BinaryReader,
        adpcm: bool,
        is_16bit: bool,
    ) -> Result<(), Box<dyn Error>> {
        if !adpcm {
            if is_16bit {
                let mut acc: i16 = 0;
                for i in 0..self.data.len() {
                    (acc, _) = acc.overflowing_add(br.read_i16());
                    self.data[i] = acc;
                }
            } else {
                let mut acc: i8 = 0;
                for i in 0..self.data.len() {
                    (acc, _) = acc.overflowing_add(br.read_i8());
                    self.data[i] = (acc as i16) * 255;
                }
            }
        } else {
            // ADPCM compression
            return Err(Box::new(FormatError::new("ADPCM not supported")));
        }

        Ok(())
    }

    pub fn get_adjusted_note(&self, note: u8) -> f32 {
        note as f32 + (self.relative_note as f32) + self.finetune - 1.0
    }
}
