use super::Box;

pub trait PlatformInterface {
    // Get program's elapsed time in microseconds
    fn get_time_us(&self) -> u32;
    fn get_available_samples(&self) -> usize;
    fn audio_wait(&self) -> bool;
    fn audio_render(&self, buffer: &[i16]);
    fn audio_mono2stereo_mix(
        &self,
        src: &[i32],
        dst: &mut [i16],
        volume_left: i32,
        volume_right: i32,
    );
}

pub struct DummyInterface {}

impl DummyInterface {
    pub fn new(_sample_rate: usize) -> Option<Self> {
        Some(Self {})
    }
}

impl PlatformInterface for DummyInterface {
    fn get_time_us(&self) -> u32 {
        0
    }

    fn get_available_samples(&self) -> usize {
        1024
    }

    fn audio_wait(&self) -> bool {
        true
    }

    fn audio_render(&self, _: &[i16]) {}

    fn audio_mono2stereo_mix(&self, _: &[i32], _: &mut [i16], _: i32, _: i32) {}
}
