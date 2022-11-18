use super::*;
use xm_player::EnvelopeDesc;

#[derive(Clone, Default)]
pub struct BEnvelope {
    pub index: usize,
    pub points: Vec<(u32, u32)>,
    pub tick_values: Vec<u16>,
    pub desc: EnvelopeDesc,
}

impl BEnvelope {
    pub fn build(&mut self, points: &[usize], enable_sustain: bool, enable_loop: bool) {
        self.tick_values.clear();

        if points.len() < 4 || (points.len() % 2) != 0 {
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
                self.tick_values.push(value as u16);
            }

            prev_tick = to_tick;
            prev_value = to_value;
        }

        // Ensure last envelope point value is stored as well
        self.tick_values.push(prev_value as u16);

        // Convert sustain to tick time
        if (self.desc.sustain as usize) < (points.len() / 2) {
            self.desc.sustain = points[(self.desc.sustain * 2) as usize] as u16;
        } else {
            self.desc.sustain = u16::MAX;
        }

        // Convert loop_start to tick time
        if (self.desc.loop_start as usize) < (points.len() / 2) {
            self.desc.loop_start = points[(self.desc.loop_start * 2) as usize] as u16;
        } else {
            self.desc.loop_start = u16::MAX;
        }

        // Convert loop_end to tick time
        if (self.desc.loop_end as usize) < (points.len() / 2) {
            self.desc.loop_end = points[(self.desc.loop_end * 2) as usize] as u16;
        } else {
            self.desc.loop_end = u16::MAX;
        }

        if !enable_sustain {
            self.desc.sustain = u16::MAX;
        }

        if !enable_loop {
            self.desc.loop_start = u16::MAX;
            self.desc.loop_end = u16::MAX;
        }
    }
}
