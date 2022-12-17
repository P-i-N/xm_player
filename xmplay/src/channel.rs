use super::math::*;
use super::player::SongState;
use super::ButterworthFilter;
use super::Fixed;
use super::Instrument;
use super::LoopType;
use super::Module;
use super::NibbleTest;
use super::Rc;
use super::Row;
use super::Sample;

fn get_frequency(period: f32) -> f32 {
    8363.0 * pow(2.0, (1152.0 - period) / 192.0)
}

fn get_period_and_frequency(note: f32) -> (f32, f32) {
    let period = 1920.0 - note * 16.0;
    let frequency = get_frequency(period);
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

fn get_vibrato_period(ticks: usize) -> f32 {
    -sin(3.14159265358f32 * 2.0f32 * (ticks as f32 / 64.0))
}

pub struct Channel<'a> {
    module: &'a Module<'a>,
    pub index: usize,
    pub mute: bool,
    sample_rate: f32,
    row: Row,
    sample: Option<Rc<Sample>>,
    instrument: Option<Rc<Instrument>>,
    note: f32,
    note_volume: usize,
    note_panning: usize,
    note_fadeout: i32,
    note_period: f32,
    note_target_period: f32,
    note_frequency: f32,
    note_step: f32,
    note_step_fp: Fixed,
    note_released: bool,
    loop_dir_forward: bool,
    volume_envelope_ticks: usize,
    panning_envelope_ticks: usize,
    vibrato_speed: u8,
    vibrato_depth: u8,
    vibrato_ticks: usize,
    vibrato_note_offset: f32,
    last_nonzero_effect_param: u8,
    sample_offset_fp: Fixed,
    final_volume: usize,
    final_panning: usize,
    pub filter: ButterworthFilter,
}

impl<'a> Channel<'a> {
    pub fn new(module: &'a Module, index: usize, sample_rate: usize) -> Self {
        Channel {
            module,
            index,
            mute: false,
            sample_rate: sample_rate as f32,
            row: Row::new(),
            sample: None,
            instrument: None,
            note: 0.0,
            note_volume: 0,
            note_panning: 0,
            note_fadeout: 65535,
            note_period: 0.0,
            note_target_period: 0.0,
            note_frequency: 0.0,
            note_step: 0.0,
            note_step_fp: Fixed::default(),
            note_released: false,
            loop_dir_forward: true,
            volume_envelope_ticks: 0,
            panning_envelope_ticks: 0,
            vibrato_speed: 0,
            vibrato_depth: 0,
            vibrato_ticks: 0,
            vibrato_note_offset: 0.0,
            last_nonzero_effect_param: 0,
            sample_offset_fp: Fixed::default(),
            final_volume: 0,
            final_panning: 0,
            filter: ButterworthFilter::new(),
        }
    }

    /// Returns true if the channel has an instrument and a sample assigned
    fn has_instrument_and_sample(&self) -> bool {
        self.instrument.is_some() && self.sample.is_some()
    }

    fn note_on(
        &mut self,
        note: u8,
        keep_period: bool,
        keep_volume: bool,
        keep_position: bool,
        keep_envelope: bool,
    ) {
        if let Some(sample) = &self.sample {
            self.note = sample.get_adjusted_note(note);

            if keep_period {
                (self.note_target_period, self.note_frequency) =
                    get_period_and_frequency(self.note);
            } else {
                (self.note_period, self.note_frequency) = get_period_and_frequency(self.note);
                self.note_target_period = self.note_period;
            }

            self.note_step = ((self.note_frequency as f64) / (self.sample_rate as f64)) as f32;
            self.note_step_fp = self.note_step.into();

            if !keep_volume {
                self.note_volume = sample.volume as usize;
                self.note_fadeout = 65536;
            }

            if !keep_position {
                self.sample_offset_fp = Fixed::default();
                self.note_released = false;
                self.loop_dir_forward = true;
            }

            self.note_panning = sample.panning as usize;
            self.vibrato_note_offset = 0.0;

            if !keep_envelope {
                self.volume_envelope_ticks = 0;
                self.panning_envelope_ticks = 0;
                self.vibrato_ticks = 0;
            }
        }
    }

    fn note_off(&mut self) {
        self.note_released = true;

        if let Some(instrument) = &self.instrument {
            if !instrument.volume_envelope.is_enabled() {
                self.note_kill();
            }
        } else {
            self.note_kill();
        }
    }

    fn note_kill(&mut self) {
        self.note_cut();
        self.note_released = true;
        self.instrument = None;
        self.sample = None;
    }

