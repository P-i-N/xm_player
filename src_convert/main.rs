use std::error;

mod channel;
use channel::Channel;

mod builder;
use builder::*;

mod formats;
use formats::*;

fn main() -> Result<(), Box<dyn error::Error>> {
    let data = std::fs::read("../../deadlock.xm")?;

    let mut builder = Builder::new();
    convert_xm(&mut builder, &data)?;

    let data = builder.build();
    std::fs::write("../../deadlock.um", &data)?;

    Ok(())
}
