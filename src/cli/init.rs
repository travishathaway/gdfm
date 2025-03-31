/// Module holding the `init` command
/// 
/// The module initializes a new project by asking for the
/// repositories to keep track of.
/// 
use console::style;
use clap::ArgMatches;
use miette::{Result, IntoDiagnostic};

use crate::constants::CLI_ARGS_REPO;
use crate::database::{setup_db, Repository};

pub async fn init(matches: &ArgMatches) -> Result<()> {
    let repo_path: &String = matches
        .get_one::<String>(CLI_ARGS_REPO)
        .expect("repos are required");

    let pool = setup_db().await.into_diagnostic()?;

    let repo = Repository::create(&pool, repo_path).await.into_diagnostic()?;

    println!();
    println!(
        "Tracking the following GitHub repo: {}/{}",
        style(repo.owner).bold().cyan(),
        style(repo.name).bold().cyan()
    );

    Ok(())
}
