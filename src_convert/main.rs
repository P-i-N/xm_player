use std::error;

mod channel;
use channel::Channel;

mod builder;
use builder::*;

mod formats;
use formats::*;

fn main() -> Result<(), Box<dyn error::Error>> {
    let data = std::fs::read("../../song.xm")?;

    let mut builder = Builder::new();
    convert_xm(&mut builder, &data)?;

    let data = builder.build();
    std::fs::write("../../song.um", &data)?;

    Ok(())
}
