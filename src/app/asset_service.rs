use crate::domain::allocation_record::{AllocationPosition, AllocationRecord};
use crate::domain::asset::{Asset, AssetReference, ReferenceType};
use crate::app::error::AppError;
use crate::app::repository::AssetRepository;
use jiff::civil::Date;

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
            id: 0,
            name: name.to_string(),
            reference,
        };

        self.repository.add_asset(&asset)
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
}