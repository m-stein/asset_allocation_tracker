use crate::app::error::AppError;

use crate::domain::allocation_record::AllocationRecord;
use crate::domain::asset::Asset;
use crate::domain::category::Category;
use crate::domain::category_value::CategoryValue;
use crate::domain::category_assignment::CategoryAssignment;
use crate::domain::named_distribution::NamedDistribution;

pub trait AssetRepository {
    fn add_asset(&mut self, asset: &Asset, catgy_assignms: &Vec<CategoryAssignment>) -> Result<(), AppError>;
    fn add_category(&mut self, category: &Category) -> Result<(), AppError>;
    fn list_assets(&self) -> Result<Vec<Asset>, AppError>;
    fn add_allocation_record(
        &mut self,
        record: &AllocationRecord,
    ) -> Result<(), AppError>;
    fn get_latest_allocation_record(&self) -> Result<Option<AllocationRecord>, AppError>;
    fn list_asset_categories(&self) -> Result<Vec<Category>, AppError>;
    fn list_asset_category_values(&self, category: &Category) -> Result<Vec<CategoryValue>, AppError>;
    fn add_asset_category_value(
        &mut self,
        value: &CategoryValue,
    ) -> Result<(), AppError>;
    fn get_distribution_for_category(
        &self,
        category_id: i64,
    ) -> Result<Vec<NamedDistribution>, AppError>;
}