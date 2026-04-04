use crate::app::error::AppError;
use crate::domain::asset::Asset;

pub trait AssetRepository {
    fn add_asset(&mut self, asset: &Asset) -> Result<(), AppError>;
}