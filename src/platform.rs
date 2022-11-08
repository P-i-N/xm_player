use super::Box;

pub trait PlatformInterface {
    // Get program's elapsed time in microseconds
    fn get_available_samples(&self) -> usize;
    fn audio_wait(&self) -> bool;
    fn audio_render(&self, buffer: &[i16]);
}

pub struct DummyInterface {}

impl DummyInterface {
    pub fn new(_sample_rate: usize) -> Option<Self> {
        Some(Self {})
    }
}

impl PlatformInterface for DummyInterface {
    fn get_available_samples(&self) -> usize {
        1024
    }

    fn audio_wait(&self) -> bool {
        true
    }

    fn audio_render(&self, _: &[i16]) {}
}
