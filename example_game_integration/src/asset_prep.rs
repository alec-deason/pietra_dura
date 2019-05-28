use structopt::StructOpt;
use std::path::PathBuf;
use std::io::Result;

use pietra_dura_tiled::TiledConverter;
use example_game_integration::LevelPrefab;

#[derive(StructOpt, Debug)]
struct Cli {
    #[structopt(parse(from_os_str))]
    map: std::path::PathBuf,
}

fn main() -> Result<()> {
    let args = Cli::from_args();
    LevelPrefab::from_map(&args.map, &PathBuf::from("map"))
        .write(&PathBuf::from("assets/map"))?;
    Ok(())
}

