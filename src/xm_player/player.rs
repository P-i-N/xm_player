use std::arch::x86_64::_mm256_adds_epi16;
use std::ops::BitAnd;
use std::time::Duration;
use std::time::Instant;

use super::Channel;
use super::Module;

pub struct Player<'a> {
    pub module: &'a Module,
    pub sample_rate: usize,
    pub samples_per_tick: usize,
    pub pattern_order_index: usize,
    pub pattern_index: usize,
    pub row_index: usize,

    // Current tick inside a row, goes from 0 to module.tempo-1
    pub row_tick: usize,

    pub num_generated_samples: usize,

    // How many times song looped - might be incorrect for some
    // modules with complicated pattern jump commands
    pub loop_count: usize,

    // Print colored pattern rows, while rendering/playing
    pub print_rows: bool,

    channels: Vec<Channel<'a>>,

    // Individual channels are rendered there each tick
    buffer: Vec<i16>,

    // Mix of all channels for each tick
    mix_buffer: Vec<i16>,

    // For calculating CPU usage
    tick_durations: Vec<Duration>,

    // How long it took to render & mix last row
    row_cpu_duration: Duration,

    // Estimated CPU usage (0.0% - 100.0%) on last row
    row_cpu_usage: f32,
}

impl<'a> Player<'a> {
    pub fn new(module: &'a Module, sample_rate: usize) -> Player {
        let samples_per_tick = ((sample_rate * 2500) / module.bpm) / 1000;

        let mut result = Player {
            module,
            sample_rate: sample_rate,
            samples_per_tick,
            pattern_order_index: 0,
            pattern_index: 0,
            row_index: 0,
            row_tick: 0,
            num_generated_samples: 0,
            loop_count: 0,
            print_rows: false,
            channels: Vec::new(),
            buffer: vec![0; samples_per_tick * 2],
            mix_buffer: vec![0; samples_per_tick * 2],
            tick_durations: Vec::new(),
            row_cpu_duration: Duration::ZERO,
            row_cpu_usage: 0.0,
        };

        for _ in 0..module.num_channels {
            result.channels.push(Channel::new(module, sample_rate));
        }

        result
    }

    pub fn reset(&mut self) {
        self.pattern_order_index = 0;
        self.pattern_index = 0;
        self.row_index = 0;
        self.row_tick = 0;
        self.loop_count = 0;
        self.buffer.fill(0);
        self.mix_buffer.fill(0);

        for channel in &mut self.channels {
            channel.reset();
        }
    }

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

            s += row.to_colored_string().as_str();
        }

        print!("{:02}{}", self.row_index, s);

        println!(
            "\x1b[0m | CPU: {}us / {:.1}%",
            self.row_cpu_duration.as_micros(),
            self.row_cpu_usage
        );
    }

    fn estimate_cpu_usage(&self) -> f32 {
        let mut result = 0.0f32;

        for t in &self.tick_durations {
            result += t.as_micros() as f32;
        }

        // Tick duration in microseconds
        let tick_duration =
            (1000000.0 * (self.samples_per_tick as f32)) / (self.sample_rate as f32);

        (result / (tick_duration * (self.tick_durations.len() as f32))) * 100.0
    }

    fn get_last_row_cpu_duration(&self) -> Duration {
        if self.tick_durations.is_empty() {
            return Duration::ZERO;
        }

        let num_items = usize::min(self.tick_durations.len(), self.module.tempo);
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

            // Pattern break
            if row.effect_type == 0x0D {
                self.pattern_order_index += 1;
                if self.pattern_order_index >= self.module.pattern_order.len() {
                    self.pattern_order_index = self.module.restart_position;
                    self.loop_count += 1;
                }

                self.pattern_index = self.module.pattern_order[self.pattern_order_index];
                self.row_index =
                    ((row.effect_param >> 4) * 10 + row.effect_param.bitand(0x0F)) as usize;

                if self.row_index < self.module.patterns[self.pattern_index].num_rows {
                    pattern_break = true;
                }
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
            self.pattern_order_index += 1;

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
                self.print_row();
            }
        }

        let time_start = Instant::now();

        // Clear 32bit mix buffer
        self.mix_buffer.fill(0);

        let mut channels_tick_duration = Duration::ZERO;

        for i in 0..self.channels.len() {
            let channel = &mut self.channels[i];

            self.pattern_index = self.module.pattern_order[self.pattern_order_index];
            let row = self.module.patterns[self.pattern_index].channels[i][self.row_index];

            let channel_tick_start = Instant::now();
            channel.tick(row, self.row_tick, &mut self.buffer);
            channels_tick_duration += channel_tick_start.elapsed();

            unsafe {
                let steps = if self.buffer.len() >= 16 {
                    (self.buffer.len() - 15) / 16
                } else {
                    0
                };

                let mut src = self.buffer.as_ptr() as *const core::arch::x86_64::__m256i;
                let mut dst = self.mix_buffer.as_mut_ptr() as *mut core::arch::x86_64::__m256i;

                for _ in 0..steps {
                    *dst = _mm256_adds_epi16(*src, *dst);

                    src = src.add(1);
                    dst = dst.add(1);
                }

                for i in (steps * 16)..self.buffer.len() {
                    let dst = self.mix_buffer.get_unchecked_mut(i);
                    *dst = dst.saturating_add(*self.buffer.get_unchecked(i));
                }
            }
        }

        self.tick_durations.push(time_start.elapsed());
        //self.tick_durations.push(channels_tick_duration);

        self.num_generated_samples = self.mix_buffer.len();

        self.row_tick += 1;
        if self.row_tick == self.module.tempo {
            self.step_row();
        }
    }

    pub fn render(&mut self, output: &mut [i16]) -> usize {
        let mut num_filled_samples = 0;

        while num_filled_samples < output.len() {
            if self.num_generated_samples > 0 {
                let to_copy = std::cmp::min(
                    self.num_generated_samples,
                    output.len() - num_filled_samples,
                );

                let src = &self.mix_buffer[self.mix_buffer.len() - self.num_generated_samples..];
                output[num_filled_samples..num_filled_samples + to_copy]
                    .copy_from_slice(&src[0..to_copy]);

                self.num_generated_samples -= to_copy;
                num_filled_samples += to_copy;
            } else {
                self.tick();
            }
        }

        num_filled_samples
    }

    pub fn benchmark(&mut self) -> Duration {
        let time_start = Instant::now();
        let prev_print_rows = self.print_rows;
        self.print_rows = false;
        self.loop_count = 0;

        let mut buffer = Vec::<i16>::with_capacity(self.sample_rate * 2);
        buffer.resize(buffer.capacity(), 0);

        while self.loop_count == 0 {
            self.render(&mut buffer);
        }

        self.reset();
        self.print_rows = prev_print_rows;
        time_start.elapsed()
    }
}
