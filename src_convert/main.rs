use std::{error, path::PathBuf};

mod channel;
use channel::*;

mod builder;
use builder::*;

mod formats;
use formats::*;

fn main() -> Result<(), Box<dyn error::Error>> {
    let mut file_name = PathBuf::from("../../deadlock.xm");
    let data = std::fs::read(&file_name)?;

    let mut builder = Builder::new();
    convert_xm(&mut builder, &data)?;

    let data = builder.build();

    file_name.set_extension("um");
    std::fs::write(&file_name, &data)?;

    Ok(())
}
