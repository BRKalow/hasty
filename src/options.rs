use std::path::PathBuf;

use clap::Parser;
use serde::Deserialize;

#[derive(Parser)]
#[clap(author, version, about, long_about = None)]
#[derive(Deserialize)]
pub struct HastyOptions {
    /// The directory of the project
    #[arg(short, long)]
    pub dir: Option<PathBuf>,

    /// Whether or the script should actually be executed
    #[arg(long)]
    pub dry_run: bool,

    /// The script to execute
    pub script: Option<String>,
}
