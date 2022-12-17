#![feature(unchecked_math)]
#![feature(stdsimd)]

use std::error;
use std::time::Instant;

#[cfg(target_os = "windows")]
mod wasapi_audio_driver;

#[cfg(target_os = "windows")]
use wasapi_audio_driver::WasapiAudioDriver as AudioDriver;

use xmplay::Module;
use xmplay::Player;

mod row_printer;
use row_printer::RowPrinter;

fn main() -> Result<(), Box<dyn error::Error>> {
    // Sample rate in Hz
    const SAMPLE_RATE: usize = 48000;

    let module = Module::from_memory(include_bytes!("../../temp/deadlock.xm"))?;
    let mut player = Player::new(&module, SAMPLE_RATE, 1);

    // Benchmark
    println!("Benchmarking...");
    for _ in 0..0 {
        let time_start = Instant::now();
        player.benchmark();
        println!("Elapsed time: {} ms", time_start.elapsed().as_millis());
    }

    let mut row_printer = RowPrinter::new();
    player.set_tick_callback(move |cb_pos, song_state, module| {
        row_printer.tick(cb_pos, song_state, module);
    });

    // Play song using audio driver
    {
        let audio_driver = AudioDriver::new(SAMPLE_RATE).unwrap();

        let mut buffer = [0 as i16; SAMPLE_RATE * 2];
        while audio_driver.wait() {
            let samples_to_render = audio_driver.get_available_samples();
            player.render(&mut buffer[0..samples_to_render]);
            audio_driver.render(&buffer[0..samples_to_render]);
        }
    }

    Ok(())
}
