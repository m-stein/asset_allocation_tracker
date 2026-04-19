use std::collections::HashMap;

use jiff::civil::Date;

use crate::app::allocation_record::{AllocationPosition, AllocationRecord};
use crate::app::asset::Asset;
use crate::app::asset_input::AssetInput;
use crate::app::error::AppError;
use crate::app::repository::AssetRepository;
use crate::app::asset_reference::AssetReference;
use crate::app::category::Category;
use crate::app::category_value::CategoryValue;
use crate::app::category_assignment::CategoryAssignment;
use crate::app::category_assignment_input::CategoryAssignmentInput;
use crate::app::named_distribution::NamedDistribution;

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

    pub fn get_distribution_for_category(
        &self,
        category_id: i64,
    ) -> Result<Vec<NamedDistribution>, AppError> {
        self.repository.get_distribution_for_category(category_id)
    }

    pub fn add_asset(
        &mut self,
        asset_input: &AssetInput,
        catgy_id_to_assignm_inputs: &HashMap<i64, Vec<CategoryAssignmentInput>>,
    ) -> Result<(), AppError> {
        let name = asset_input.name.trim();
        if name.is_empty() {
            return Err(AppError::Validation(
                "Asset name must not be empty".into(),
            ));
        }
        let reference = AssetReference::new(
            asset_input.reference_type, asset_input.reference_value.clone()
        ).map_err(AppError::Validation)?;

        let asset = Asset {
            id: 0,
            name: name.to_string(),
            reference,
        };
        let mut catgy_assignms: Vec<CategoryAssignment> = Vec::new();
        for (_, assignm_inputs) in catgy_id_to_assignm_inputs.iter() {
            for assignm_input in assignm_inputs {
                if let Some(id) = assignm_input.value_id {
                    catgy_assignms.push(CategoryAssignment { value_id: id, ratio: assignm_input.percentage / 100. })
                } else {
                    return Err(AppError::Validation("Unset catgory value".into()));
                };
            }
        }
        self.repository.add_asset(&asset, &catgy_assignms)
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
    
    pub fn list_asset_category_values(&self, category: &Category) -> Result<Vec<CategoryValue>, AppError> {
        self.repository.list_asset_category_values(category)
    }

    pub fn add_asset_category_value(
        &mut self,
        asset_category_id: i64,
        name: String,
    ) -> Result<(), AppError> {
        let value = CategoryValue::new(asset_category_id, name)
            .map_err(AppError::Validation)?;

        self.repository.add_asset_category_value(&value)
    }
}