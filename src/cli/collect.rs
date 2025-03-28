/// Module holding the `collect` command
/// 
/// This module collects the data from the repositories and stores it in the database.
/// We do this using the GitHub API.
use url::Url;
use clap::ArgMatches;
use miette::{miette, Result, IntoDiagnostic};
use octocrab::models::IssueState;
use octocrab::params::State;
use octocrab::{Octocrab, models::pulls::PullRequest};

use crate::constants::CLI_ARGS_REPO;
use crate::database::{setup_db, Repository};

pub async fn collect(matches: &ArgMatches) -> Result<()> {
    let github_api_token = std::env::var("GITHUB_TOKEN")
        .map_err(|_| miette!("GitHub token not found. Please set GITHUB_TOKEN environment variable"))?;

    let project_name= matches
        .get_one::<String>(CLI_ARGS_REPO)
        .expect("repository is required");

    let pool = setup_db().await.into_diagnostic()?;
    let repo = Repository::from(&pool, project_name).await.into_diagnostic()?;

    let octocrab = Octocrab::builder()
        .personal_token(github_api_token)
        .build()
        .into_diagnostic()?;
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

    println!("Pull Requests:");

    get_pull_requests(&octocrab, &repo.owner, &repo.name).await.into_diagnostic()?;

    Ok(())
}


pub async fn get_pull_requests(octocrab: &Octocrab, owner: &str, repo: &str) -> Result<(), octocrab::Error> {
    let mut current_page = octocrab
        .pulls(owner, repo)
        .list()
        .state(State::All)
        .per_page(100)
        .send()
        .await?;

    for pr in current_page.take_items(){
        let state_display = match pr.state.unwrap_or(IssueState::Open) {
            IssueState::Closed => "closed".to_string(),
            IssueState::Open => "open".to_string(),
            _ => "unknown".to_string(),
        };

        println!("- {}: {}", pr.title.unwrap_or_default(), pr.number);
        println!("  URL: {}", pr.html_url.unwrap_or(Url::parse("https://github.com").unwrap()));
        println!("  Comments URL: {}", pr.comments_url.unwrap_or(Url::parse("https://github.com").unwrap()));
        println!("  State: {}", state_display);
        println!("  Created at: {}", pr.created_at.unwrap_or_default());
        println!("  Updated at: {}", pr.updated_at.unwrap_or_default());
        println!("  Merged at: {:?}", pr.merged_at.unwrap_or_default());
        println!();
    }

    while let Ok(Some(new_page)) = octocrab.get_page::<PullRequest>(&current_page.next).await {
        current_page = new_page;
        for pr in current_page.take_items() {
            let state_display = match pr.state.unwrap_or(IssueState::Open) {
                IssueState::Closed => "closed".to_string(),
                IssueState::Open => "open".to_string(),
                _ => "unknown".to_string(),
            };

            println!("- {}: {}", pr.title.unwrap_or_default(), pr.number);
            println!("  URL: {}", pr.html_url.unwrap_or(Url::parse("https://github.com").unwrap()));
            println!("  Comments URL: {}", pr.comments_url.unwrap_or(Url::parse("https://github.com").unwrap()));
            println!("  State: {}", state_display);
            println!("  Created at: {}", pr.created_at.unwrap_or_default());
            println!("  Updated at: {}", pr.updated_at.unwrap_or_default());
            println!("  Merged at: {:?}", pr.merged_at.unwrap_or_default());
            println!();
        }

    }

    Ok(())
}