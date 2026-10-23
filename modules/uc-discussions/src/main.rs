#![forbid(unsafe_code)]

use std::env;
use uc_discussions::{cli, manifest, DiscussionsHandler};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // If a CLI domain command, probe, or flag was handled, exit cleanly.
    if cli::run_cli(env::args())? {
        return Ok(());
    }

    // Default supervisor mode: serve discussions management surface over SubC
    let handler = DiscussionsHandler::try_default()?;
    subc_client_rs::serve(manifest(), handler).await?;
    Ok(())
}
