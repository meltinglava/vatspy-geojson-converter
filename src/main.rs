use std::error::Error;

pub(crate) mod fir_boundaries;
pub(crate) mod geo_json;

fn main() -> Result<(), Box<dyn Error>> {
    let data = fir_boundaries::read_file()?;
    fir_boundaries::write_to_file(&data)?;
    Ok(())
}
