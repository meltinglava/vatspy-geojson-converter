use std::{error::Error, fs::File};

use geo_json::GeoJson;

use clap::Clap;
use either::{Either::{Left, Right}};

pub(crate) mod fir_boundaries;
pub(crate) mod geo_json;

mod cli;

fn main() -> Result<(), Box<dyn Error>> {
    let opts = cli::Opts::parse();
    let data = match opts.input.extension().map(|os| os.to_str().unwrap()) {
        Some("json") | Some("geojson") => Left(serde_json::from_reader::<_, GeoJson>(File::open(opts.input)?)?),
        Some("dat") => Right(fir_boundaries::read_file(opts.input)?),
        Some(e) => return Err(format!("Unrecognized file extention: .{}. run --help for more info", e).into()),
        None => return Err("No file extention found. run --help for more info".into()),
    };

    if let Some(f) = opts.output {
        match data {
            Left(geojson_data) => {
                let fir_data = fir_boundaries::convert_from_geojson(geojson_data);
                fir_boundaries::write_to_file(&fir_data, f)?;
            },
            Right(fir_data) => {
                let gj: GeoJson = fir_data.into();
                serde_json::to_writer_pretty(File::create(f)?, &gj)?;
            },
        }
    }
    Ok(())
}
