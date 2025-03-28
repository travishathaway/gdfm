/// Module holding the `init` command
/// 
/// The module initializes a new project by asking for the
/// repositories to keep track of.
/// 
use console::style;
use clap::ArgMatches;
use miette::{Result, IntoDiagnostic};

use crate::constants::CLI_ARGS_NAME;
use crate::constants::CLI_ARGS_REPOS;
use crate::database::{setup_db, Project, Repository};

pub async fn init(matches: &ArgMatches) -> Result<()> {
    let project_name= matches
        .get_one::<String>(CLI_ARGS_NAME)
        .expect("name is required");

    let repo_paths: Vec<&str> = matches
        .get_many::<String>(CLI_ARGS_REPOS)
        .expect("repos are required")
        .map(String::as_str)
        .collect();

    let pool = setup_db().await.into_diagnostic()?;

    let project = Project::create(&pool, project_name).await.into_diagnostic()?;

    let repos = Repository::create_many(&pool, project.id, repo_paths).await.into_diagnostic()?;

    println!();
    println!("Project {} is tracking the following GitHub repos:", style(project.name).bold().cyan());
    
    for repo in repos {
        println!("- {}/{}", repo.owner, repo.name);
    }

    Ok(())
}
