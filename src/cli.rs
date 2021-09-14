use std::path::PathBuf;

use clap::{AppSettings, Clap, crate_version};

#[derive(Clap)]
#[clap(version = crate_version!(), author = "meltinglava. <meltinglavaoutland@gmail.com>")]
#[clap(setting = AppSettings::ColoredHelp)]
pub(crate) struct Opts {
    /// Input file input. This has to end with .bat or .geojson/.json.
    pub(crate) input: PathBuf,
    /// If this argument is missing only validation will be done.
    /// If this file is the same type. Fixes will be applied to that file.
    /// If this file is of the other type. It will be converted and filled into the other file.
    pub(crate) output: Option<PathBuf>,
}
