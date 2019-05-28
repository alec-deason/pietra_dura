use structopt::StructOpt;
use std::io::Result;

use tiled_preprocessor::{
    SimpleConverter, RealLevelPrefab, TiledConverter,
};

#[derive(StructOpt, Debug)]
struct Cli {
    #[structopt(parse(from_os_str))]
    map: std::path::PathBuf,
    #[structopt(parse(from_os_str))]
    output: std::path::PathBuf,
}

fn main() -> Result<()> {
    let args = Cli::from_args();
    SimpleConverter::<RealLevelPrefab>::from_map(&args.map)
        .write(&args.output)?;
    Ok(())
}
