use super::math::*;

#[derive(Clone, Copy)]
pub struct FixedU32U16 {
    pub integer: u32,
    pub fract: u16,
}

impl FixedU32U16 {
    pub fn has_only_fract(&self) -> bool {
        self.integer == 0
    }

    pub fn add_fract_mut(&mut self, fract: u16) -> bool {
        let (new_fract, carry) = self.fract.overflowing_add(fract);
        self.fract = new_fract;

        if carry {
            self.integer = unsafe { self.integer.unchecked_add(1) };
        }

        carry
    }

    pub fn add_mut(&mut self, value: &FixedU32U16) {
        unsafe {
            self.integer = self.integer.unchecked_add(value.integer);
            let (new_fract, carry) = self.fract.overflowing_add(value.fract);
            self.fract = new_fract;

            if carry {
                self.integer = self.integer.unchecked_add(1);
            }
        }
    }

    pub fn sub_fract_mut(&mut self, fract: u16) -> bool {
        let (new_fract, carry) = self.fract.overflowing_sub(fract);
        self.fract = new_fract;

        if carry {
            self.integer = unsafe { self.integer.unchecked_sub(1) };
        }

        carry
    }

    pub fn sub_mut(&mut self, value: &FixedU32U16) {
        unsafe {
            self.integer = self.integer.unchecked_sub(value.integer);
            let (new_fract, carry) = self.fract.overflowing_sub(value.fract);
            self.fract = new_fract;

            if carry {
                self.integer = self.integer.unchecked_sub(1);
            }
        }
    }

    // Multiply+Add - returns integral part of: value * count + self
    pub fn mad_u32(&self, value: &FixedU32U16, count: u32) -> u32 {
        unsafe {
            let result = self
                .integer
                .unchecked_add(value.integer.unchecked_mul(count));

            let fract_mult = (value.fract as u32).unchecked_mul(count) + (self.fract as u32);

            result + fract_mult.unchecked_shr(16)
        }
    }
}

impl Default for FixedU32U16 {
    fn default() -> Self {
        Self {
            integer: 0,
            fract: 0,
        }
    }
}

impl Into<f32> for FixedU32U16 {
    fn into(self) -> f32 {
        self.integer as f32 + (self.fract as f32 / 65536_f32)
    }
}

impl Into<FixedU32U16> for f32 {
    fn into(self) -> FixedU32U16 {
        FixedU32U16 {
            integer: floor(self) as u32,
            fract: (fract(self) * 65536_f32) as u16,
        }
    }
}

impl Into<FixedU32U16> for u32 {
    fn into(self) -> FixedU32U16 {
        FixedU32U16 {
            integer: self,
            fract: 0,
        }
    }
}
