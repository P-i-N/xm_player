use super::{Channel, Duration, Module, NibbleTest, PlatformInterface, String, Vec};

#[derive(Clone, Copy)]
pub struct SongState {
    pub bpm: usize,
    pub tempo: usize,
}

pub struct Player<'a, 'b> {
    pub module: &'a Module<'a>,
    pub platform: &'b dyn PlatformInterface,
    pub sample_rate: usize,
    pub oversample: usize,
    pub samples_per_tick: usize,

    pub song_state: SongState,

    pub pattern_order_index: usize,
    pub pattern_index: usize,
    pub row_index: usize,

    // Current tick inside a row, goes from 0 to module.tempo-1
    pub row_tick: usize,

    pub num_generated_samples: usize,

    // Repeat current pattern
    pub loop_current_pattern: bool,

    // How many times song looped - might be incorrect for some
    // modules with complicated pattern jump commands
    pub loop_count: usize,

    // Print colored pattern rows, while rendering/playing
    pub print_rows: bool,

    channels: Vec<Channel<'a>>,

    // Individual channels are rendered there each tick - mono
    channel_buffer: Vec<i32>,

    // Mix of all channels for each tick - stereo
    mix_buffer: Vec<i16>,

    // For calculating CPU usage
    tick_durations: Vec<Duration>,

    // How long it took to render & mix last row
    row_cpu_duration: Duration,

    // Estimated CPU usage (0.0% - 100.0%) on last row
    row_cpu_usage: f32,
}

fn get_samples_per_tick(sample_rate: usize, bpm: usize, oversample: usize) -> usize {
    (((sample_rate * 2500) / bpm) / 1000) * oversample
}

