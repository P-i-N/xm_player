use std::cell::RefCell;
use std::error;
use std::ops::BitAnd;
use std::rc::Rc;
use std::rc::Weak;

use super::BinaryReader;
use super::Envelope;
use super::FormatError;
use super::Sample;

#[derive(Clone, Default)]
pub struct Instrument {
    pub name: String,
    pub samples: Vec<Rc<Sample>>,
    pub sample_keymap: Vec<Weak<Sample>>,
    pub volume_envelope: Envelope,
    pub panning_envelope: Envelope,
    pub vibrato_type: u8,
    pub vibrato_sweep: u8,
    pub vibrato_depth: u8,
    pub vibrato_rate: u8,
}

impl Instrument {
    pub fn parse(&mut self, br: &mut BinaryReader) -> Result<(), Box<dyn error::Error>> {
        let mut instrument_size = br.read_u32() as usize;
        if instrument_size == 0 || instrument_size > 263 {
            instrument_size = 263;
        }

        let skip_pos = br.pos + instrument_size - 4;

        self.name = br.read_string_segment(22).trim().to_string();

        // Instrument type, no meaning
        br.read_u8();

        let num_samples = br.read_u16() as usize;

        self.samples.clear();
        self.sample_keymap.clear();

        if num_samples > 0 {
            let _sample_header_size = br.read_u32() as usize;

            let mut sample_index_keymap = [0 as u8; 96];
            for i in &mut sample_index_keymap {
                *i = br.read_u8();
            }

            let mut volume_env_points = [0 as usize; 24];
            let mut panning_env_points = [0 as usize; 24];

            // Volume envelope points
            for i in 0..24 {
                volume_env_points[i] = br.read_u16() as usize;
            }

            // Panning envelope points
            for i in 0..24 {
                panning_env_points[i] = br.read_u16() as usize;
            }

            let num_volume_points = br.read_u8() as usize;
            let num_panning_points = br.read_u8() as usize;

            self.volume_envelope.sustain = br.read_u8() as usize;
            self.volume_envelope.loop_start = br.read_u8() as usize;
            self.volume_envelope.loop_end = br.read_u8() as usize;

            self.panning_envelope.sustain = br.read_u8() as usize;
            self.panning_envelope.loop_start = br.read_u8() as usize;
            self.panning_envelope.loop_end = br.read_u8() as usize;

            let volume_flags = br.read_u8();
            let panning_flags = br.read_u8();

            self.volume_envelope.build(
                &volume_env_points[0..num_volume_points * 2],
                (volume_flags & 2) != 0,
                (volume_flags & 4) != 0,
            );
            self.panning_envelope.build(
                &panning_env_points[0..num_panning_points * 2],
                (panning_flags & 2) != 0,
                (panning_flags & 4) != 0,
            );

            self.vibrato_type = br.read_u8();
            self.vibrato_sweep = br.read_u8();
            self.vibrato_depth = br.read_u8();
            self.vibrato_rate = br.read_u8();

            self.volume_envelope.fadeout = br.read_u16();

            // Reserved, unused
            br.pos += 22;

            let first_sample_header_pos = br.pos;
            let mut sample_data_pos = br.pos + num_samples * 40;

            // Read all samples
            for i in 0..num_samples {
                // Seek binary reader to start of sample header
                br.pos = first_sample_header_pos + i * 40;

                self.samples
                    .push(Rc::new(Sample::new(br, sample_data_pos)?));

                // Current binary reader position is start of next sample data position
                sample_data_pos = br.pos;
            }

            // Build sample keymap from previously loaded indices
            for i in sample_index_keymap {
                if (i as usize) < num_samples {
                    self.sample_keymap
                        .push(Rc::downgrade(&self.samples[i as usize]));
                } else {
                    // Invalid sample index, probably a corrupted file?
                    self.sample_keymap.push(Weak::new());
                }
            }
        } else {
            // There are no samples, so sample keymap should be full of 'None's
            for _ in 0..96 {
                self.sample_keymap.push(Weak::new());
            }

            br.pos = skip_pos;
        }

        Ok(())
    }

    pub fn get_note_sample(&self, note: usize) -> Option<Rc<Sample>> {
        if note >= self.sample_keymap.len() {
            None
        } else {
            self.sample_keymap[note].upgrade()
        }
    }
}
