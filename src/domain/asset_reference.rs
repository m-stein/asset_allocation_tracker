use crate::domain::asset_reference_type::AssetReferenceType;


#[derive(Debug, Clone)]
pub struct AssetReference {
    pub reference_type: AssetReferenceType,
    pub value: String,
}

impl AssetReference {
    pub fn new(reference_type: AssetReferenceType, value: String) -> Result<Self, String> {
        let trimmed = value.trim();

        if trimmed.is_empty() {
            return Err("Reference value must not be empty".into());
        }

        Ok(Self {
            reference_type,
            value: trimmed.to_string(),
        })
    }
}