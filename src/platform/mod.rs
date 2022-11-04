mod win32;
use win32::Win32;

mod dummy;
use dummy::DummyInterface;

pub trait PlatformInterface {
    fn get_available_samples(&self) -> usize;
    fn audio_wait(&self) -> bool;
    fn audio_render(&self, buffer: &[i16]);
    fn audio_stereo_mix(&self, src: &[i16], dst: &mut [i16], volume_left: u16, volume_right: u16);
}

pub fn create_platform_interface(sample_rate: usize) -> Box<dyn PlatformInterface> {
    if let Some(i) = Win32::create(sample_rate) {
        Box::new(i)
    } else {
        Box::new(DummyInterface {})
    }
}