    fn note_cut(&mut self) {
        // This is NOT the same as note_kill, as it does not reset the instrument
        self.note_volume = 0;
    }

    fn tick_new_row_(&mut self, row: Row) {
        if row.instrument > 0 {
            if self.row.has_portamento() && self.has_instrument_and_sample() {
                self.note_on(self.row.note, true, false, true, false);
            } else if row.note == 0 && self.has_instrument_and_sample() {
                self.note_on(self.row.note, false, false, true, false);
            } else if (row.instrument as usize) > self.module.num_instruments {
                self.note_kill();
            } else {
                self.instrument = self.module.get_instrument((row.instrument - 1) as usize);
            }
        }

        if row.has_valid_note() {
            if self.row.has_portamento() && self.has_instrument_and_sample() {
                //
            } else if let Some(instrument) = &self.instrument {
                //
            } else {
                self.note_cut();
            }
        } else if row.is_note_off() {
            self.note_off();
        }
    }

    fn tick_new_row(&mut self, row: Row) {
        let mut keep_period = false;
        let mut keep_volume = false;
        let mut keep_position = false;
        let mut keep_envelope = false;

        if row.instrument > 0 {
            self.instrument = self.module.get_instrument((row.instrument - 1) as usize);

            // Instrument without note - sample position is kept, only envelopes are reset
            if row.note == 0 && self.has_instrument_and_sample() {
                keep_position = true;
            }
            // Select new instrument and sample
            else if let Some(instrument) = &self.instrument {
                if let Some(sample) = instrument.get_sample_for_note(row.note as usize) {
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
            keep_volume = row.volume != 0;
        }

        if row.has_valid_note() && self.has_instrument_and_sample() {
            if row.has_portamento() {
                keep_period = true;
                //keep_volume = true;
                keep_position = !self.note_released;
                keep_envelope = !self.note_released;
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

    fn apply_vibrato(&mut self) {
        self.vibrato_ticks += self.vibrato_speed as usize;
        self.vibrato_note_offset =
            -2.0f32 * get_vibrato_period(self.vibrato_ticks) * (self.vibrato_depth as f32 / 15.0);
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
        let mut cancel_vibrato = true;

        // Volume slide down (or fine slide down)
        if self.row.volume.test_high_nibble(0x60)
            || (self.row.volume.test_high_nibble(0x80) && row_tick_index == 0)
        {
            self.note_volume = self
                .note_volume
                .saturating_sub((self.row.volume & 0x0F) as usize);
        }
        // Volume slide up (or fine slide up)
        else if self.row.volume.test_high_nibble(0x70)
            || (self.row.volume.test_high_nibble(0x90) && row_tick_index == 0)
        {
            self.note_volume += (self.row.volume & 0x0F) as usize;
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
                // TBD
            }
            // Vibrato
            0x04 => {
                let depth = self.row.effect_param & 0x0F;
                let speed = unsafe { self.row.effect_param.unchecked_shr(4) as u8 };

                if depth > 0 {
                    self.vibrato_depth = depth;
                }

                if speed > 0 {
                    self.vibrato_speed = speed;
                }

                self.apply_vibrato();
                cancel_vibrato = false;
            }
            _ => {}
        }

        if self.note_period != self.note_target_period {
            self.note_period = slide_towards(
                self.note_period,
                self.note_target_period,
                1.0 * (self.last_nonzero_effect_param as f32),
            );
        }

        if cancel_vibrato {
            self.vibrato_note_offset = 0.0;
        }

        self.note_frequency = get_frequency(self.note_period - 16.0 * self.vibrato_note_offset);
        self.note_step = ((self.note_frequency as f64) / (self.sample_rate as f64)) as f32;
        self.note_step_fp = self.note_step.into();
        self.filter = self.filter.copy_with_new_coefs(self.note_step);

        self.final_volume = self.note_volume;
        self.final_panning = self.note_panning;
    }

    fn tick_envelopes(&mut self) -> bool {
        let mut kill_note = false;

        if let Some(instrument) = &self.instrument {
            let volume = instrument.volume_envelope.tick_and_get_value(
                &mut self.volume_envelope_ticks,
                self.note_released,
                64,
            ) as usize;

            if self.note_released {
                self.note_fadeout -= instrument.volume_envelope.fadeout as i32;

                if self.note_fadeout < 0 {
                    self.note_fadeout = 0;
                    kill_note = true;
                }
            }

            self.final_volume =
                (self.note_volume * volume * (self.note_fadeout as usize)) / (64 * 65536);

            let mut panning = (self.note_panning as i32) - 128;

            panning += 4
                * (instrument.panning_envelope.tick_and_get_value(
                    &mut self.panning_envelope_ticks,
                    self.note_released,
                    32,
                ) as i32)
                - 128;

            self.final_panning = (panning.clamp(-128, 128) + 128).clamp(0, 255) as usize;
        }

        kill_note
    }

    pub fn reset(&mut self) {
        self.note_kill();
    }

    pub fn get_current_stereo_volume(&self) -> (u8, u8) {
        let p = (self.final_panning.clamp(0, 255) as f32) / 255.0;

        // Square law panning
        let vr = sqrt(p) * (self.final_volume as f32);
        let vl = sqrt(1.0 - p) * (self.final_volume as f32);

        (vl.clamp(0.0, 255.0) as u8, vr.clamp(0.0, 255.0) as u8)
    }

    pub fn tick(&mut self, row: Row, song_state: &SongState, buffer: &mut [i32]) -> (u8, u8) {
        if self.mute {
            return (0, 0);
        }

        if song_state.row_tick == 0 {
            self.tick_new_row(row);
        }

        self.apply_volume(song_state.row_tick);
        self.apply_effects(song_state.row_tick);

        if self.tick_envelopes() {
            self.note_kill();
            return (0, 0);
        }

        let (vl, vr) = self.get_current_stereo_volume();
        if vl == 0 && vr == 0 {
            return (0, 0);
        }

        if let Some(sample) = &self.sample {
            let mut offset_fp = self.sample_offset_fp;
            let step_fp = self.note_step_fp;

            unsafe {
                let mut buffer_ptr = buffer.as_mut_ptr();

                // Can we use fast path for mixing? Using fast path means we can safely forward the sample
                // on every buffer element and not worry about hitting loop boundaries
                let end_offset = offset_fp.mad_u32(&step_fp, buffer.len() as u32);

                let use_fast_path = match sample.loop_type {
                    LoopType::None => end_offset < sample.sample_end,
                    LoopType::Forward => end_offset < sample.loop_end,
                    LoopType::PingPong => self.loop_dir_forward && (end_offset < sample.loop_end),
                };

                macro_rules! render_samples_fp {
                    ($test:block) => {
                        let end = buffer_ptr.add(buffer.len());

                        if step_fp.has_only_fract() {
                            let mut v =
                                *sample.data.get_unchecked(offset_fp.integer as usize) as i32;

                            while buffer_ptr < end {
                                *buffer_ptr = v;
                                if offset_fp.add_fract_mut(step_fp.fract) {
                                    $test;
                                    v = *sample.data.get_unchecked(offset_fp.integer as usize)
                                        as i32;
                                }

                                buffer_ptr = buffer_ptr.add(1);
                            }
                        } else {
                            while buffer_ptr < end {
                                *buffer_ptr =
                                    *sample.data.get_unchecked(offset_fp.integer as usize) as i32;

                                offset_fp.add_mut(&step_fp);
                                $test;

                                buffer_ptr = buffer_ptr.add(1);
                            }
                        }

                        self.sample_offset_fp = offset_fp;
                    };
                }

                if use_fast_path {
                    render_samples_fp!({});
                } else {
                    match sample.loop_type {
                        LoopType::None => {
                            buffer.fill(0);

                            render_samples_fp!({
                                if offset_fp.integer >= sample.sample_end {
                                    break;
                                }
                            });

                            self.note_kill();
                        }
                        LoopType::Forward => {
                            render_samples_fp!({
                                if offset_fp.integer >= sample.loop_end {
                                    offset_fp.integer -= sample.loop_end - sample.loop_start;
                                }
                            });
                        }
                        LoopType::PingPong => {
                            for f in buffer {
                                *f = *sample.data.get_unchecked(offset_fp.integer as usize) as i32;

                                if self.loop_dir_forward {
                                    offset_fp.add_mut(&step_fp);
                                    if offset_fp.integer >= sample.loop_end {
                                        offset_fp.sub_mut(&step_fp);
                                        self.loop_dir_forward = false;
                                    }
                                } else {
                                    offset_fp.sub_mut(&step_fp);
                                    if offset_fp.integer < sample.loop_start
                                        || offset_fp.integer >= (u32::MAX / 2)
                                    {
                                        offset_fp.add_mut(&step_fp);
                                        self.loop_dir_forward = true;
                                    }
                                }
                            }

                            self.sample_offset_fp = offset_fp;
                        }
                    }
                }
            }
        }
        // No active sample playing on this channel right now
        else {
            return (0, 0);
        }

        (vl, vr)
    }
}
