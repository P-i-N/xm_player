use wasapi::*;

use super::AudioInterface;

pub struct Win32 {
    _device: wasapi::Device,
    audio_client: wasapi::AudioClient,
    event_handle: wasapi::Handle,
    render_client: wasapi::AudioRenderClient,
    block_size: usize,
}

impl Win32 {
    pub fn create() -> Option<Self> {
        wasapi::initialize_mta().unwrap();

        let device = wasapi::get_default_device(&Direction::Render).unwrap();
        let mut audio_client = device.get_iaudioclient().unwrap();
        let format = wasapi::WaveFormat::new(16, 16, &SampleType::Int, 48000, 2);
        let block_size = format.get_blockalign() as usize;
        let (def_time, _) = audio_client.get_periods().unwrap();

        audio_client
            .initialize_client(
                &format,
                def_time as i64,
                &Direction::Render,
                &ShareMode::Shared,
                true,
            )
            .unwrap();

        let event_handle = audio_client.set_get_eventhandle().unwrap();
        let render_client = audio_client.get_audiorenderclient().unwrap();

        audio_client.start_stream().unwrap();

        Some(Win32 {
            _device: device,
            audio_client,
            event_handle,
            render_client,
            block_size,
        })
    }
}

impl AudioInterface for Win32 {
    fn get_available_samples(&self) -> usize {
        self.audio_client.get_available_space_in_frames().unwrap() as usize * self.block_size / 2
    }

    fn wait(&self) -> bool {
        if self.event_handle.wait_for_event(10000).is_err() {
            self.audio_client.stop_stream().unwrap();
            return false;
        }

        return true;
    }

    fn render(&self, buffer: &[i16]) {
        unsafe {
            let (_, bytes, _) = buffer.align_to::<u8>();
            self.render_client
                .write_to_device(
                    buffer.len() / self.block_size * 2,
                    self.block_size,
                    &bytes,
                    None,
                )
                .unwrap();
        }
    }
}
