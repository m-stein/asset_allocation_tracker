use std::fs;
use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::app::error::AppError;
use crate::app::repository::AssetRepository;
use crate::app::allocation_record::{AllocationPosition, AllocationRecord};
use crate::app::asset::Asset;
use crate::app::asset_reference::AssetReference;
use crate::app::category::Category;
use crate::app::category_value::CategoryValue;
use crate::app::category_assignment::CategoryAssignment;
use crate::app::named_distribution::NamedDistribution;
use crate::app::asset_reference_type::AssetReferenceType;

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
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL,
                    reference_type TEXT NOT NULL,
                    reference_value TEXT NOT NULL
                );
                "#,
                [],
            )
            .map_err(|err| AppError::Storage(format!("Failed to initialize schema: {err}")))?;

        self.connection
            .execute(
                r#"
                CREATE TABLE IF NOT EXISTS asset_categories (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    name TEXT NOT NULL
                );
                "#,
                [],
            )
            .map_err(|err| AppError::Storage(format!("Failed to initialize schema: {err}")))?;

        self.connection.execute(
            r#"
            CREATE TABLE IF NOT EXISTS asset_category_values (
                id                INTEGER PRIMARY KEY AUTOINCREMENT,
                asset_category_id INTEGER NOT NULL,
                name              TEXT NOT NULL,
                FOREIGN KEY (asset_category_id) REFERENCES asset_categories(id)
            )
            "#,
            [],
        ).map_err(|e| AppError::Storage(e.to_string()))?;

        self.connection.execute(
            r#"
            CREATE TABLE IF NOT EXISTS allocation_records (
                id   INTEGER PRIMARY KEY AUTOINCREMENT,
                date TEXT NOT NULL
            )
            "#,
            [],
        ).map_err(|err| AppError::Storage(format!("Failed to initialize schema: {err}")))?;

        self.connection.execute(
            r#"
            CREATE TABLE IF NOT EXISTS allocation_record_positions (
                allocation_record_id INTEGER NOT NULL,
                asset_id             INTEGER NOT NULL,
                amount               INTEGER NOT NULL,
                PRIMARY KEY (allocation_record_id, asset_id),
                FOREIGN KEY (allocation_record_id) REFERENCES allocation_records(id),
                FOREIGN KEY (asset_id) REFERENCES assets(id)
            )
            "#,
            [],
        ).map_err(|err| AppError::Storage(format!("Failed to initialize schema: {err}")))?;

        self.connection.execute(
            r#"
                CREATE TABLE IF NOT EXISTS asset_category_value_assignments (
                    asset_id INTEGER NOT NULL,
                    asset_category_value_id INTEGER NOT NULL,
                    ratio DECIMAL(5,4) CHECK (ratio >= 0 AND ratio <= 1) NOT NULL,
                    PRIMARY KEY (asset_id, asset_category_value_id),
                    FOREIGN KEY (asset_id) REFERENCES assets(id),
                    FOREIGN KEY (asset_category_value_id) REFERENCES asset_category_values(id)
                )
                "#,
            [],
        ).map_err(|err| AppError::Storage(format!("Failed to initialize schema: {err}")))?;

        Ok(())
    }
}

impl AssetRepository for SqliteAssetRepository {

    fn get_distribution_for_category(
        &self,
        category_id: i64,
    ) -> Result<Vec<NamedDistribution>, AppError> {

        let mut stmt = self.connection.prepare(
            r#"
            WITH latest_record AS (
                SELECT id
                FROM allocation_records
                ORDER BY date DESC
                LIMIT 1
            )
            SELECT
                acv.id,
                acv.name,
                SUM(arp.amount * acva.ratio) AS value_amount
            FROM allocation_record_positions arp
            JOIN latest_record lr ON arp.allocation_record_id = lr.id
            JOIN assets a ON a.id = arp.asset_id
            JOIN asset_category_value_assignments acva ON acva.asset_id = a.id
            JOIN asset_category_values acv ON acv.id = acva.asset_category_value_id
            WHERE acv.asset_category_id = ?1
            GROUP BY acv.id, acv.name
            ORDER BY value_amount DESC;
            "#
        )?;

        let rows = stmt.query_map(params![category_id], |row| {
            Ok(NamedDistribution {
                name: row.get(1)?,
                amount: row.get(2)?,
            })
        })?;

        let mut result = Vec::new();
        for row in rows {
            result.push(row?);
        }

        Ok(result)
    }

    fn list_asset_category_values(
        &self,
        category: &Category,
    ) -> Result<Vec<CategoryValue>, AppError> {
        let mut stmt = self.connection
            .prepare(
                "
                SELECT id, asset_category_id, name
                FROM asset_category_values
                WHERE asset_category_id = ?
                ORDER BY name
                ",
            )
            .map_err(|e| AppError::Storage(e.to_string()))?;

        let values_iter = stmt
            .query_map(params![category.id], |row| {
                Ok(CategoryValue {
                    id: row.get(0)?,
                    asset_category_id: row.get(1)?,
                    name: row.get(2)?,
                })
            })
            .map_err(|e| AppError::Storage(e.to_string()))?;

        let mut values = Vec::new();
        for value in values_iter {
            values.push(
                value.map_err(|e| AppError::Storage(e.to_string()))?
            );
        }

        Ok(values)
    }

    fn list_asset_categories(&self) -> Result<Vec<Category>, AppError> {
        let mut stmt = self.connection
            .prepare(
                "SELECT id, name
                FROM asset_categories
                ORDER BY name ASC"
            )
            .map_err(|e| AppError::Storage(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                Ok(Category {
                    id: row.get(0)?,
                    name: row.get(1)?,
                })
            })
            .map_err(|e| AppError::Storage(e.to_string()))?;

        let mut categories = Vec::new();
        for row in rows {
            categories.push(row.map_err(|e| AppError::Storage(e.to_string()))?);
        }

        Ok(categories)
    }

