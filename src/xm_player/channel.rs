use std::ops::BitAnd;
use std::rc::Rc;

use super::Envelope;
use super::Instrument;
use super::LoopType;
use super::Module;
use super::NibbleTest;
use super::Row;
use super::Sample;

fn get_frequency(period: f32) -> f32 {
    8363.0 * 2.0f32.powf((1152.0 - period) / 192.0)
}

fn get_period_and_frequency(note: f32) -> (f32, f32) {
    let period = 1920.0 - note * 16.0;
    let frequency = 8363.0 * 2.0f32.powf((1152.0 - period) / 192.0);
    (period, frequency)
}

fn slide_towards(mut value: f32, target: f32, step: f32) -> f32 {
    if value < target {
        value = target.min(value + step);
    } else if value > target {
        value = target.max(value - step);
    }

    value
}

fn follow_envelope(mut ticks: usize, note_released: bool, envelope: &Envelope) -> usize {
    if envelope.tick_values.is_empty() {
        return ticks;
    }

    if !note_released {
        if ticks < envelope.sustain as usize {
            ticks += 1
        }
    } else {
        ticks += 1;
        if ticks == envelope.loop_end {
            ticks = envelope.loop_start;
        }
    }

    ticks
}

pub struct Channel<'a> {
    module: &'a Module,
    inv_sample_rate: f32,
    pub row: Row,
    sample: Option<Rc<Sample>>,
    instrument: Option<Rc<Instrument>>,
    note: f32,
    note_volume: usize,
    note_panning: usize,
    note_period: f32,
    note_target_period: f32,
    note_frequency: f32,
    note_step: f32,
    note_released: bool,
    loop_dir_forward: bool,
    volume_envelope_ticks: usize,
    panning_envelope_ticks: usize,
    last_nonzero_effect_param: u8,
    sample_offset: f32,
    final_volume: usize,
    final_panning: usize,
}

macro_rules! render_samples {
    ($buffer:ident, $sample:ident, $offset:ident, $volume_left:expr, $volume_right:expr, $step:expr, $test:block) => {
        let mut dst = $buffer.as_mut_ptr();
        let end = dst.add($buffer.len());

        if ($step) < 1.0 {
            let mut v = 0;
            let mut o = usize::MAX;

            // Step back
            $offset -= $step;

            while dst < end {
                $offset += $step;
                if ($offset as usize) != o {
                    $test;
                    o = $offset as usize;
                    v = *$sample.data.get_unchecked(o) as i32;
                    v = ((v.unchecked_mul($volume_left).unchecked_shr(8)) & 0x0000FFFF)
                        | ((v.saturating_mul($volume_right).unchecked_shr(8)).unchecked_shl(16));
                }

                *dst = v;
                dst = dst.add(1);
            }
        } else {
            while dst < end {
                let v = *$sample.data.get_unchecked($offset as usize) as i32;
                *dst = ((v.unchecked_mul($volume_left).unchecked_shr(8)) & 0x0000FFFF)
                    | ((v.saturating_mul($volume_right).unchecked_shr(8)).unchecked_shl(16));

                $offset += $step;
                $test;

                dst = dst.add(1);
            }
        }
    };
}

impl<'a> Channel<'a> {
    pub fn new(module: &'a Module, sample_rate: usize) -> Self {
        Channel {
            module,
            inv_sample_rate: 1.0 / (sample_rate as f32),
            row: Row::default(),
            sample: None,
            instrument: None,
            note: 0.0,
            note_volume: 0,
            note_panning: 0,
            note_period: 0.0,
            note_target_period: 0.0,
            note_frequency: 0.0,
            note_step: 0.0,
            note_released: false,
            loop_dir_forward: true,
            volume_envelope_ticks: 0,
            panning_envelope_ticks: 0,
            last_nonzero_effect_param: 0,
            sample_offset: 0.0,
            final_volume: 0,
            final_panning: 0,
        }
    }

    fn is_note_active(&self) -> bool {
        self.instrument.is_some() && self.sample.is_some()
    }

    fn get_active_instrument_with_sample(&self) -> Option<(Rc<Instrument>, Rc<Sample>)> {
        if self.instrument.is_some() && self.sample.is_some() {
            Some((
                self.instrument.as_ref().unwrap().clone(),
                self.sample.as_ref().unwrap().clone(),
            ))
        } else {
            None
        }
    }

