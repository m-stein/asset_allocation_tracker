use std::fs;
use std::path::Path;

use rusqlite::{params, Connection};

use crate::app::error::AppError;
use crate::app::repository::AssetRepository;
use crate::domain::asset::NewAsset;

pub struct SqliteAssetRepository {
    connection: Connection,
}

impl SqliteAssetRepository {
    pub fn new(db_path: &str) -> Result<Self, AppError> {
        if let Some(parent) = Path::new(db_path).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|err| {
                    AppError::Storage(format!("Failed to create database directory: {err}"))
                })?;
            }
        }

        let connection = Connection::open(db_path)
            .map_err(|err| AppError::Storage(format!("Failed to open database: {err}")))?;

        let repository = Self { connection };
        repository.init_schema()?;
        Ok(repository)
    }

    fn init_schema(&self) -> Result<(), AppError> {
        self.connection
            .execute(
                r#"
                CREATE TABLE IF NOT EXISTS assets (
                    id         INTEGER PRIMARY KEY AUTOINCREMENT,
                    name       TEXT NOT NULL
                )
                "#,
                [],
            )
            .map_err(|err| AppError::Storage(format!("Failed to initialize schema: {err}")))?;

        Ok(())
    }
}

impl AssetRepository for SqliteAssetRepository {
    fn add_asset(&mut self, asset: &NewAsset) -> Result<(), AppError> {
        self.connection
            .execute(
                "INSERT INTO assets (name) VALUES (?1)",
                params![asset.name],
            )
            .map_err(|err| AppError::Storage(format!("Failed to insert asset: {err}")))?;

        Ok(())
    }
}