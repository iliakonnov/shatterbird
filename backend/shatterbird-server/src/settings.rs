use clap::Parser;
use eyre::OptionExt;
use shatterbird_utils::cli::load_args;
use std::sync::OnceLock;

static SETTINGS: OnceLock<Settings> = OnceLock::new();

pub fn get() -> eyre::Result<&'static Settings> {
    SETTINGS.get().ok_or_eyre("failed to get settings")
}

pub(super) fn init() -> eyre::Result<()> {
    let args = load_args();
    SETTINGS
        .set(args)
        .map_err(|_| eyre::anyhow!("failed to set settings"))?;
    Ok(())
}

#[derive(Parser)]
pub struct Settings {
    #[arg(long)]
    pub db_url: String,

    #[arg(long, default_value = "127.0.0.1:3000")]
    pub addr: String,
}
