use crate::domain::asset::{Asset, AssetReference, ReferenceType};
use crate::app::error::AppError;
use crate::app::repository::AssetRepository;

pub struct AssetService {
    repository: Box<dyn AssetRepository>,
}

impl AssetService {
    pub fn new(repository: Box<dyn AssetRepository>) -> Self {
        Self { repository }
    }

    pub fn add_asset(
        &mut self,
        name: String,
        reference_type: ReferenceType,
        reference_value: String,
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
            name: name.to_string(),
            reference,
        };

        self.repository.add_asset(&asset)
    }
}

pub fn reference_type_to_str(rt: ReferenceType) -> &'static str {
    match rt {
        ReferenceType::Iban => "IBAN",
        ReferenceType::Isin => "ISIN",
        ReferenceType::Ticker => "TICKER",
    }
}