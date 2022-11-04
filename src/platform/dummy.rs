use super::PlatformInterface;

pub struct DummyInterface {
    //
}

impl PlatformInterface for DummyInterface {
    fn get_available_samples(&self) -> usize {
        1024
    }

    fn audio_wait(&self) -> bool {
        true
    }

    fn audio_render(&self, _: &[i16]) {}

    fn audio_stereo_mix(&self, _: &[i16], _: &mut [i16], _: u16, _: u16) {}
}
