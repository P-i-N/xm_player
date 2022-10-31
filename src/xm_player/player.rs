use core::arch::x86_64;
use std::arch::x86_64::_mm256_adds_epi16;
use std::arch::x86_64::_mm256_loadu_si256;
use std::arch::x86_64::_mm_loadu_si128;
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
    pub tick_index: usize,
    pub num_generated_samples: usize,

    channels: Vec<Channel<'a>>,

    // Individual channels are rendered there each tick
    buffer: Vec<i16>,

    // Mix of all channels for each tick
    mix_buffer: Vec<i16>,

    // For calculating CPU usage
    tick_durations: Vec<Duration>,

    // How many microseconds it took to render & mix last row
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
            tick_index: 0,
            num_generated_samples: 0,
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

    fn print_row(&self) {
        let mut s = String::new();

        for i in 0..self.channels.len() {
            let pattern_index = self.module.pattern_order[self.pattern_order_index];
            let row = self.module.patterns[pattern_index].channels[i][self.row_index];

            if self.row_index == 0 {
                s += "-+-";
            } else {
                s += " | ";
            }

            s += row.to_colored_string().as_str();
        }

        println!(
            "{:02}{}\x1b[0m | CPU: {:.1}% row: {}us",
            self.row_index,
            s,
            self.row_cpu_usage,
            self.row_cpu_duration.as_micros()
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
        self.pattern_index = self.module.pattern_order[self.pattern_order_index];
        self.row_index += 1;

        self.row_cpu_usage = self.estimate_cpu_usage();
        self.row_cpu_duration = self.get_last_row_cpu_duration();
        self.tick_durations.clear();

        if self.row_index == self.module.patterns[self.pattern_index].num_rows {
            self.row_index = 0;
            self.pattern_order_index += 1;

            if self.pattern_order_index >= self.module.pattern_order.len() {
                self.pattern_order_index = self.module.restart_position;
                self.row_index = 0;
            }
        }
    }

    fn tick(&mut self) {
        if (self.tick_index % self.module.tempo) == 0 {
            self.print_row();
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
            channel.tick(row, self.tick_index % self.module.tempo, &mut self.buffer);
            channels_tick_duration += channel_tick_start.elapsed();

            unsafe {
                let mut src = self.buffer.as_ptr() as *const core::arch::x86_64::__m256i;
                let mut dst = self.mix_buffer.as_mut_ptr() as *mut core::arch::x86_64::__m256i;

                for _ in (0..self.buffer.len()).step_by(16) {
                    *dst = _mm256_adds_epi16(*src, *dst);

                    src = src.add(1);
                    dst = dst.add(1);
                }
            }
        }

        //self.tick_durations.push(time_start.elapsed());
        self.tick_durations.push(channels_tick_duration);

        self.num_generated_samples = self.mix_buffer.len();

        self.tick_index += 1;
        if (self.tick_index % self.module.tempo) == 1 {
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
                for i in 0..to_copy {
                    output[num_filled_samples + i] = src[i];
                }

                self.num_generated_samples -= to_copy;
                num_filled_samples += to_copy;
            } else {
                self.tick();
            }
        }

        num_filled_samples
    }
}
