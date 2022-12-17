use super::*;

#[derive(Clone, Default)]
pub struct Envelope {
    pub tick_values: Vec<u8>,
    pub sustain: usize,
    pub loop_start: usize,
    pub loop_end: usize,
    pub fadeout: u16,
}

impl Envelope {
    pub fn build(&mut self, points: &[usize], enable_sustain: bool, enable_loop: bool) {
        self.tick_values.clear();

        if points.len() < 4 || (points.len() % 2) != 0 {
            self.loop_start = usize::MAX;
            self.loop_end = usize::MAX;
            self.sustain = usize::MAX;
            return;
        }

        let mut prev_tick = points[0] as i32;
        let mut prev_value = points[1] as i32;

        for pi in (2..points.len() - 1).step_by(2) {
            let to_tick = points[pi] as i32;
            let to_value = points[pi + 1] as i32;
            let num_ticks = to_tick - prev_tick;

            for ti in 0..num_ticks {
                let value = prev_value + ((to_value - prev_value) * ti) / num_ticks;
                self.tick_values.push(value as u8);
            }

            prev_tick = to_tick;
            prev_value = to_value;
        }

        // Ensure last envelope point value is stored as well
        self.tick_values.push(prev_value as u8);

        // Convert sustain to tick time
        if (self.sustain as usize) < (points.len() / 2) {
            self.sustain = points[(self.sustain * 2) as usize] as usize;
        } else {
            self.sustain = usize::MAX;
        }

        // Convert loop_start to tick time
        if (self.loop_start as usize) < (points.len() / 2) {
            self.loop_start = points[(self.loop_start * 2) as usize] as usize;
        } else {
            self.loop_start = usize::MAX;
        }

        // Convert loop_end to tick time
        if (self.loop_end as usize) < (points.len() / 2) {
            self.loop_end = points[(self.loop_end * 2) as usize] as usize;
        } else {
            self.loop_end = usize::MAX;
        }

        if !enable_sustain {
            self.sustain = usize::MAX;
        }

        if !enable_loop {
            self.loop_start = usize::MAX;
            self.loop_end = usize::MAX;
        }
    }

    pub fn is_enabled(&self) -> bool {
        !self.tick_values.is_empty()
    }

    pub fn get_value(&self, ticks: usize, default: u8) -> u8 {
        if !self.is_enabled() {
            return default;
        }

        if ticks >= self.tick_values.len() {
            return self.tick_values[self.tick_values.len() - 1];
        }

        self.tick_values[ticks]
    }

    fn advance_ticks(&self, mut ticks: usize, note_released: bool) -> usize {
        if !self.is_enabled() {
            return ticks;
        }

        if !note_released && self.sustain != usize::MAX {
            if ticks < self.sustain as usize {
                ticks += 1
            }
        } else {
            ticks += 1;
            if ticks == self.loop_end {
                ticks = self.loop_start;
            }
        }

        ticks
    }

    pub fn tick_and_get_value(&self, ticks: &mut usize, note_released: bool, default: u8) -> u8 {
        *ticks = self.advance_ticks(*ticks, note_released);
        self.get_value(*ticks, default)
    }
}
