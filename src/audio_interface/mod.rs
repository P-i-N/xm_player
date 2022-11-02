mod win32;
use win32::Win32;

pub trait AudioInterface {
    fn get_available_samples(&self) -> usize;
    fn wait(&self) -> bool;
    fn render(&self, buffer: &[i16]);
}

pub fn create_audio_interface(sample_rate: usize) -> Option<Box<dyn AudioInterface>> {
    if let Some(i) = Win32::create(sample_rate) {
        return Some(Box::new(i));
    }

    None
}
