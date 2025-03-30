/// Module holding the `collect` command
/// 
/// This module collects the data from the repositories and stores it in the database.
/// We do this using the GitHub API.
use std::fmt::Display;
use clap::ArgMatches;
use miette::{miette, Result, IntoDiagnostic};
use octocrab::models::pulls::PullRequest;
use octocrab::models::{pulls, IssueState};
use octocrab::params::State;
use octocrab::Octocrab;
use sqlx::{Pool, Sqlite};
use tokio::try_join;
use tokio::sync::Semaphore;
use std::sync::{Arc, Mutex};

use crate::constants::CLI_ARGS_REPO;
use crate::database::{setup_db, Repository};

#[derive(Debug)]
struct CollectionError;

impl Display for CollectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Collection error")
    }
}

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

    if let Some(total_pages) = get_total_pages(&octocrab, &repo.owner, &repo.name).await.into_diagnostic()? {
        println!("Total pages: {}", total_pages);
        collect_pull_requests(&octocrab, &pool, repo.owner.clone(), repo.name.clone(), total_pages).await?;
    } else {
        println!("No pages found");
    }
    // get_pull_requests(&octocrab, &repo.owner, &repo.name).await.into_diagnostic()?;

    Ok(())
}

pub async fn collect_reviews(octocrab: &Octocrab, pool: &Pool<Sqlite>, owner: &str, repo: &str, pr_number: u32) -> Result<()> {
    let reviews = octocrab.pulls(owner, repo)
        .list_reviews(pr_number as u64)
        .page(1u32)
        .per_page(100)
        .send()
        .await.into_diagnostic()?;

    // Write the code for saving the reviews to the database here
    

    Ok(())
}

pub async fn collect_events(octocrab: &Octocrab, pool: &Pool<Sqlite>, owner: &str, repo: &str, pr_number: u32) -> Result<()> {
    let events = octocrab.issues(owner, repo)
        .list_timeline_events(pr_number as u64)
        .page(1u32)
        .per_page(100)
        .send()
        .await.into_diagnostic()?;


    // Process events here

    Ok(())
}

pub async fn collect_pull_requests(
    octocrab: &Octocrab,
    pool: &Pool<Sqlite>,
    owner: String,
    repo: String,
    total_pages: u32
) -> Result<(), miette::Error> {
    let concurrent_requests = 5;
    let sem = Arc::new(Semaphore::new(concurrent_requests));
    let octocrab_rc = Arc::new(octocrab.clone());
    let pool = Arc::new(pool.clone());

    let mut results = Vec::new();
    let mut pulls: Arc<Mutex<Vec<PullRequest>>> = Arc::new(Mutex::new(Vec::new()));

    for page in 1..=total_pages {
        let octocrab_a = octocrab_rc.clone();
        let pool_a = pool.clone();
        let owner_a = owner.clone();
        let repo_a = repo.clone();
        let permit = Arc::clone(&sem).acquire_owned().await;
        let pulls = pulls.clone();

        let result = tokio::spawn(async move {
            let _permit = permit;

            let mut page = match octocrab_a
                .pulls(owner_a, repo_a)
                .list()
                .state(State::All)
                .per_page(100)
                .page(page)
                .send()
                .await
            {
                Ok(response) => response,
                Err(_err) => {
                    eprintln!("Error fetching page {}", page);
                    return;
                }
            };
            drop(_permit);

            pulls.lock().unwrap().extend(page.take_items());

        });
        results.push(result);
    }

    for result in results {
        result.await.map_err(|_| miette!("Error collecting pull requests"))?;
    }
    let pulls = pulls.lock().unwrap();
    println!("Collected {} pull requests", pulls.len());

    Ok(())
}


/// This is a pretty brittle and naive approach to getting the total number of pages.
/// It relies on the fact that the GitHub API returns a `Link` header with a `rel="last"`
/// 
/// It could be improved by writing a better parser for the `Link` header.
pub async fn get_total_pages(octocrab: &Octocrab, owner: &str, repo: &str) -> Result<Option<u32>, octocrab::Error> {
    let state = "all";
    let per_page = 100;
    let response = octocrab
        ._get(format!("https://api.github.com/repos/{}/{}/pulls?state={}&per_page={}", owner, repo, state, per_page))
        .await?;

    if response.headers().get("Link").is_some() {
        let link_header = response.headers().get("Link").unwrap().to_str().unwrap();
        let last_page_link = link_header.split(",").find(|link| link.contains("rel=\"last\""));
        if let Some(last_page) = last_page_link {
            let last_page_number = last_page.split(";").next().unwrap();
            let last_page_number = last_page_number.split("&").last().unwrap().split("=").last().unwrap();
            return Ok(Some(last_page_number.strip_suffix(">").unwrap().parse::<u32>().unwrap()));
        }
    }

    Ok(None) // Return None if no "last" page link is found
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

        let pulls_api = octocrab.pulls(owner, repo);
        let reviews_fut = pulls_api
            .list_reviews(pr.number)
            .page(1u32)
            .per_page(100)
            .send();
        let issues_api = octocrab.issues(owner, repo);
        let events_fut = issues_api
            .list_timeline_events(pr.number)
            .page(1u32)
            .per_page(100)
            .send();

        let (reviews, events) = try_join!(reviews_fut, events_fut)?;

        println!("  - Reviews:");
        for review in reviews {
            println!("    - {}: {:?}", review.user.unwrap().login, review.state);
            println!("      Created at: {}", review.submitted_at.unwrap_or_default());
        }
        println!();

        println!("  - Events:");
        for event in events {
            if event.actor.is_some() {
                println!("    - Actor: {}", event.actor.unwrap().login);
                println!("      Event: {:?}", event.event);
                println!("      Created at: {}", event.created_at.unwrap_or_default());
            }
        }
        println!();
    }

    // while let Ok(Some(new_page)) = octocrab.get_page::<PullRequest>(&current_page.next).await {
    //     current_page = new_page;
    //     for pr in current_page.take_items() {
    //         let state_display = match pr.state.unwrap_or(IssueState::Open) {
    //             IssueState::Closed => "closed".to_string(),
    //             IssueState::Open => "open".to_string(),
    //             _ => "unknown".to_string(),
    //         };

    //         println!("- {}: {}", pr.title.unwrap_or_default(), pr.number);
    //         println!("  URL: {}", pr.html_url.unwrap_or(Url::parse("https://github.com").unwrap()));
    //         println!("  Comments URL: {}", pr.comments_url.unwrap_or(Url::parse("https://github.com").unwrap()));
    //         println!("  State: {}", state_display);
    //         println!("  Created at: {}", pr.created_at.unwrap_or_default());
    //         println!("  Updated at: {}", pr.updated_at.unwrap_or_default());
    //         println!("  Merged at: {:?}", pr.merged_at.unwrap_or_default());
    //         println!();
    //     }

    // }

    Ok(())
}
