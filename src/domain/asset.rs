#[derive(Debug, Clone)]
pub struct Asset {
    pub name: String,
    pub reference: AssetReference,
}

#[derive(Debug, Clone)]
pub struct AssetReference {
    pub reference_type: ReferenceType,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceType {
    Iban,
    Isin,
    Ticker,
}

impl AssetReference {
    pub fn new(reference_type: ReferenceType, value: String) -> Result<Self, String> {
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