use super::math::*;

// IIR 2-pole lowpass Butterworth filter
#[derive(Clone, Copy)]
pub struct ButterworthFilter {
    coefs: [f32; 3],
    state: [f32; 3],
}

impl ButterworthFilter {
    pub fn new() -> Self {
        Self {
            coefs: [0.0; 3],
            state: [0.0; 3],
        }
    }

    pub fn copy_with_new_coefs(&self, mut relative_center_freq: f32) -> Self {
        relative_center_freq = relative_center_freq.min(0.5);

        let wct = sqrt(2.0) * 3.14159265358 * relative_center_freq;
        let e = exp(-wct);
        let c = e * cos(wct);
        let gain = (1.0 - 2.0 * c + e * e) / 2.0;

        Self {
            coefs: [gain, 2.0 * c, -e * e],
            state: self.state,
        }
    }

    #[inline]
    pub fn process(&mut self, sample: f32) -> f32 {
        let result = self.coefs[0] * (sample + self.state[0])
            + self.coefs[1] * self.state[1]
            + self.coefs[2] * self.state[2];

        self.state[2] = self.state[1];
        self.state[1] = result;
        self.state[0] = sample;

        result
    }

    pub fn process_i16(&mut self, sample: i16) -> i16 {
        self.process(sample as f32) as i16
    }

    pub fn process_i32(&mut self, sample: i32) -> i32 {
        self.process(sample as f32) as i32
    }
}
