use std::collections::HashMap;

use jiff::civil::Date;

use crate::app::allocation_record_input::{AllocationPositionInput, AllocationRecordInput};
use crate::app::allocation_record::AllocationRecord;
use crate::app::asset::Asset;
use crate::app::asset_input::AssetInput;
use crate::app::error::AppError;
use crate::app::repository::AssetRepository;
use crate::app::asset_reference::AssetReference;
use crate::app::category::Category;
use crate::app::category_value::CategoryValue;
use crate::app::category_assignment::CategoryAssignment;
use crate::app::category_assignment_input::CategoryAssignmentInput;
use crate::app::named_distribution::{DatedDistribution, NamedDistribution};

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

    pub fn calc_distribution_for_category(
        &self,
        records: Vec<AllocationRecord>,
        category_name: &str,
    ) -> Vec<DatedDistribution> {
        records
            .into_iter()
            .map(|record| {
                let mut amounts: HashMap<String, f64> = HashMap::new();

                for position in record.positions {
                    let Some(category) = position
                        .asset
                        .categories
                        .iter()
                        .find(|category| category.name == category_name)
                    else {
                        continue;
                    };

                    for value in &category.values {
                        *amounts.entry(value.name.clone()).or_insert(0.0) +=
                            position.amount as f64 * value.ratio;
                    }
                }

                let mut values: Vec<NamedDistribution> = amounts
                    .into_iter()
                    .map(|(name, amount)| NamedDistribution {
                        name,
                        amount: amount,
                    })
                    .collect();

                values.sort_by(|a, b| {
                    b.amount
                        .partial_cmp(&a.amount)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.name.cmp(&b.name))
                });

                DatedDistribution {
                    date: record.date,
                    values,
                }
            })
            .collect()
    }

    pub fn get_distribution_for_category(
        &self,
        category_id: i64,
        days: i64,
    ) -> Result<Vec<DatedDistribution>, AppError> {

        let records = self.repository.get_latest_allocation_records(days as usize)?;
        let category_name = self.repository.get_category_name_by_id(category_id)?;
        Ok(self.calc_distribution_for_category(records, &category_name))
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
        positions: Vec<AllocationPositionInput>,
    ) -> Result<(), AppError> {
        let record = AllocationRecordInput::new(date, positions)
            .map_err(AppError::Validation)?;

        self.repository.add_allocation_record(&record)
    }

    pub fn get_latest_allocation_record(
        &self,
    ) -> Result<Option<AllocationRecord>, AppError> {
        let mut records = self.repository.get_latest_allocation_records(1)?;

        Ok(records.pop())
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