    fn note_on(
        &mut self,
        note: u8,
        keep_period: bool,
        keep_volume: bool,
        keep_position: bool,
        keep_envelope: bool,
    ) {
        if let Some((instrument, sample)) = self.get_active_instrument_with_sample() {
            self.note = sample.get_adjusted_note(note);

            if keep_period {
                (self.note_target_period, self.note_frequency) =
                    get_period_and_frequency(self.note);
            } else {
                (self.note_period, self.note_frequency) = get_period_and_frequency(self.note);
                self.note_target_period = self.note_period;
            }

            self.note_step = self.note_frequency * self.inv_sample_rate;

            if !keep_volume {
                self.note_volume = sample.volume as usize;
            }

            if !keep_position {
                self.sample_offset = 0.0;
                self.note_released = false;
                self.loop_dir_forward = true;
            }

            self.note_panning = sample.panning as usize;
            self.volume_envelope_ticks = 0;
            self.panning_envelope_ticks = 0;

            /*
             */
        }
    }

    fn note_off(&mut self) {
        self.note_released = true;
    }

    fn note_kill(&mut self) {
        self.note_released = true;
        self.instrument = None;
        self.sample = None;
    }

    fn tick_new_row(&mut self, row: Row) {
        let mut trigger_note = true;
        let mut keep_period = false;
        let mut keep_volume = false;
        let mut keep_position = false;
        let mut keep_envelope = false;

        /*
        // ?!
        if self.row.has_portamento() && self.is_note_active() {
            keep_period = true;
            keep_position = true;
        }
        */

        if row.instrument > 0 {
            // Instrument without note - sample position is kept, only envelopes are reset
            if row.note == 0 && self.is_note_active() {
                keep_position = true;
                trigger_note = true;
            }
            // Select new instrument and sample
            else if let Some(instrument) =
                self.module.get_instrument((row.instrument - 1) as usize)
            {
                if let Some(sample) = instrument.get_note_sample(row.note as usize) {
                    self.instrument = Some(instrument);
                    self.sample = Some(sample);
                } else {
                    // Invalid note sample (missing?)
                    self.note_kill();
                }
            } else {
                // Invalid instrument
                self.note_kill();
            }
        } else {
            keep_volume = true;
        }

        if row.has_valid_note() && self.is_note_active() {
            if row.has_portamento() {
                keep_period = true;
                keep_volume = true;
                keep_position = true;
            }

            self.note_on(
                row.note,
                keep_period,
                keep_volume,
                keep_position,
                keep_envelope,
            );
        } else if row.is_note_off() {
            self.note_off();
        }

        self.row = row;
    }

    fn apply_volume(&mut self, row_tick_index: usize) {
        if row_tick_index == 0 {
            // Set initial note volume
            if self.row.volume >= 0x10 && self.row.volume <= 0x50 {
                self.note_volume *= (self.row.volume - 16) as usize;
                self.note_volume /= 64;
            }
            // Set initial note panning
            else if self.row.volume >= 0xC0 && self.row.volume <= 0xCF {
                self.note_panning = ((self.row.volume & 0x0F) * 17) as usize;
            }
        }
    }

    fn apply_effects(&mut self, row_tick_index: usize) {
        // Volume slide down (or fine slide down)
        if self.row.volume.test_high_nibble(0x60)
            || (self.row.volume.test_high_nibble(0x80) && row_tick_index == 0)
        {
            self.note_volume = self
                .note_volume
                .saturating_sub(self.row.volume.bitand(0x0F) as usize);
        }
        // Volume slide up (or fine slide up)
        else if self.row.volume.test_high_nibble(0x70)
            || (self.row.volume.test_high_nibble(0x90) && row_tick_index == 0)
        {
            self.note_volume += self.row.volume.bitand(0x0F) as usize;
            self.note_volume = self.note_volume.clamp(0, 64);
        }

        if self.row.effect_param != 0 {
            self.last_nonzero_effect_param = self.row.effect_param;
        }

        match self.row.effect_type {
            // Set panning
            0x08 => {
                self.note_panning = self.row.effect_param as usize;
            }
            // Volume slide
            0x0A => {
                self.note_volume = self
                    .note_volume
                    .saturating_sub(self.last_nonzero_effect_param as usize);
            }
            // Tone portamento
            0x03 => {
                //
            }
            _ => {}
        }

        if self.note_period != self.note_target_period {
            self.note_period = slide_towards(
                self.note_period,
                self.note_target_period,
                1.0 * (self.last_nonzero_effect_param as f32),
            );

            self.note_frequency = get_frequency(self.note_period);
            self.note_step = self.note_frequency * self.inv_sample_rate;
        }

        self.final_volume = self.note_volume;
        self.final_panning = self.note_panning;
    }

