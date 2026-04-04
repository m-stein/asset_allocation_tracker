use crate::app::error::AppError;
use crate::domain::asset::NewAsset;

pub trait AssetRepository {
    fn add_asset(&mut self, asset: &NewAsset) -> Result<(), AppError>;
}