impl<'a, 'b> Player<'a, 'b> {
    pub fn new(
        module: &'a Module,
        platform: &'b dyn PlatformInterface,
        sample_rate: usize,
        oversample: usize,
    ) -> Player<'a, 'b> {
        let samples_per_tick = get_samples_per_tick(sample_rate, module.bpm, oversample);

        let mut result = Player {
            module,
            platform,
            sample_rate: sample_rate,
            oversample,
            samples_per_tick,
            song_state: SongState {
                bpm: module.bpm,
                tempo: module.tempo,
            },
            pattern_order_index: 0,
            pattern_index: 0,
            row_index: 0,
            row_tick: 0,
            num_generated_samples: 0,
            loop_current_pattern: false,
            loop_count: 0,
            print_rows: false,
            channels: Vec::new(),
            channel_buffer: Vec::with_capacity(samples_per_tick),
            mix_buffer: Vec::with_capacity(samples_per_tick * 2),
            tick_durations: Vec::new(),
            row_cpu_duration: Duration::ZERO,
            row_cpu_usage: 0.0,
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

        //result.solo_channel(0);
        //result.pattern_order_index = 5;
        //result.loop_current_pattern = true;

        result
    }

    pub fn reset(&mut self) {
        self.pattern_order_index = 0;
        self.pattern_index = 0;
        self.row_index = 0;
        self.row_tick = 0;
        self.loop_count = 0;
        self.channel_buffer.fill(0);
        self.mix_buffer.fill(0);

        for channel in &mut self.channels {
            channel.reset();
        }
    }

    /*
    fn print_row(&self) {
        let mut s = String::new();

        for i in 0..self.channels.len() {
            let pattern_index = self.module.pattern_order[self.pattern_order_index];
            let row = self.module.patterns[pattern_index].channels[i][self.row_index];

            if self.row_index == 0 {
                s += "\x1b[0m-+-";
            } else {
                s += "\x1b[0m | ";
            }

            if !self.channels[i].mute {
                s += row.to_colored_string().as_str();
            } else {
                s += "           ";
            }
        }

        print!("{:02}{}", self.row_index, s);

        println!(
            "\x1b[0m | CPU: {}us / {:.1}%",
            self.row_cpu_duration.as_micros(),
            self.row_cpu_usage
        );
    }
    */

    fn estimate_cpu_usage(&self) -> f32 {
        let mut result = 0.0f32;

        for t in &self.tick_durations {
            result += t.as_micros() as f32;
        }

        // Tick duration in microseconds
        let tick_duration = (1000000.0 * (self.samples_per_tick as f32))
            / ((self.oversample * self.sample_rate) as f32);

        (result / (tick_duration * (self.tick_durations.len() as f32))) * 100.0
    }

    fn get_last_row_cpu_duration(&self) -> Duration {
        if self.tick_durations.is_empty() {
            return Duration::ZERO;
        }

        let num_items = usize::min(self.tick_durations.len(), self.song_state.tempo);
        let slice = &self.tick_durations[self.tick_durations.len() - num_items..];

        let mut result = Duration::ZERO;
        for d in slice {
            result += *d;
        }

        result
    }

    fn step_row(&mut self) {
        self.row_cpu_usage = self.estimate_cpu_usage();
        self.row_cpu_duration = self.get_last_row_cpu_duration();
        self.tick_durations.clear();

        let mut pattern_break = false;

        for channel_index in 0..self.channels.len() {
            let row = self.module.get_channel_row_ordered(
                self.pattern_order_index,
                channel_index,
                self.row_index,
            );

            match row.effect_type {
                // Pattern break
                0x0D => {
                    self.pattern_order_index += 1;
                    if self.pattern_order_index >= self.module.pattern_order.len() {
                        self.pattern_order_index = self.module.restart_position;
                        self.loop_count += 1;
                    }

                    self.pattern_index = self.module.pattern_order[self.pattern_order_index];
                    self.row_index =
                        ((row.effect_param >> 4) * 10 + row.effect_param & 0x0F) as usize;

                    if self.row_index < self.module.patterns[self.pattern_index].num_rows {
                        pattern_break = true;
                    }
                }

                // Set module BPM/tempo
                0x0F => {
                    // Set tempo
                    if row.effect_param.test_high_nibble(0) {
                        self.song_state.tempo = row.effect_param as usize;
                    }
                    // Set BPM
                    else {
                        self.song_state.bpm = row.effect_param as usize;
                    }

                    let samples_per_tick = get_samples_per_tick(
                        self.sample_rate,
                        self.song_state.bpm,
                        self.oversample,
                    );

                    if samples_per_tick != self.samples_per_tick {
                        self.channel_buffer.resize(samples_per_tick, 0);
                        self.mix_buffer.resize(samples_per_tick * 2, 0);
                        self.samples_per_tick = samples_per_tick;
                    }
                }

                _ => (),
            }
        }

        self.row_tick = 0;

        if pattern_break {
            return;
        }

        self.pattern_index = self.module.pattern_order[self.pattern_order_index];
        self.row_index += 1;

        if self.row_index >= self.module.patterns[self.pattern_index].num_rows {
            self.row_index = 0;

            if !self.loop_current_pattern {
                self.pattern_order_index += 1;
            }

            if self.pattern_order_index >= self.module.pattern_order.len() {
                self.pattern_order_index = self.module.restart_position;
                self.row_index = 0;
                self.loop_count += 1;
            }
        }
    }

    fn tick(&mut self) {
        if self.row_tick == 0 {
            if self.print_rows {
                //self.print_row();
            }
        }

        //let time_start = self.platform.get_time_us();

        // Clear 32bit mix buffer
        self.mix_buffer.fill(0);

        let mut channels_tick_duration = Duration::ZERO;

        for i in 0..self.channels.len() {
            let channel = &mut self.channels[i];

            self.pattern_index = self.module.pattern_order[self.pattern_order_index];
            let row = self.module.patterns[self.pattern_index].channels[i][self.row_index];

            //let channel_tick_start = self.platform.get_time_us();

            let (vl, vr) = channel.tick(
                row,
                &self.song_state,
                self.row_tick,
                &mut self.channel_buffer,
            );

            //channels_tick_duration += channel_tick_start.elapsed();

            if vl > 0 && vr > 0 {
                self.platform.audio_mono2stereo_mix(
                    &self.channel_buffer,
                    &mut self.mix_buffer,
                    vl as i32,
                    vr as i32,
                );
            }
        }

        //self.tick_durations.push(time_start.elapsed());
        //self.tick_durations.push(channels_tick_duration);

        self.num_generated_samples = self.mix_buffer.len() / self.oversample;

        self.row_tick += 1;
        if self.row_tick >= self.song_state.tempo {
            self.step_row();
        }
    }

    pub fn render(&mut self, output: &mut [i16]) -> usize {
        let mut num_filled_samples = 0;

        while num_filled_samples < output.len() {
            if self.num_generated_samples > 0 {
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
            } else {
                self.tick();
            }
        }

        num_filled_samples
    }

    pub fn solo_channel(&mut self, channel_index: usize) {
        for channel in &mut self.channels {
            channel.mute = channel.index != channel_index;
        }
    }

    pub fn unmute_all(&mut self) {
        for channel in &mut self.channels {
            channel.mute = false;
        }
    }

    pub fn benchmark(&mut self) -> Duration {
        //let time_start = Instant::now();
        let prev_print_rows = self.print_rows;
        self.print_rows = false;
        self.loop_count = 0;

        let mut buffer = Vec::<i16>::with_capacity(self.oversample * self.sample_rate * 2);
        buffer.resize(buffer.capacity(), 0);

        while self.loop_count == 0 {
            self.render(&mut buffer);
        }

        self.reset();
        self.print_rows = prev_print_rows;
        //time_start.elapsed()

        Duration::ZERO
    }
}
