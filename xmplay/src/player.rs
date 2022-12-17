use super::{Channel, Module, NibbleTest, Row, Vec};

#[derive(Clone, Copy, Default)]
pub struct SongState {
    // Beats per minute
    pub bpm: usize,

    // Tempo (number of ticks per row)
    pub tempo: usize,

    // Current ordered pattern index
    pub pattern_order_index: usize,

    // Current pattern index
    pub pattern_index: usize,

    // Current tick inside a row, goes from 0 to module.tempo-1
    pub row_tick: usize,

    // Current row inside a pattern
    pub row_index: usize,
}

pub enum CallbackPosition {
    RenderBegin,
    TickBegin,
    ChannelTickBegin,
    ChannelTickEnd,
    ChannelMixBegin,
    ChannelMixEnd,
    TickEnd,
    MixBegin,
    MixEnd,
    RenderEnd,
}

pub struct Player<'a> {
    pub module: &'a Module<'a>,
    pub sample_rate: usize,
    pub oversample: usize,
    pub samples_per_tick: usize,
    pub song_state: SongState,

    pub num_generated_samples: usize,

    // Repeat current pattern
    pub loop_current_pattern: bool,

    // How many times song looped - might be incorrect for some
    // modules with complicated pattern jump commands
    pub loop_count: usize,

    channels: Vec<Channel<'a>>,

    // Individual channels are rendered there each tick - mono
    channel_buffer: Vec<i32>,

    // Mix of all channels for each tick - stereo
    mix_buffer: Vec<i16>,

    // Tick callback
    pub callback: Option<Box<dyn FnMut(CallbackPosition, &SongState, &Module<'a>)>>,
}

fn get_samples_per_tick(sample_rate: usize, bpm: usize, oversample: usize) -> usize {
    (((sample_rate * 2500) / bpm) / 1000) * oversample
}

macro_rules! notify_callback {
    ($self:ident, $position:ident) => {
        if let Some(cb) = &mut $self.callback {
            cb(
                CallbackPosition::$position,
                &$self.song_state,
                &$self.module,
            );
        }
    };
}

impl<'a> Player<'a> {
    pub fn new(module: &'a Module, sample_rate: usize, oversample: usize) -> Self {
        let samples_per_tick = get_samples_per_tick(sample_rate, module.bpm, oversample);

        let mut result = Player {
            module,
            sample_rate: sample_rate,
            oversample,
            samples_per_tick,
            song_state: SongState {
                bpm: module.bpm,
                tempo: module.tempo,
                ..Default::default()
            },
            num_generated_samples: 0,
            loop_current_pattern: false,
            loop_count: 0,
            channels: Vec::new(),
            channel_buffer: Vec::with_capacity(samples_per_tick),
            mix_buffer: Vec::with_capacity(samples_per_tick * 2),
            callback: None,
        };

        result
            .channel_buffer
            .resize(result.channel_buffer.capacity(), 0);

        result.mix_buffer.resize(result.mix_buffer.capacity(), 0);

        for index in 0..module.num_channels {
            result
                .channels
                .push(Channel::new(module, index, oversample * sample_rate));
        }

        result.reset();

        /*
        result.solo_channel(21);
        result.toggle_channel(22);
        result.toggle_channel(23);
        result.loop_current_pattern = true;
        result.song_state.pattern_order_index = 18;
        */

        result
    }

    pub fn reset(&mut self) {
        self.song_state.pattern_order_index = 0;
        self.song_state.pattern_index = 0;
        self.song_state.row_index = usize::MAX;
        self.song_state.row_tick = usize::MAX;
        self.loop_count = 0;
        self.channel_buffer.fill(0);
        self.mix_buffer.fill(0);

        for channel in &mut self.channels {
            channel.reset();
        }
    }

