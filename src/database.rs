/// Holds functions and methods used for database operations
// use rusqlite::{Connection, Error as RusqliteError};

use std::fs::create_dir_all;
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

    Ok(pool)
}

pub async fn destroy_db() -> Result<(), sqlx::Error> {
    let db_uri = get_db_uri();
    Sqlite::drop_database(&db_uri).await?;

    Ok(())
}

impl Maintainer {
    pub async fn from(pool: &Pool<Sqlite>, username: &str) -> Result<Self, sqlx::Error> {
        let maintainer: Self = sqlx::query_as("SELECT id, username FROM maintainers WHERE username = $1")
            .bind(username)
            .fetch_one(pool)
            .await?;

        Ok(maintainer)
    }

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

    pub async fn repositories(&self, pool: &Pool<Sqlite>) -> Result<Vec<Repository>, sqlx::Error> {
        let repos: Vec<Repository> = sqlx::query_as(
            "SELECT id, name, owner FROM repositories r LEFT JOIN repository_maintainers rm ON r.id = rm.repo_id WHERE rm.maintainer_id = $1"
        )
            .bind(self.id)
            .fetch_all(pool)
            .await?;

        Ok(repos)
    }
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

    pub async fn maintainers(&self, pool: &Pool<Sqlite>) -> Result<Vec<Maintainer>, sqlx::Error> {
        let maintainers: Vec<Maintainer> = sqlx::query_as(
            "SELECT id, username FROM maintainers m LEFT JOIN repository_maintainers rm ON m.id = rm.maintainer_id WHERE rm.repo_id = $1"
        )
            .bind(self.id)
            .fetch_all(pool)
            .await?;

        Ok(maintainers)
    }

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