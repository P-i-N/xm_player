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
    mix_buffer: Vec<i32>,
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

        println!("{:02}{}\x1b[0m", self.row_index, s);
    }

    fn step_row(&mut self) {
        self.pattern_index = self.module.pattern_order[self.pattern_order_index];
        self.row_index += 1;

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

        // Clear 32bit mix buffer
        for s in &mut self.mix_buffer {
            *s = 0;
        }

        for i in 0..self.channels.len() {
            let channel = &mut self.channels[i];

            // Clear 16bit channel temp buffer
            for j in &mut self.buffer {
                *j = 0;
            }

            self.pattern_index = self.module.pattern_order[self.pattern_order_index];
            let row = self.module.patterns[self.pattern_index].channels[i][self.row_index];

            channel.tick(row, self.tick_index % self.module.tempo, &mut self.buffer);

            for j in 0..self.buffer.len() {
                self.mix_buffer[j] += self.buffer[j] as i32;
            }
        }

        // Clamp mix buffer to 16bit range
        for i in &mut self.mix_buffer {
            *i = (*i).clamp(i16::MIN as i32, i16::MAX as i32);
        }

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
                    output[num_filled_samples + i] = src[i] as i16;
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
