use super::math::*;

#[derive(Clone, Copy)]
pub struct FixedU32x {
    pub integer: u32,
    pub fract: u16,
}

impl FixedU32x {
    pub fn from_u32(value: u32) -> Self {
        Self {
            integer: value,
            fract: 0,
        }
    }

    pub fn from_f32(value: f32) -> Self {
        Self {
            integer: floor(value) as u32,
            fract: (fract(value) * 65536_f32) as u16,
        }
    }

    pub fn to_f32(&self) -> f32 {
        (self.integer as f32) + (self.fract as f32) / 65536_f32
    }

    pub fn has_only_fract(&self) -> bool {
        self.integer == 0
    }

    pub fn add_fract_mut(&mut self, fract: u16) -> bool {
        let (new_fract, overflow) = self.fract.overflowing_add(fract);
        self.fract = new_fract;

        if overflow {
            self.integer = unsafe { self.integer.unchecked_add(1) };
        }

        overflow
    }

    pub fn add_mut(&mut self, value: &FixedU32x) {
        let old_fract = self.fract;

        unsafe {
            self.integer = self.integer.unchecked_add(value.integer);
            let (new_fract, overflow) = self.fract.overflowing_add(value.fract);
            self.fract = new_fract;

            if overflow {
                self.integer = self.integer.unchecked_add(1);
            }
        }
    }
}
