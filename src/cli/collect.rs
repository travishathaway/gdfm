/// Module holding the `collect` command
/// 
/// This module collects the data from the repositories and stores it in the database.
/// We do this using the GitHub API.

use clap::ArgMatches;
use miette::{miette, Result, IntoDiagnostic};
use octocrab::Octocrab;

use crate::constants::CLI_ARGS_NAME;
use crate::database::{setup_db, Project};

pub async fn collect(matches: &ArgMatches) -> Result<()> {
    let github_api_token = std::env::var("GITHUB_TOKEN")
        .map_err(|_| miette!("GitHub token not found. Please set GITHUB_TOKEN environment variable"))?;

    let project_name= matches
        .get_one::<String>(CLI_ARGS_NAME)
        .expect("name is required");

    let pool = setup_db().await.into_diagnostic()?;
    let project = Project::from(&pool, project_name).await.into_diagnostic()?;
    let repos = project.repositories(&pool).await.into_diagnostic()?;

    let octocrab = Octocrab::builder()
        .personal_token(github_api_token)
        .build()
        .into_diagnostic()?;

    for repo in repos {
        let gh_repo = octocrab
            .repos(&repo.owner, &repo.name)
            .get()
            .await
            .into_diagnostic()?;

        println!("Repo: {}", repo.name);
        println!("Description: {}", gh_repo.description.unwrap_or_default());
        println!("Stars: {}", gh_repo.stargazers_count.unwrap_or_default());
        println!("Forks: {}", gh_repo.forks_count.unwrap_or_default());
        println!("Issues: {}", gh_repo.open_issues_count.unwrap_or_default());
        println!("Watchers: {}", gh_repo.watchers_count.unwrap_or_default());
        println!("Language: {}", gh_repo.language.unwrap_or_default());
        println!("License: {}", gh_repo.license.unwrap().description.unwrap_or_default());
        println!();
    }

    Ok(())
}