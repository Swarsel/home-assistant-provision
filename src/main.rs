mod api;
mod declaration;
mod flow;
mod onboarding;

use std::{fs, path::PathBuf, time::Duration};

use anyhow::{Context, Result};
use clap::Parser;

#[derive(Parser)]
#[command(version, about)]
struct Args {
    #[arg(long, default_value = "http://127.0.0.1:8123")]
    url: String,
    #[arg(long)]
    declaration: PathBuf,
    #[arg(long)]
    password_file: PathBuf,
    #[arg(long, default_value_t = 300)]
    wait: u64,
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    let args = Args::parse();
    let declaration = declaration::load(&args.declaration)?;
    let password = fs::read_to_string(&args.password_file)
        .with_context(|| format!("reading {}", args.password_file.display()))?;

    let mut api = api::Api::new(&args.url);
    api.wait_until_ready(Duration::from_secs(args.wait))?;
    onboarding::authenticate(
        &mut api,
        &declaration.owner,
        password.trim_end_matches(['\n', '\r']),
    )?;
    log::info!("provisioning finished");
    Ok(())
}
