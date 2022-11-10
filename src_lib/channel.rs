use super::math::*;
use super::player::SongState;
use super::ButterworthFilter;
use super::Cell;
use super::Envelope;
use super::Fixed;
use super::Instrument;
use super::LoopType;
use super::Module;
use super::NibbleTest;
use super::Rc;
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

fn follow_envelope(mut ticks: usize, note_released: bool, envelope: &Envelope) -> usize {
    if envelope.tick_values.is_empty() {
        return ticks;
    }

    if !note_released && envelope.sustain != usize::MAX {
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
    module: &'a Module<'a>,
    song_state: SongState,
    pub index: usize,
    pub mute: bool,
    inv_sample_rate: f32,
    row: Cell,
    sample: Option<Rc<Sample>>,
    instrument: Option<Rc<Instrument>>,
    note: f32,
    note_volume: usize,
    note_panning: usize,
    note_period: f32,
    note_target_period: f32,
    note_frequency: f32,
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
            song_state: SongState { bpm: 0, tempo: 0 },
            index,
            mute: false,
            inv_sample_rate: 1.0 / (sample_rate as f32),
            row: Cell::new(),
            sample: None,
            instrument: None,
            note: 0.0,
            note_volume: 0,
            note_panning: 0,
            note_period: 0.0,
            note_target_period: 0.0,
            note_frequency: 0.0,
            note_step_fp: Fixed::from_u32(0),
            note_released: false,
            loop_dir_forward: true,
            volume_envelope_ticks: 0,
            panning_envelope_ticks: 0,
            vibrato_speed: 0,
            vibrato_depth: 0,
            vibrato_ticks: 0,
            vibrato_note_offset: 0.0,
            last_nonzero_effect_param: 0,
            sample_offset_fp: Fixed::from_u32(0),
            final_volume: 0,
            final_panning: 0,
            filter: ButterworthFilter::new(),
        }
    }

    fn is_note_active(&self) -> bool {
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

            self.note_step_fp = Fixed::from_f32(self.note_frequency * self.inv_sample_rate);

            if !keep_volume {
                self.note_volume = sample.volume as usize;
            }

            if !keep_position {
                self.sample_offset_fp = Fixed::from_u32(0);
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
    }

    fn note_kill(&mut self) {
        self.note_released = true;
        self.instrument = None;
        self.sample = None;
    }

    fn tick_new_row(&mut self, row: Cell) {
        let mut keep_period = false;
        let mut keep_volume = false;
        let mut keep_position = false;
        let mut keep_envelope = false;

        if row.instrument > 0 {
            // Instrument without note - sample position is kept, only envelopes are reset
            if row.note == 0 && self.is_note_active() {
                keep_position = true;
            }
            // Select new instrument and sample
            else if let Some(instrument) =
                self.module.get_instrument((row.instrument - 1) as usize)
            {
                if let Some(sample) = instrument.get_sample_for_note(row.note as usize) {
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
                //
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
        self.note_step_fp = Fixed::from_f32(self.note_frequency * self.inv_sample_rate);
        self.filter = self
            .filter
            .copy_with_new_coefs(self.note_frequency * self.inv_sample_rate);

        self.final_volume = self.note_volume;
        self.final_panning = self.note_panning;
    }

    fn tick_envelopes(&mut self) {
        if let Some(instrument) = &self.instrument {
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

            let mut panning = (self.note_panning as i32) - 128;

            panning += 4
                * (instrument
                    .panning_envelope
                    .get_value(self.panning_envelope_ticks) as i32)
                - 128;

            self.final_panning = (panning.clamp(-128, 128) + 128).clamp(0, 255) as usize;
        }
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

    pub fn tick(
        &mut self,
        row: Cell,
        song_state: &SongState,
        row_tick_index: usize,
        buffer: &mut [i32],
    ) -> (u8, u8) {
        self.song_state = *song_state;

        if self.mute {
            return (0, 0);
        }

        if row_tick_index == 0 {
            self.tick_new_row(row);
        }

        self.apply_volume(row_tick_index);
        self.apply_effects(row_tick_index);
        self.tick_envelopes();

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
                                    offset_fp = Fixed::from_u32(sample.loop_start);
                                }
                            });
                        }
                        LoopType::PingPong => {
                            /*
                            for f in buffer {
                                *f = *sample.data.get_unchecked(offset as usize) as i32;

                                if self.loop_dir_forward {
                                    offset += step;
                                    if offset >= sample.loop_end {
                                        offset = sample.loop_end - 1.0;
                                        //self.loop_dir_forward = false;
                                    }
                                } else {
                                    offset -= step;
                                    if offset < sample.loop_start {
                                        offset = sample.loop_start;
                                        //self.loop_dir_forward = true;
                                    }
                                }
                            }

                            self.sample_offset = offset;
                            */
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
