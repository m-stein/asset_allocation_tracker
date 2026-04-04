use crate::app::error::AppError;
use crate::app::repository::AssetRepository;
use crate::domain::asset::NewAsset;

pub struct AssetService {
    repository: Box<dyn AssetRepository>,
}

impl AssetService {
    pub fn new(repository: Box<dyn AssetRepository>) -> Self {
        Self { repository }
    }

    pub fn add_asset(&mut self, name: String) -> Result<(), AppError> {
        let trimmed_name = name.trim();

        if trimmed_name.is_empty() {
            return Err(AppError::Validation(
                "Asset name must not be empty.".to_string(),
            ));
        }

        let asset = NewAsset::new(trimmed_name.to_string());
        self.repository.add_asset(&asset)
    }
}