use std::error::Error;

mod fir_boundaries;

fn main() -> Result<(), Box<dyn Error>> {
    let data = fir_boundaries::read_file()?;
    fir_boundaries::write_to_file(&data);
    Ok(())
}