    fn add_asset_category_value(
        &mut self,
        value: &CategoryValue,
    ) -> Result<(), AppError> {
        self.connection.execute(
            "INSERT INTO asset_category_values (asset_category_id, name)
            VALUES (?1, ?2)",
            rusqlite::params![value.asset_category_id, value.name],
        ).map_err(|e| AppError::Storage(e.to_string()))?;

        Ok(())
    }

    fn add_asset(&mut self, asset: &Asset, catgy_assignms: &Vec<CategoryAssignment>) -> Result<(), AppError> {
        let tx = self.connection
            .transaction()
            .map_err(|e| AppError::Storage(e.to_string()))?;

        tx.execute(
            "INSERT INTO assets (name, reference_type, reference_value) VALUES (?1, ?2, ?3)",
            params![
                asset.name,
                reference_type_to_str(asset.reference.reference_type),
                asset.reference.value
            ],
        )
        .map_err(|e| AppError::Storage(e.to_string()))?;

        let asset_id = tx.last_insert_rowid();
        for assignm in catgy_assignms.iter() {
            tx.execute(
                "
                INSERT INTO asset_category_value_assignments
                (asset_id, asset_category_value_id, ratio)
                VALUES (?1, ?2, ?3)
                ",
                params![asset_id, assignm.value_id, assignm.ratio],
            )
            .map_err(|e| AppError::Storage(e.to_string()))?;
        }
        tx.commit().map_err(|e| AppError::Storage(e.to_string()))?;
        Ok(())
    }

    fn add_category(&mut self, category: &Category) -> Result<(), AppError> {
        self.connection
            .execute(
                "INSERT INTO asset_categories (name) VALUES (?1)",
                params![category.name],
            )
            .map_err(|e| AppError::Storage(e.to_string()))?;

        Ok(())
    }

    fn list_assets(&self) -> Result<Vec<Asset>, AppError> {
        let mut stmt = self.connection
            .prepare(
                "SELECT id, name, reference_type, reference_value
                 FROM assets
                 ORDER BY name ASC"
            )
            .map_err(|e| AppError::Storage(e.to_string()))?;

        let rows = stmt
            .query_map([], |row| {
                let reference_type_str: String = row.get(2)?;
                let reference_type = str_to_reference_type(&reference_type_str)
                    .ok_or_else(|| {
                        rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Text,
                            Box::new(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "invalid reference type",
                            )),
                        )
                    })?;

                Ok(Asset {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    reference: AssetReference {
                        reference_type,
                        value: row.get(3)?,
                    },
                })
            })
            .map_err(|e| AppError::Storage(e.to_string()))?;

        let mut assets = Vec::new();
        for row in rows {
            assets.push(row.map_err(|e| AppError::Storage(e.to_string()))?);
        }

        Ok(assets)
    }

    fn add_allocation_record(
        &mut self,
        record: &AllocationRecord,
    ) -> Result<(), AppError> {
        let tx = self.connection
            .transaction()
            .map_err(|e| AppError::Storage(e.to_string()))?;

        let date_str = record.date.to_string();

        tx.execute(
            "INSERT INTO allocation_records (date) VALUES (?1)",
            params![date_str],
        )
        .map_err(|e| AppError::Storage(e.to_string()))?;

        let allocation_record_id = tx.last_insert_rowid();

        for position in &record.positions {
            tx.execute(
                "INSERT INTO allocation_record_positions (allocation_record_id, asset_id, amount)
                VALUES (?1, ?2, ?3)",
                params![allocation_record_id, position.asset_id, position.amount],
            )
            .map_err(|e| AppError::Storage(e.to_string()))?;
        }

        tx.commit()
            .map_err(|e| AppError::Storage(e.to_string()))?;

        Ok(())
    }

    fn get_latest_allocation_record(&self) -> Result<Option<AllocationRecord>, AppError> {
        let latest_row: Option<(i64, String)> = self.connection
            .query_row(
                "SELECT id, date
                FROM allocation_records
                ORDER BY date DESC, id DESC
                LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| AppError::Storage(e.to_string()))?;

        let Some((record_id, date_str)) = latest_row else {
            return Ok(None);
        };

        let date = jiff::civil::Date::strptime("%Y-%m-%d", &date_str)
            .map_err(|e| AppError::Storage(e.to_string()))?;

        let mut stmt = self.connection.prepare(
            "SELECT asset_id, amount
            FROM allocation_record_positions
            WHERE allocation_record_id = ?1
            ORDER BY amount DESC, asset_id ASC"
        ).map_err(|e| AppError::Storage(e.to_string()))?;

        let rows = stmt.query_map([record_id], |row| {
            Ok(AllocationPosition {
                asset_id: row.get(0)?,
                amount: row.get(1)?,
            })
        }).map_err(|e| AppError::Storage(e.to_string()))?;

        let mut positions = Vec::new();
        for row in rows {
            positions.push(row.map_err(|e| AppError::Storage(e.to_string()))?);
        }

        Ok(Some(AllocationRecord { date, positions }))
    }
}

fn reference_type_to_str(rt: AssetReferenceType) -> &'static str {
    match rt {
        AssetReferenceType::Iban => "IBAN",
        AssetReferenceType::Isin => "ISIN",
        AssetReferenceType::Ticker => "TICKER",
    }
}

fn str_to_reference_type(s: &str) -> Option<AssetReferenceType> {
    match s {
        "IBAN" => Some(AssetReferenceType::Iban),
        "ISIN" => Some(AssetReferenceType::Isin),
        "TICKER" => Some(AssetReferenceType::Ticker),
        _ => None,
    }
}