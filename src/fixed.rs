use super::math::*;

use core::clone::Clone;
use core::marker::Copy;

#[derive(Clone, Copy)]
pub struct FixedU32<const N: u32> {
    pub value: u32,
}

impl<const N: u32> FixedU32<N> {
    pub fn new_u32(value: u32) -> Self {
        Self { value: value << N }
    }

    pub fn new_f32(value: f32) -> Self {
        Self {
            value: ((fract(value) * ((1 << N) as f32)) as u32) | ((floor(value) as u32) << N),
        }
    }
}

/*
impl<const N: u32> From<u32> for FixedU32<N> {
    fn from(item: u32) -> Self {
        Self { value: item }
    }
}

impl<const N: u32> From<FixedU32<N>> for u32 {
    fn from(item: FixedU32<N>) -> u32 {
        item.value >> N
    }
}

impl<const N: u32> From<FixedU32<N>> for usize {
    fn from(item: FixedU32<N>) -> usize {
        (item.value >> N) as usize
    }
}

impl<const N: u32> From<FixedU32<N>> for f32 {
    fn from(item: FixedU32<N>) -> f32 {
        ((item.value >> N) as f32) + ((item.value & ((1 << N) - 1)) as f32 / ((1 << N) as f32))
    }
}
*/

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Copy)]
pub struct FixedU32x<const N: u32> {
    pub integer: u32,
    pub fract: u32,
}

impl<const N: u32> FixedU32x<N> {
    pub fn new_u32(value: u32) -> Self {
        Self {
            integer: value,
            fract: 0,
        }
    }

    pub fn new_f32(value: f32) -> Self {
        Self {
            integer: floor(value) as u32,
            fract: (fract(value) * ((1 << N) as f32)) as u32,
        }
    }

    pub unsafe fn add_fract_mut(&mut self, fract: u32) -> bool {
        self.fract = self.fract.unchecked_add(fract);
        let result = self.fract.unchecked_shr(N) > 0;
        self.fract = (self.fract & ((1 << N) - 1)) as u32;

        result
    }

    pub unsafe fn add_mut(&mut self, value: &FixedU32x<N>) {
        self.fract = self.fract.unchecked_add(value.fract);

        let overflow = self.fract.unchecked_shr(N);
        if overflow > 0 {
            self.integer = self.integer.unchecked_add(overflow);
        }

        self.integer = self.integer.unchecked_add(value.integer);
        self.fract = (self.fract & ((1 << N) - 1)) as u32;
    }
}

/*
impl<const N: u32> From<FixedU32x<N>> for u32 {
    fn from(item: FixedU32x<N>) -> u32 {
        item.integer
    }
}

impl<const N: u32> From<FixedU32x<N>> for usize {
    fn from(item: FixedU32x<N>) -> usize {
        item.integer as usize
    }
}

impl<const N: u32> From<FixedU32x<N>> for f32 {
    fn from(item: FixedU32x<N>) -> f32 {
        (item.integer as f32) + ((item.fract as f32) / ((1 << N) as f32))
    }
}
*/
