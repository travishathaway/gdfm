/// Holds functions and methods used for database operations
// use rusqlite::{Connection, Error as RusqliteError};

use std::fs::create_dir_all;
use octocrab::models::pulls::PullRequest as OctoPullRequest;
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::Sqlite;
use sqlx::Pool;

use crate::constants::{DB_FILE, APP_NAME};

#[derive(Debug, sqlx::FromRow)]
pub struct Repository {
    pub id: u32,
    pub owner: String,
    pub name: String,
}

#[derive(Debug, sqlx::FromRow)]
pub struct Maintainer {
    pub id: u32,
    pub username: String
}

#[derive(Debug, sqlx::FromRow)]
pub struct PullRequest {
    pub id: u32,
    pub repo_id: u32,
    pub number: u32,
    pub title: String,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
    pub closed_at: Option<String>,
    pub merged_at: Option<String>,
    pub author: String,
    pub author_association: String
}

#[derive(Debug, sqlx::FromRow)]
pub struct IssuePullEvent {
    pub id: u32,
    pub issue_pull_id: u32,
    pub event_type: String,
    pub actor: String,
    pub created_at: String
}

#[derive(Debug, sqlx::FromRow)]
pub struct IssuePullReview {
    pub id: u32,
    pub issue_pull_id: u32,
    pub reviewer: String,
    pub state: String,
    pub author_association: String,
    pub submitted_at: String
}

/// Function used to get the database URI while creating its directory if it doesn't exist
/// 
/// TODO: maybe there's better error handling we could add for this?
fn get_db_uri() -> String {
    let data_dir = dirs::data_dir().expect("Data directory should exist").join(APP_NAME);
    create_dir_all(&data_dir).expect("Data directory should be created");
    let db_file = data_dir.join(DB_FILE);
    let db_uri = format!("sqlite://{}", db_file.to_str().expect("Path should be a string"));

    db_uri
}

pub async fn setup_db() -> Result<sqlx::SqlitePool, sqlx::Error> {
    let db_uri = get_db_uri();

    Sqlite::create_database(&db_uri).await?;
    let pool = sqlx::sqlite::SqlitePool::connect(&db_uri).await?;
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS maintainers (
            id INTEGER PRIMARY KEY,
            username TEXT NOT NULL UNIQUE
        )",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS repositories (
            id INTEGER PRIMARY KEY,
            owner TEXT NOT NULL,
            name TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_repositories_owner_name 
            ON repositories (owner, name)
        ",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS repository_maintainers (
            id INTEGER PRIMARY KEY,
            repo_id INTEGER NOT NULL,
            maintainer_id INTEGER NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_repository_maintainers_repo_id_maintainer_id 
            ON repository_maintainers (repo_id, maintainer_id)
        ",
    )
    .execute(&pool)
    .await?;

    // We store issues and pull requests in the same table
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS pulls(
            id INTEGER PRIMARY KEY,
            repo_id INTEGER NOT NULL,
            number INTEGER NOT NULL,
            title TEXT NOT NULL,
            state TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            closed_at TEXT,
            merged_at TEXT,
            author TEXT NOT NULL,
            author_association TEXT NOT NULL,
            FOREIGN KEY (repo_id) REFERENCES repositories (id) ON DELETE CASCADE
        )",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_pulls_repository_id_number
            ON pulls (repo_id, number)
        ",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
    "CREATE TABLE IF NOT EXISTS issue_pull_events (
        id INTEGER PRIMARY KEY,
        issue_pull_id INTEGER NOT NULL,
        event_type TEXT NOT NULL,
        actor TEXT NOT NULL,
        created_at TEXT NOT NULL,
        FOREIGN KEY (issue_pull_id) REFERENCES pulls (id) ON DELETE CASCADE
        )",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_issue_pull_events_issue_pull_id 
            ON issue_pull_events (issue_pull_id)
        ",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
    "CREATE TABLE IF NOT EXISTS issue_pull_reviews (
        id INTEGER PRIMARY KEY,
        issue_pull_id INTEGER NOT NULL,
        reviewer TEXT NOT NULL,
        state TEXT NOT NULL,
        author_association TEXT NOT NULL,
        submitted_at TEXT NOT NULL,
        FOREIGN KEY (issue_pull_id) REFERENCES pulls (id) ON DELETE CASCADE
        )",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE INDEX IF NOT EXISTS idx_issue_pull_reviews_issue_pull_id 
            ON issue_pull_reviews (issue_pull_id)
        ",
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

pub async fn destroy_db() -> Result<(), sqlx::Error> {
    let db_uri = get_db_uri();
    Sqlite::drop_database(&db_uri).await?;

    Ok(())
}

impl Maintainer {
    // pub async fn from(pool: &Pool<Sqlite>, username: &str) -> Result<Self, sqlx::Error> {
    //     let maintainer: Self = sqlx::query_as("SELECT id, username FROM maintainers WHERE username = $1")
    //         .bind(username)
    //         .fetch_one(pool)
    //         .await?;

    //     Ok(maintainer)
    // }

    pub async fn create(pool: &Pool<Sqlite>, username: &str) -> Result<Self, sqlx::Error> {
        let id = sqlx::query("INSERT INTO maintainers (username) VALUES ($1)")
            .bind(username)
            .execute(pool)
            .await?;

        let maintainer: Self = sqlx::query_as("SELECT id, username FROM maintainers WHERE id = $1")
            .bind(id.last_insert_rowid())
            .fetch_one(pool)
            .await?;

        Ok(maintainer)
    }

