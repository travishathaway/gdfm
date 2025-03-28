/// Module for commands that removes the database file
use clap::ArgMatches;
use miette::{Result, IntoDiagnostic};

use crate::constants::CLI_ARGS_YES;
use crate::database::destroy_db;

pub async fn clean(matches: &ArgMatches) -> Result<()> {
    let force = matches.get_flag(CLI_ARGS_YES);

    if force {
        destroy_db().await.into_diagnostic()?;
    } else {
        let confirm = dialoguer::Confirm::new()
            .with_prompt("Are you sure you want to remove the database file and all data collected?")
            .interact()
            .into_diagnostic()?;

        if confirm {
            destroy_db().await.into_diagnostic()?;
        }
    }

    Ok(())
}