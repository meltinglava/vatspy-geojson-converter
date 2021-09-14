use std::{error::Error, fs::File};

use geo_json::GeoJson;

pub(crate) mod fir_boundaries;
pub(crate) mod geo_json;

fn main() -> Result<(), Box<dyn Error>> {
    let data = fir_boundaries::read_file()?;
    fir_boundaries::write_to_file(&data, "fir_parsed.fir")?;
    let gj: GeoJson = data.into();
    let gj_file = File::create("fir.geojson")?;
    serde_json::to_writer_pretty(gj_file, &gj)?;
    let gj_str = serde_json::to_string(&gj)?;
    let gj_from: GeoJson = serde_json::from_str(&gj_str)?;
    let data1 = fir_boundaries::convert_from_geojson(gj_from);
    fir_boundaries::write_to_file(&data1, "from_geo.fir")?;
    Ok(())
}
