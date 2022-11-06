use core::arch::x86_64::{
    _mm256_adds_epi16, _mm256_blend_epi16, _mm256_cvtepi16_epi32, _mm256_extract_epi16,
    _mm256_extracti128_si256, _mm256_mask_blend_epi16, _mm256_movehdup_ps, _mm256_mul_epi32,
    _mm256_mulhi_epi16, _mm256_mullo_epi16, _mm256_mullo_epi32, _mm256_packs_epi32,
    _mm256_set_epi16, _mm256_set_epi32, _mm256_setr_epi16, _mm256_slli_epi32, _mm256_srai_epi32,
};

use wasapi::*;

use super::PlatformInterface;

pub struct Win32 {
    _device: wasapi::Device,
    audio_client: wasapi::AudioClient,
    event_handle: wasapi::Handle,
    render_client: wasapi::AudioRenderClient,
    block_size: usize,
}

impl Win32 {
    pub fn new(sample_rate: usize) -> Option<Self> {
        wasapi::initialize_mta().unwrap();

        let device = wasapi::get_default_device(&Direction::Render).unwrap();
        let mut audio_client = device.get_iaudioclient().unwrap();
        let format = wasapi::WaveFormat::new(16, 16, &SampleType::Int, sample_rate, 2);
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

impl PlatformInterface for Win32 {
    fn get_time_us(&self) -> u32 {
        //
    }

    fn get_available_samples(&self) -> usize {
        let result = self.audio_client.get_available_space_in_frames().unwrap() as usize
            * (self.block_size / 2);
        result - (result % self.block_size)
    }

    fn audio_wait(&self) -> bool {
        if self.event_handle.wait_for_event(10000).is_err() {
            self.audio_client.stop_stream().unwrap();
            return false;
        }

        return true;
    }

    fn audio_render(&self, buffer: &[i16]) {
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

    fn audio_mono2stereo_mix(
        &self,
        src: &[i32],
        dst: &mut [i16],
        volume_left: i32,
        volume_right: i32,
    ) {
        // Destinatin slice MUST be twice as long, since it stores calculated stereo
        // result of the source mono data
        assert!(src.len() * 2 == dst.len());

        // Destination slice MUST have even number of elements
        assert!((dst.len() % 2) == 0);

        const STEP_SIZE: usize = 16;

        // How many 16 sample chunks can we process at once? 16x16bit samples fit nicely
        // into one __m256i register
        let steps = if src.len() >= STEP_SIZE {
            (src.len() - (STEP_SIZE - 1)) / STEP_SIZE
        } else {
            0
        };

        unsafe {
            let mut dst_ptr = dst.as_mut_ptr();

            for i in 0..src.len() {
                let s = *src.get_unchecked(i);
                let vl = s.unchecked_mul(volume_left).unchecked_shr(8) as i16;
                let vr = s.unchecked_mul(volume_right).unchecked_shr(8) as i16;

                *dst_ptr = (*dst_ptr).saturating_add(vl);
                dst_ptr = dst_ptr.add(1);

                *dst_ptr = (*dst_ptr).saturating_add(vr);
                dst_ptr = dst_ptr.add(1);
            }
        }

        /*
        unsafe {
            let mut src_ptr = src.as_ptr() as *const core::arch::x86_64::__m256i;
            let mut dst_ptr = dst.as_mut_ptr() as *mut core::arch::x86_64::__m256i;

            for _ in 0..steps {
                *dst_ptr = _mm256_adds_epi16(*src_ptr, *dst_ptr);

                src_ptr = src_ptr.add(1);
                dst_ptr = dst_ptr.add(1);
            }

            for i in (steps * 16)..src.len() {
                let dst_ref = dst.get_unchecked_mut(i);
                *dst_ref = dst_ref.saturating_add(*src.get_unchecked(i));
            }
        }
        */
    }
}
