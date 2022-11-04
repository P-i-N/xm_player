#![feature(unchecked_math)]
#![feature(stdsimd)]

mod platform;
mod xm_player;

use std::error;

fn main() -> Result<(), Box<dyn error::Error>> {
    let sample_rate = 48000;
    let platform = platform::create_platform_interface(sample_rate);

    let module = xm_player::Module::load("../../song.xm")?;

    let mut player = xm_player::Player::new(&module, platform.as_ref(), sample_rate, 1);

    println!("Benchmarking...");
    println!("Elapsed time: {}ms", player.benchmark().as_millis());

    //return Ok(());

    player.print_rows = true;

    let mut buffer = [0 as i16; 48000 * 2];

    while platform.audio_wait() {
        let samples_to_render = platform.get_available_samples();
        player.render(&mut buffer[0..samples_to_render]);
        platform.audio_render(&buffer[0..samples_to_render]);
    }

    Ok(())
}