    pub fn set_tick_callback(
        &mut self,
        callback: impl FnMut(CallbackPosition, &SongState, &Module<'a>) + 'static,
    ) {
        self.callback = Some(Box::new(callback));
    }

    // Calculate tick duration in microseconds
    pub fn get_tick_duration_us(&self) -> u32 {
        (1000000 * (self.samples_per_tick as u32)) / ((self.oversample * self.sample_rate) as u32)
    }

    pub fn get_channel_row_ordered(&self, channel_index: usize) -> Row {
        self.module.get_channel_row_ordered(
            self.song_state.pattern_order_index,
            channel_index,
            self.song_state.row_index,
        )
    }

    fn step_row(&mut self) {
        let mut pattern_break = false;
        let mut ss = self.song_state;

        ss.row_tick = 0;
        ss.row_index = ss.row_index.wrapping_add(1);

        if ss.row_index >= self.module.patterns[ss.pattern_index].num_rows {
            ss.row_index = 0;

            if !self.loop_current_pattern {
                ss.pattern_order_index += 1;
            }

            if ss.pattern_order_index >= self.module.pattern_order.len() {
                ss.pattern_order_index = self.module.restart_position;
                ss.row_index = 0;
                self.loop_count += 1;
            }
        }

        for channel_index in 0..self.channels.len() {
            let row = self.module.get_channel_row_ordered(
                ss.pattern_order_index,
                channel_index,
                ss.row_index,
            );

            match row.effect_type {
                // Pattern break
                0x0D => {
                    ss.pattern_order_index += 1;
                    if ss.pattern_order_index >= self.module.pattern_order.len() {
                        ss.pattern_order_index = self.module.restart_position;
                        self.loop_count += 1;
                    }

                    ss.pattern_index = self.module.pattern_order[ss.pattern_order_index];
                    ss.row_index =
                        ((row.effect_param >> 4) * 10 + row.effect_param & 0x0F) as usize;

                    if ss.row_index < self.module.patterns[ss.pattern_index].num_rows {
                        pattern_break = true;
                    }
                }

                // Set module BPM/tempo
                0x0F => {
                    // Set tempo
                    if row.effect_param.test_high_nibble(0) {
                        ss.tempo = row.effect_param as usize;
                    }
                    // Set BPM
                    else {
                        ss.bpm = row.effect_param as usize;
                    }

                    let samples_per_tick =
                        get_samples_per_tick(self.sample_rate, ss.bpm, self.oversample);

                    if samples_per_tick != self.samples_per_tick {
                        self.channel_buffer.resize(samples_per_tick, 0);
                        self.mix_buffer.resize(samples_per_tick * 2, 0);
                        self.samples_per_tick = samples_per_tick;
                    }
                }

                _ => (),
            }
        }

        if !pattern_break {
            ss.pattern_index = self.module.pattern_order[ss.pattern_order_index];
        }

        self.song_state = ss;
    }

    #[inline(never)]
    fn tick(&mut self) {
        self.song_state.row_tick = self.song_state.row_tick.wrapping_add(1) % self.song_state.tempo;
        if self.song_state.row_tick == 0 {
            self.step_row();
        }

        notify_callback!(self, TickBegin);

        // Clear 32bit mix buffer
        self.mix_buffer.fill(0);

        self.song_state.pattern_index =
            self.module.pattern_order[self.song_state.pattern_order_index];

        for i in 0..self.channels.len() {
            let channel = &mut self.channels[i];

            let row = self.module.get_channel_row_ordered(
                self.song_state.pattern_order_index,
                i,
                self.song_state.row_index,
            );

            notify_callback!(self, ChannelTickBegin);

            let (vl, vr) = channel.tick(row, &self.song_state, &mut self.channel_buffer);

            notify_callback!(self, ChannelTickEnd);

            notify_callback!(self, ChannelMixBegin);

            if vl > 0 && vr > 0 {
                const USE_FILTER: bool = true;

                unsafe {
                    let mut dst_ptr = self.mix_buffer.as_mut_ptr();

                    if USE_FILTER {
                        let mut filter = channel.filter;

                        for &s in &self.channel_buffer {
                            let sf = filter.process_i32(s);
                            let vl = sf.unchecked_mul(vl as i32).unchecked_shr(8) as i16;
                            let vr = sf.unchecked_mul(vr as i32).unchecked_shr(8) as i16;

                            *dst_ptr = (*dst_ptr).saturating_add(vl);
                            dst_ptr = dst_ptr.add(1);

                            *dst_ptr = (*dst_ptr).saturating_add(vr);
                            dst_ptr = dst_ptr.add(1);
                        }

                        channel.filter = filter;
                    } else {
                        for &s in &self.channel_buffer {
                            let vl = s.unchecked_mul(vl as i32).unchecked_shr(8) as i16;
                            let vr = s.unchecked_mul(vr as i32).unchecked_shr(8) as i16;

                            *dst_ptr = (*dst_ptr).saturating_add(vl);
                            dst_ptr = dst_ptr.add(1);

                            *dst_ptr = (*dst_ptr).saturating_add(vr);
                            dst_ptr = dst_ptr.add(1);
                        }
                    }
                }
            }

            notify_callback!(self, ChannelMixEnd);
        }

        self.num_generated_samples = self.mix_buffer.len() / self.oversample;

        notify_callback!(self, TickEnd);
    }

    pub fn render(&mut self, output: &mut [i16]) -> usize {
        notify_callback!(self, RenderBegin);

        let mut num_filled_samples = 0;

        while num_filled_samples < output.len() {
            if self.num_generated_samples > 0 {
                notify_callback!(self, MixBegin);

                let to_copy = core::cmp::min(
                    self.num_generated_samples,
                    output.len() - num_filled_samples,
                );

                let src = &self.mix_buffer
                    [self.mix_buffer.len() - self.oversample * self.num_generated_samples..];

                if self.oversample == 1 {
                    output[num_filled_samples..num_filled_samples + to_copy]
                        .copy_from_slice(&src[0..to_copy]);
                } else {
                    for i in 0..to_copy {
                        let mut acc = 0i32;

                        let off = ((i - (i % 2)) * self.oversample) + (i % 2);
                        for j in (off..off + 2 * self.oversample).step_by(2) {
                            acc += src[j] as i32;
                        }

                        output[num_filled_samples + i] = (acc / (self.oversample as i32)) as i16;
                    }
                }

                self.num_generated_samples -= to_copy;
                num_filled_samples += to_copy;

                notify_callback!(self, MixEnd);
            } else {
                self.tick();
            }
        }

        notify_callback!(self, RenderEnd);

        num_filled_samples
    }

    pub fn solo_channel(&mut self, channel_index: usize) {
        for channel in &mut self.channels {
            channel.mute = channel.index != channel_index;
        }
    }

    pub fn toggle_channel(&mut self, channel_index: usize) {
        let channel = &mut self.channels[channel_index];
        channel.mute = !channel.mute;
    }

    pub fn unmute_all(&mut self) {
        for channel in &mut self.channels {
            channel.mute = false;
        }
    }

    pub fn benchmark(&mut self) {
        self.loop_count = 0;

        let mut buffer = Vec::<i16>::with_capacity(self.oversample * self.sample_rate * 2);
        buffer.resize(buffer.capacity(), 0);

        while self.loop_count == 0 {
            self.render(&mut buffer);
        }

        self.reset();
    }
}
