use std::path::PathBuf;

use clap::{AppSettings, Clap, crate_version};

#[derive(Clap)]
#[clap(version = crate_version!(), author = "meltinglava. <meltinglavaoutland@gmail.com>")]
#[clap(setting = AppSettings::ColoredHelp)]
pub(crate) struct Opts {
    /// Input file input. This has to end with .bat or .geojson/.json.
    pub(crate) input: PathBuf,
    /// If this argument is missing only validation will be done. If
    /// it is given the oposite file will be generated. Note that this
    /// file has the same requirements with file-endings as the input field.
    pub(crate) output: Option<PathBuf>,
}
