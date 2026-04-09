use crate::domain::allocation_record::{AllocationPosition, AllocationRecord};
use crate::domain::asset::{Asset, AssetReference, ReferenceType};
use crate::app::error::AppError;
use crate::app::repository::AssetRepository;
use crate::domain::category::Category;
use crate::domain::category_value::AssetCategoryValue;
use jiff::civil::Date;

pub struct AssetService {
    repository: Box<dyn AssetRepository>,
}

impl AssetService {
    pub fn new(repository: Box<dyn AssetRepository>) -> Self {
        Self { repository }
    }

    pub fn add_category(
        &mut self,
        name: String
    ) -> Result<(), AppError> {
        let name = name.trim();

        if name.is_empty() {
            return Err(AppError::Validation(
                "Category name must not be empty".into(),
            ));
        }

        let category = Category {
            id: 0,
            name: name.to_string(),
        };

        self.repository.add_category(&category)
    }

    pub fn add_asset(
        &mut self,
        name: String,
        reference_type: ReferenceType,
        reference_value: String,
        category_value_ids: &Vec<i64>,
    ) -> Result<(), AppError> {
        let name = name.trim();

        if name.is_empty() {
            return Err(AppError::Validation(
                "Asset name must not be empty".into(),
            ));
        }

        let reference = AssetReference::new(reference_type, reference_value)
            .map_err(AppError::Validation)?;

        let asset = Asset {
            id: 0,
            name: name.to_string(),
            reference,
        };

        self.repository.add_asset(&asset, category_value_ids)
    }
    
    pub fn list_assets(&self) -> Result<Vec<Asset>, AppError> {
        self.repository.list_assets()
    }

    pub fn add_allocation_record(
        &mut self,
        date: Date,
        positions: Vec<AllocationPosition>,
    ) -> Result<(), AppError> {
        let record = AllocationRecord::new(date, positions)
            .map_err(AppError::Validation)?;

        self.repository.add_allocation_record(&record)
    }

    pub fn get_latest_allocation_record(
        &self,
    ) -> Result<Option<AllocationRecord>, AppError> {
        self.repository.get_latest_allocation_record()
    }

    pub fn list_asset_categories(&self) -> Result<Vec<Category>, AppError> {
        self.repository.list_asset_categories()
    }
    
    pub fn list_asset_category_values(&self, category: &Category) -> Result<Vec<AssetCategoryValue>, AppError> {
        self.repository.list_asset_category_values(category)
    }

    pub fn add_asset_category_value(
        &mut self,
        asset_category_id: i64,
        name: String,
    ) -> Result<(), AppError> {
        let value = AssetCategoryValue::new(asset_category_id, name)
            .map_err(AppError::Validation)?;

        self.repository.add_asset_category_value(&value)
    }
}