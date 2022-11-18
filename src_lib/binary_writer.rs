use std::mem::transmute;

pub struct BinaryWriter<'a> {
    output: &'a mut Vec<u8>,
}

impl<'a> BinaryWriter<'a> {
    pub fn new(output: &'a mut Vec<u8>) -> Self {
        Self { output }
    }

    pub fn pos(&self) -> usize {
        self.output.len()
    }

    pub fn write_u8(&mut self, value: u8) {
        self.output.push(value);
    }

    pub fn write_i8(&mut self, value: i8) {
        self.write_u8(unsafe { transmute::<i8, u8>(value) });
    }

    pub fn write_u16(&mut self, value: u16) {
        self.output.push((value & 0xFF) as u8);
        self.output.push(((value >> 8) & 0xFF) as u8);
    }

    pub fn write_i16(&mut self, value: i16) {
        self.write_u16(unsafe { transmute::<i16, u16>(value) });
    }

    pub fn write_u32(&mut self, value: u32) {
        self.output.push((value & 0xFF) as u8);
        self.output.push(((value >> 8) & 0xFF) as u8);
        self.output.push(((value >> 16) & 0xFF) as u8);
        self.output.push(((value >> 24) & 0xFF) as u8);
    }

    pub fn write_i32(&mut self, value: i32) {
        self.write_u32(unsafe { transmute::<i32, u32>(value) });
    }

    pub fn align_to(&mut self, alignment: usize) {
        let pos = self.pos();
        let padding = (alignment - (pos % alignment)) % alignment;
        for _ in 0..padding {
            self.write_u8(0);
        }
    }

    pub fn write_aligned_struct<T>(&mut self, value: &T) {
        self.align_to(core::mem::align_of::<T>());
        self.write_struct(value);
    }

    pub fn write_struct<T>(&mut self, value: &T) {
        let size = core::mem::size_of::<T>();
        let ptr = value as *const T;
        let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, size) };
        self.output.extend_from_slice(bytes);
    }

    pub fn write_aligned_slice<T>(&mut self, slice: &[T]) {
        self.align_to(core::mem::align_of::<T>());
        self.write_slice(slice);
    }

    pub fn write_slice<T>(&mut self, slice: &[T]) {
        let size = core::mem::size_of::<T>();
        let ptr = slice.as_ptr();
        let bytes = unsafe { std::slice::from_raw_parts(ptr as *const u8, size * slice.len()) };
        self.output.extend_from_slice(bytes);
    }
}
