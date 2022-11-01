#![feature(unchecked_math)]

mod audio_interface;
mod xm_player;

use std::error;

fn main() -> Result<(), Box<dyn error::Error>> {
    let module = xm_player::Module::load("../../deadlock.xm")?;

    let mut player = xm_player::Player::new(&module, 48000);

    println!("Benchmarking...");
    //println!("Elapsed time: {}ms", player.benchmark().as_millis());

    //return Ok(());

    player.print_rows = true;

    let audio_iface = audio_interface::create_audio_interface().unwrap();
    let mut buffer = [0 as i16; 48000 * 2];

    while audio_iface.wait() {
        let samples_to_render = audio_iface.get_available_samples();
        player.render(&mut buffer[0..samples_to_render]);
        audio_iface.render(&buffer[0..samples_to_render]);
    }

    Ok(())
}
