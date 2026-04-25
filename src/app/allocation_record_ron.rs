use serde::{Deserialize, Serialize};

use crate::app::asset_reference_type::AssetReferenceType;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationRecordRon {
    pub date: String,
    pub positions: Vec<AllocationPositionRon>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationPositionRon {
    pub asset: AssetRon,
    pub amount: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetRon {
    pub name: String,
    pub reference: AssetReferenceRon,
    pub categories: Vec<AssetCategoryRon>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetReferenceRon {
    pub r#type: AssetReferenceType,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetCategoryRon {
    pub name: String,
    pub values: Vec<AssetCategoryValueRon>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetCategoryValueRon {
    pub name: String,
    pub ratio: f64,
}