use crate::app::error::AppError;
use crate::domain::allocation_record::AllocationRecord;
use crate::domain::asset::Asset;
use crate::domain::category::Category;

pub trait AssetRepository {
    fn add_asset(&mut self, asset: &Asset) -> Result<(), AppError>;
    fn add_category(&mut self, category: &Category) -> Result<(), AppError>;
    fn list_assets(&self) -> Result<Vec<Asset>, AppError>;
    fn add_allocation_record(
        &mut self,
        record: &AllocationRecord,
    ) -> Result<(), AppError>;
    fn get_latest_allocation_record(&self) -> Result<Option<AllocationRecord>, AppError>;
}