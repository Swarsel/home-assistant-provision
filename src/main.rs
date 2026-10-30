mod declaration;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

#[derive(Parser)]
#[command(version, about)]
struct Args {
    #[arg(long, default_value = "http://127.0.0.1:8123")]
    url: String,
    #[arg(long)]
    declaration: PathBuf,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Args::parse();
    let declaration = declaration::load(&args.declaration)?;
    log::info!(
        "provisioning {} as {}",
        args.url,
        declaration.owner.username
    );
    Ok(())
}
