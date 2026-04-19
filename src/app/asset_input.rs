use crate::app::asset_reference_type::AssetReferenceType;

pub struct AssetInput {
    pub name: String,
    pub reference_value: String,
    pub reference_type: AssetReferenceType,
}

impl Default for AssetInput {
    fn default() -> Self {
        Self {
            name: String::new(),
            reference_value: String::new(),
            reference_type: AssetReferenceType::Isin,
        }
    }
}