    pub async fn create_many(pool: &Pool<Sqlite>, usernames: Vec<&str>) -> Result<Vec<Self>, sqlx::Error> {
        let mut maintainers = Vec::new();

        for username in usernames {
            let maintainer = Self::create(pool, username).await?;
            maintainers.push(maintainer);
        }

        Ok(maintainers)
    }

    // pub async fn repositories(&self, pool: &Pool<Sqlite>) -> Result<Vec<Repository>, sqlx::Error> {
    //     let repos: Vec<Repository> = sqlx::query_as(
    //         "SELECT id, name, owner FROM repositories r LEFT JOIN repository_maintainers rm ON r.id = rm.repo_id WHERE rm.maintainer_id = $1"
    //     )
    //         .bind(self.id)
    //         .fetch_all(pool)
    //         .await?;

    //     Ok(repos)
    // }
}

impl Repository {
    pub async fn from(pool: &Pool<Sqlite>, path: &str) -> Result<Self, sqlx::Error> {
        let owner = path.split("/").nth(0).expect("Repository owner should exist");
        let name = path.split("/").last().expect("Repository name should exist");

        let repository: Self = sqlx::query_as("SELECT id, owner, name FROM repositories WHERE owner = $1 AND name = $2")
            .bind(owner)
            .bind(name)
            .fetch_one(pool)
            .await?;

        Ok(repository)
    }

    pub async fn create(pool: &Pool<Sqlite>, path: &str) -> Result<Self, sqlx::Error> {
        let owner = path.split("/").nth(0).expect("Repository owner should exist");
        let name = path.split("/").last().expect("Repository name should exist");

        let id = sqlx::query(
            "INSERT INTO repositories (owner, name) VALUES ($1, $2)")
            .bind(owner)
            .bind(name)
            .execute(pool)
            .await?;

        let repository = sqlx::query_as("SELECT id, owner, name FROM repositories WHERE id = $1")
            .bind(id.last_insert_rowid())
            .fetch_one(pool)
            .await?;
        
        Ok(repository)
    }

    // pub async fn maintainers(&self, pool: &Pool<Sqlite>) -> Result<Vec<Maintainer>, sqlx::Error> {
    //     let maintainers: Vec<Maintainer> = sqlx::query_as(
    //         "SELECT id, username FROM maintainers m LEFT JOIN repository_maintainers rm ON m.id = rm.maintainer_id WHERE rm.repo_id = $1"
    //     )
    //         .bind(self.id)
    //         .fetch_all(pool)
    //         .await?;

    //     Ok(maintainers)
    // }

    pub async fn add_maintainers(&self, pool: &Pool<Sqlite>, maintainers: &Vec<Maintainer>) -> Result<(), sqlx::Error> {
        for maintainer in maintainers {
            sqlx::query(
                "INSERT INTO repository_maintainers (repo_id, maintainer_id) VALUES ($1, $2)"
            )
            .bind(self.id)
            .bind(maintainer.id)
            .execute(pool)
            .await?;
        }

        Ok(())
    }
}

impl PullRequest {
    pub async fn create(
        pool: &Pool<Sqlite>,
        pull: &OctoPullRequest,
        repo_id: u32,
    ) -> Result<Self, sqlx::Error> {
        let updated_at = match pull.updated_at {
            Some(updated_at) => updated_at.to_string(),
            None => "".to_string(),
        };
        let closed_at = match pull.closed_at {
            Some(closed_at) => closed_at.to_string(),
            None => "".to_string(),
        };
        let merged_at = match pull.merged_at {
            Some(merged_at) => merged_at.to_string(),
            None => "".to_string(),
        };
        let author_login = match &pull.user {
            Some(user) => user.login.to_string(),
            None => "".to_string(),
        };

        let id = sqlx::query(
            "INSERT INTO pulls (
                id, repo_id, number, title, state, created_at, updated_at, closed_at, merged_at, author, author_association
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)",
        )
        .bind(pull.id.to_string())
        .bind(repo_id)
        .bind(pull.number.to_string())
        .bind(pull.title.clone().unwrap_or("".to_string()))
        .bind(format!("{:?}", pull.state))
        .bind(pull.created_at.unwrap().to_string()) // "created_at" should always be set
        .bind(updated_at)
        .bind(closed_at)
        .bind(merged_at)
        .bind(author_login)
        .bind(format!("{:?}", pull.author_association))
        .execute(pool)
        .await?;

        let issue_pull: Self = sqlx::query_as(
            "SELECT id, repo_id, number, title, state, created_at, updated_at, closed_at, merged_at, author, author_association 
            FROM pulls WHERE id = $1",
        )
        .bind(id.last_insert_rowid())
        .fetch_one(pool)
        .await?;

        Ok(issue_pull)
    }
}

impl IssuePullReview {
    pub async fn create(
        pool: &Pool<Sqlite>,
        issue_pull_id: u32,
        reviewer: &str,
        state: &str,
        author_association: &str,
        submitted_at: &str,
    ) -> Result<Self, sqlx::Error> {
        let id = sqlx::query(
            "INSERT INTO issue_pull_reviews (
                issue_pull_id, reviewer, state, author_association, submitted_at
            ) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(issue_pull_id)
        .bind(reviewer)
        .bind(state)
        .bind(author_association)
        .bind(submitted_at)
        .execute(pool)
        .await?;

        let issue_pull_review: Self = sqlx::query_as(
            "SELECT id, issue_pull_id, reviewer, state, author_association, submitted_at 
            FROM issue_pull_reviews WHERE id = $1",
        )
        .bind(id.last_insert_rowid())
        .fetch_one(pool)
        .await?;

        Ok(issue_pull_review)
    }
}