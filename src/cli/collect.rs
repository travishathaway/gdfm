/// Module holding the `collect` command
/// 
/// This module collects the data from the repositories and stores it in the database.
/// We do this using the GitHub API.
use clap::ArgMatches;
use miette::{miette, Result, IntoDiagnostic};
use octocrab::models::pulls::PullRequest;
use octocrab::params::State;
use octocrab::Octocrab;
use sqlx::{Pool, Sqlite};
use tokio::try_join;
use tokio::sync::Semaphore;
use std::sync::{Arc, Mutex};
use indicatif::{MultiProgress,ProgressBar, ProgressStyle};

use crate::constants::CLI_ARGS_REPO;
use crate::database::{setup_db, Repository, PullRequest as DbPullRequest, PullRequestReview, PullRequestEvent};

/// Maximum number of concurrent requests to the GitHub API
const MAX_CONCURRENT_REQUESTS: usize = 5;

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
        collect_pull_requests(&octocrab, &pool, repo.owner.clone(), repo.name.clone(), 1).await?;
    } else {
        println!("No pages found");
    }
    // get_pull_requests(&octocrab, &repo.owner, &repo.name).await.into_diagnostic()?;

    Ok(())
}

pub async fn collect_reviews(
    octocrab: &Octocrab,
    pool: &Pool<Sqlite>,
    owner: &str,
    repo: &str,
    pull_request: &DbPullRequest,
) -> Result<()> {
    let reviews = octocrab.pulls(owner, repo)
        .list_reviews(pull_request.number as u64)
        .page(1u32)
        .per_page(100)
        .send()
        .await.into_diagnostic()?;

    for review in reviews {
        PullRequestReview::create(pool, pull_request.id, &review).await.map_err(|err| {
            miette!("Error creating pull request review db record: {}", err)
        })?;
    }

    Ok(())
}

pub async fn collect_events(
    octocrab: &Octocrab,
    pool: &Pool<Sqlite>,
    owner: &str,
    repo: &str,
    pull_request: &DbPullRequest,
) -> Result<()> {
    let events = octocrab.issues(owner, repo)
        .list_timeline_events(pull_request.number as u64)
        .page(1u32)
        .per_page(100)
        .send()
        .await.into_diagnostic()?;

    for event in events {
        if event.created_at.is_some() {
            PullRequestEvent::create(pool, pull_request.id, &event).await.map_err(|err| {
                miette!("Error creating pull request event db record: {}", err)
            })?;
        }
    }

    Ok(())
}

/// Creates a standard progress bar with a custom message
pub fn get_progress_bar(total: u64, message: &str) -> ProgressBar {
    let pb = ProgressBar::new(total);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} {wide_msg} [{elapsed_precise}] {bar:40} {bytes}/{total_bytes} ({percent}%)")
        .unwrap()
        .progress_chars("##-"));
    pb.set_message(message.to_string());
    pb
}

pub async fn collect_pull_requests(
    octocrab: &Octocrab,
    pool: &Pool<Sqlite>,
    owner: String,
    repo: String,
    total_pages: u32
) -> Result<(), miette::Error> {
    let sem = Arc::new(Semaphore::new(MAX_CONCURRENT_REQUESTS));
    let octocrab_rc = Arc::new(octocrab.clone());
    let pool = Arc::new(pool.clone());
    let repo_db = Repository::from(&pool, &format!("{}/{}", owner, repo)).await.map_err(|err| {
        miette!("Error getting repository from database: {}", err)
    })?;

    // Create progress bars
    let multi_pb = MultiProgress::new();
    let pull_request_pb = Arc::new(
        multi_pb.add(get_progress_bar(total_pages as u64, "Fetching pull requests"))
    );
    let events_pb = Arc::new(
        multi_pb.add(get_progress_bar(total_pages as u64, "Fetching events"))
    );
    let reviews_pb = Arc::new(
        multi_pb.add(get_progress_bar(total_pages as u64, "Fetching reviews"))
    );

    let mut results = Vec::new();
    let pulls: Arc<Mutex<Vec<PullRequest>>> = Arc::new(Mutex::new(Vec::new()));

    for page in 1..=total_pages {
        let octocrab_a = octocrab_rc.clone();
        let pool_a = pool.clone();
        let owner_a = owner.clone();
        let repo_a = repo.clone();
        let permit = Arc::clone(&sem).acquire_owned().await;
        let pulls = pulls.clone();
        let pb = pull_request_pb.clone();
        let events_pb = events_pb.clone();
        let reviews_pb = reviews_pb.clone();

        let result = tokio::spawn(async move {
            let _permit = permit;

            let mut page = match octocrab_a
                .pulls(&owner_a, &repo_a)
                .list()
                .state(State::Closed)
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
            pb.inc(1);

            for pull in page.take_items() {
                let pr_number = pull.number as u32;

                let pull_db = DbPullRequest::create(&pool_a, &pull, repo_db.id).await.map_err(|_| {
                    miette!("Error creating pull request db record")
                }).unwrap();

                match try_join!(
                    collect_events(&octocrab_a, &pool_a, &owner_a, &repo_a, &pull_db),
                    collect_reviews(&octocrab_a, &pool_a, &owner_a, &repo_a, &pull_db),
                ) {
                    Ok(_) => {
                        events_pb.inc(1);
                        reviews_pb.inc(1);
                    }
                    Err(err) => {
                        eprintln!("Error collecting data for pull request {}: {}", pr_number, err);
                    }
                }
            }

            pulls.lock().unwrap().extend(page.take_items());
        });
        results.push(result);
    }

    // Wait for all the tasks to finish
    for result in results {
        result.await.map_err(|_| miette!("Error collecting pull requests"))?;
    }

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
