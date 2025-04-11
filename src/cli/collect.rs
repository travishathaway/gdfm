/// Module holding the `collect` command
/// 
/// This module collects the data from the repositories and stores it in the database.
/// We do this using the GitHub API.
use clap::ArgMatches;
use miette::{miette, Result, IntoDiagnostic};
use octocrab::params::State;
use octocrab::Octocrab;
use indicatif::{ProgressBar, ProgressStyle};
use tokio::time::{sleep, Duration};

use crate::constants::{
    CLI_ARGS_REPO,
    CLI_ARGS_NUMBER
};
use crate::database::{
    setup_db,
    Repository,
    PullRequest as DbPullRequest,
    PullRequestReview,
    PullRequestEvent
};

pub async fn collect_pull_requests(matches: &ArgMatches) -> Result<()> {
    let github_api_token = std::env::var("GITHUB_TOKEN")
        .map_err(|_| miette!("GitHub token not found. Please set GITHUB_TOKEN environment variable"))?;

    let project_name= matches
        .get_one::<String>(CLI_ARGS_REPO)
        .expect("repository is required");
    
    let pool = setup_db().await.into_diagnostic()?;
    let repo_db = Repository::from(&pool, project_name).await.into_diagnostic()?;

    let octocrab = Octocrab::builder()
        .personal_token(github_api_token)
        .build()
        .into_diagnostic()?;

    let per_page = 100;

    let total_prs = get_total_pull_requests(
        &octocrab, &repo_db.owner, &repo_db.name
    ).await.into_diagnostic()?;   

    if total_prs > 0 {
        let total_pages = (total_prs as f64 / per_page as f64).ceil() as u32;
        let progress_bar = get_progress_bar(total_prs as u64, "Fetching pull requests");

        for page in 1..=total_pages {
            let pulls = octocrab
                .pulls(&repo_db.owner, &repo_db.name)
                .list()
                .state(State::Closed)
                .per_page(per_page)
                .page(page)
                .send()
                .await.into_diagnostic()?;

            for pull in pulls {
                let _pull_db = DbPullRequest::create(&pool, &pull, repo_db.id).await.into_diagnostic()?;
                progress_bar.inc(1);
            }
        }
    } else {
        println!("No pull requests found");
    }
    Ok(())
}

/// Parses the command line arguments and figures out what operations to perform
pub async fn collect_pull_events(matches: &ArgMatches) -> Result<()> {
    let github_api_token = std::env::var("GITHUB_TOKEN")
        .map_err(|_| miette!("GitHub token not found. Please set GITHUB_TOKEN environment variable"))?;

    let project_name= matches
        .get_one::<String>(CLI_ARGS_REPO)
        .expect("repository is required");

    let pr_numbers = match matches.get_many(CLI_ARGS_NUMBER) {
        Some(numbers) => numbers.copied().collect(),
        None => vec![],
    };

    let pool = setup_db().await.into_diagnostic()?;
    let repo = Repository::from(&pool, project_name).await.into_diagnostic()?;
    let pulls = DbPullRequest::fetch_many(&pool, repo.id, &pr_numbers).await.into_diagnostic()?;

    println!("Length {}: {}", pulls.len(), pr_numbers.len());
    // Number of numbers provided should match records fetched from the database
    if !pr_numbers.is_empty() && pulls.len() != pr_numbers.len()  {
        return Err(miette!("Number of pull requests provided does not match the number of records in the database"));
    }

    let octocrab = Octocrab::builder()
        .personal_token(github_api_token)
        .build()
        .into_diagnostic()?;

    let progress_bar = get_progress_bar(pulls.len() as u64, "Fetching pull events");

    for pull in pulls {
        let events = octocrab.issues(&repo.owner, &repo.name)
            .list_timeline_events(pull.number as u64)
            .page(1u32)
            .per_page(100)
            .send()
            .await.into_diagnostic()?;

        for event in events {
            if event.id.is_some() {
                PullRequestEvent::create(&pool, pull.id, &event).await.map_err(|err| {
                    miette!("Error creating pull request event db record: {}", err)
                })?;
            }
        }
        progress_bar.inc(1);
        sleep(Duration::from_millis(1000)).await;
    }
    progress_bar.finish_with_message("Finished fetching pull request events");

    Ok(())
}

pub async fn collect_pull_reviews(matches: &ArgMatches) -> Result<()> {
    let github_api_token = std::env::var("GITHUB_TOKEN")
        .map_err(|_| miette!("GitHub token not found. Please set GITHUB_TOKEN environment variable"))?;

    let project_name= matches
        .get_one::<String>(CLI_ARGS_REPO)
        .expect("repository is required");

    let pr_numbers = match matches.get_many(CLI_ARGS_NUMBER) {
        Some(numbers) => numbers.copied().collect(),
        None => vec![],
    };

    let pool = setup_db().await.into_diagnostic()?;
    let repo = Repository::from(&pool, project_name).await.into_diagnostic()?;
    let pulls = DbPullRequest::fetch_many(&pool, repo.id, &pr_numbers).await.into_diagnostic()?;

    // Number of numbers provided should match records fetched from the database
    if !pr_numbers.is_empty() && pulls.len() != pr_numbers.len()  {
        return Err(miette!("Number of pull requests provided does not match the number of records in the database"));
    }

    let octocrab = Octocrab::builder()
        .personal_token(github_api_token)
        .build()
        .into_diagnostic()?;

    let progress_bar = get_progress_bar(pulls.len() as u64, "Fetching pull request reviews");

    for pull in pulls {
        let reviews = octocrab.pulls(&repo.owner, &repo.name)
            .list_reviews(pull.number as u64)
            .page(1u32)
            .per_page(100)
            .send()
            .await.into_diagnostic()?;

        for review in reviews {
            PullRequestReview::create(&pool, pull.id, &review).await.map_err(|err| {
                miette!("Error creating pull request event db record: {}", err)
            })?;
        }
        progress_bar.inc(1);
        sleep(Duration::from_millis(1000)).await;
    }
    progress_bar.finish_with_message("Finished fetching pull request reviews");

    Ok(())
}

/// Creates a standard progress bar with a custom message
pub fn get_progress_bar(total: u64, message: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(ProgressStyle::default_bar()
        .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",)
        .unwrap()
        .progress_chars("##-"));
    pb.set_message(message.to_string());
    pb
}

pub async fn get_total_pull_requests(octocrab: &Octocrab, owner: &str, repo: &str) -> Result<u32, octocrab::Error> {
    let search = format!("repo:{}/{} is:pr", owner, repo);
    let results = octocrab.search()
        .issues_and_pull_requests(&search)
        .per_page(1)
        .send()
        .await?;

    if let Some(count) = results.total_count {
        Ok(count as u32)
    } else {
        Ok(0u32)
    }

}