    fn tick_envelopes(&mut self) {
        if let Some(instrument) = self.instrument.clone() {
            self.volume_envelope_ticks = follow_envelope(
                self.volume_envelope_ticks,
                self.note_released,
                &instrument.volume_envelope,
            );

            let volume = instrument
                .volume_envelope
                .get_value(self.volume_envelope_ticks) as usize;

            self.final_volume = ((self.note_volume as usize) * volume) / 64;

            self.panning_envelope_ticks = follow_envelope(
                self.panning_envelope_ticks,
                self.note_released,
                &instrument.panning_envelope,
            );

            let panning = 4
                * (instrument
                    .panning_envelope
                    .get_value(self.panning_envelope_ticks) as usize);

            //self.final_panning =
        }
    }

    pub fn reset(&mut self) {
        self.note_kill();
    }

    pub fn tick(&mut self, mut row: Row, row_tick_index: usize, buffer: &mut [i16]) {
        if row_tick_index == 0 {
            self.tick_new_row(row);
        }

        self.apply_volume(row_tick_index);
        self.apply_effects(row_tick_index);
        self.tick_envelopes();

        if let Some(sample) = self.sample.clone() {
            let mut offset = self.sample_offset;
            let step = self.note_step;

            let mut vr = self.final_panning.clamp(0, 255) as i32;
            let mut vl = 255 - vr;

            vr = (vr * (self.final_volume as i32)) / 64;
            vl = (vl * (self.final_volume as i32)) / 64;

            unsafe {
                let (_, bufferi32, _) = buffer.align_to_mut::<i32>();

                // Can we use fast path for mixing? Using fast path means we can safely forward the sample
                // on every buffer element and not worry about hitting loop boundaries.
                let use_fast_path = match sample.loop_type {
                    LoopType::None => {
                        (offset + (bufferi32.len() as f32) * step) < sample.sample_end
                    }
                    LoopType::Forward => {
                        (offset + (bufferi32.len() as f32) * step) < sample.loop_end
                    }
                    LoopType::PingPong => {
                        self.loop_dir_forward
                            && ((offset + (bufferi32.len() as f32) * step) < sample.loop_end)
                    }
                };

                if use_fast_path {
                    render_samples!(bufferi32, sample, offset, vl, vr, step, {});
                } else {
                    match sample.loop_type {
                        LoopType::None => {
                            bufferi32.fill(0);

                            render_samples!(bufferi32, sample, offset, vl, vr, step, {
                                if offset >= sample.sample_end {
                                    break;
                                }
                            });

                            self.note_kill();
                        }
                        // Orig benchmark time: 323ms
                        LoopType::Forward => {
                            render_samples!(bufferi32, sample, offset, vl, vr, step, {
                                if offset >= sample.loop_end {
                                    offset = sample.loop_start;
                                }
                            });
                        }
                        LoopType::PingPong => {
                            for f in bufferi32 {
                                let v = *sample.data.get_unchecked(offset as usize) as i32;
                                *f = (((v * vl) / 256) & 0x0000FFFF) | (((v * vr) / 256) << 16);

                                if self.loop_dir_forward {
                                    offset += step;
                                    if offset >= sample.loop_end {
                                        offset = sample.loop_end - 1.0;
                                        self.loop_dir_forward = false;
                                    }
                                } else {
                                    offset -= step;
                                    if offset < sample.loop_start {
                                        offset = sample.loop_start;
                                        self.loop_dir_forward = true;
                                    }
                                }
                            }
                        }
                    }
                }

                self.sample_offset = offset;
            }
        }
        // No active sample playing on this channel right now
        else {
            buffer.fill(0);
        }
    }
}
