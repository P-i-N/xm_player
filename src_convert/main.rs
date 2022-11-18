use std::{error, path::PathBuf};

mod event_stream;
pub use event_stream::*;

mod benvelope;
pub use benvelope::*;

mod builder;
use builder::*;

mod formats;
use formats::*;

fn main() -> Result<(), Box<dyn error::Error>> {
    let mut file_name = PathBuf::from("../../alf.xm");
    let data = std::fs::read(&file_name)?;

    let mut builder = Builder::new();
    convert_xm(&mut builder, &data)?;

    let um_data = builder.build();

    file_name.set_extension("um");
    std::fs::write(&file_name, &um_data)?;

    let module_desc = xm_player::ModuleDesc::new(&um_data)?;

    Ok(())
}
