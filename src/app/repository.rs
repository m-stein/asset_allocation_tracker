use crate::app::allocation_record::AllocationRecord;
use crate::app::error::AppError;

use crate::app::allocation_record_input::AllocationRecordInput;
use crate::app::asset::Asset;
use crate::app::category::Category;
use crate::app::category_value::CategoryValue;
use crate::app::category_assignment::CategoryAssignment;

pub trait AssetRepository {
    fn add_asset(&mut self, asset: &Asset, catgy_assignms: &Vec<CategoryAssignment>) -> Result<(), AppError>;
    fn add_category(&mut self, category: &Category) -> Result<(), AppError>;
    fn list_assets(&self) -> Result<Vec<Asset>, AppError>;
    fn add_allocation_record(
        &mut self,
        record: &AllocationRecordInput,
    ) -> Result<(), AppError>;
    fn list_asset_categories(&self) -> Result<Vec<Category>, AppError>;
    fn list_asset_category_values(&self, category: &Category) -> Result<Vec<CategoryValue>, AppError>;
    fn add_asset_category_value(
        &mut self,
        value: &CategoryValue,
    ) -> Result<(), AppError>;
    fn get_latest_allocation_records(
        &self,
        limit: usize,
    ) -> Result<Vec<AllocationRecord>, AppError>;
    fn get_category_name_by_id(&self, category_id: i64) -> Result<String, AppError>;
}