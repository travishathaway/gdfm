/// Holds functions and methods used for database operations
// use rusqlite::{Connection, Error as RusqliteError};

use std::fs::create_dir_all;
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::Sqlite;
use sqlx::Pool;

use crate::constants::{DB_FILE, APP_NAME};

#[derive(Debug, sqlx::FromRow)]
pub struct Project {
    pub id: u32,
    pub name: String,
}

#[derive(Debug, sqlx::FromRow)]
pub struct Repository {
    pub id: u32,
    pub project_id: u32,
    pub name: String,
    pub owner: String,
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
        "CREATE TABLE IF NOT EXISTS projects (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        )",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS repositories (
            id INTEGER PRIMARY KEY,
            project_id INTEGER NOT NULL,
            name TEXT NOT NULL,
            owner TEXT NOT NULL
        )",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_repositories_project_id_name 
            ON repositories (project_id, name, owner)
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

impl Project {
    pub async fn from(pool: &Pool<Sqlite>, name: &str) -> Result<Self, sqlx::Error> {
        let project: Self = sqlx::query_as("SELECT id, name FROM projects WHERE name = $1")
            .bind(name)
            .fetch_one(pool)
            .await?;

        Ok(project)
    }

    pub async fn create(pool: &Pool<Sqlite>, name: &str) -> Result<Self, sqlx::Error> {
        let id = sqlx::query("INSERT INTO projects (name) VALUES ($1)")
            .bind(name)
            .execute(pool)
            .await?;

        let project: Self = sqlx::query_as("SELECT id, name FROM projects WHERE id = $1")
            .bind(id.last_insert_rowid())
            .fetch_one(pool)
            .await?;

        Ok(project)
    }

    pub async fn repositories(&self, pool: &Pool<Sqlite>) -> Result<Vec<Repository>, sqlx::Error> {
        let repos: Vec<Repository> = sqlx::query_as("SELECT id, project_id, name, owner FROM repositories WHERE project_id = $1")
            .bind(self.id)
            .fetch_all(pool)
            .await?;

        Ok(repos)
    }
}

impl Repository {
    pub async fn create(pool: &Pool<Sqlite>, project_id: u32, path: &str) -> Result<Self, sqlx::Error> {
        let name = path.split("/").last().expect("Repository name should exist");
        let owner = path.split("/").nth(0).expect("Repository owner should exist");

        let id = sqlx::query(
            "INSERT INTO repositories (project_id, name, owner) VALUES ($1, $2, $3)")
            .bind(project_id)
            .bind(name)
            .bind(owner)
            .execute(pool)
            .await?;

        let repository = sqlx::query_as("SELECT id, project_id, name, owner FROM repositories WHERE id = $1")
            .bind(id.last_insert_rowid())
            .fetch_one(pool)
            .await?;
        
        Ok(repository)
    }

    pub async fn create_many(pool: &Pool<Sqlite>, project_id: u32, repo_paths: Vec<&str>) -> Result<Vec<Repository>, sqlx::Error> {
        let mut repos: Vec<Repository> = vec![];

        for repo in repo_paths {
            repos.push(Self::create(pool, project_id, repo).await?);
        }

        Ok(repos)
    }
}