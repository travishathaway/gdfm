/// Module holding the `init` command
/// 
/// The module initializes a new project by asking for the
/// repositories to keep track of.
/// 
use console::style;
use clap::ArgMatches;
use miette::{Result, IntoDiagnostic};

use crate::constants::CLI_ARGS_MAINTAINERS;
use crate::constants::CLI_ARGS_REPO;
use crate::database::{setup_db, Repository, Maintainer};

pub async fn init(matches: &ArgMatches) -> Result<()> {
    let repo_path: &String = matches
        .get_one::<String>(CLI_ARGS_REPO)
        .expect("repos are required");

    let maintainers: Vec<&str> = matches
        .get_many::<String>(CLI_ARGS_MAINTAINERS)
        .expect("at least one maintainer is required")
        .map(String::as_str)
        .collect();

    let pool = setup_db().await.into_diagnostic()?;

    let maintainers = Maintainer::create_many(&pool, maintainers).await.into_diagnostic()?;
    let repo = Repository::create(&pool, repo_path).await.into_diagnostic()?;

    repo.add_maintainers(&pool, &maintainers).await.into_diagnostic()?;

    println!();
    println!("Tracking the following GitHub repo: {}", style(repo_path).bold().cyan());
    println!("With the following maintainers:");
    for maintainer in &maintainers {
        println!(" - {}", style(&maintainer.username).bold().green());
    }

    Ok(())
}
