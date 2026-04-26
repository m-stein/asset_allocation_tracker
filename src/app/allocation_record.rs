use serde::{Deserialize, Serialize};

use crate::app::asset_reference::AssetReference;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationRecord {
    pub date: String,
    pub positions: Vec<AllocationPosition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationPosition {
    pub asset: AllocationAsset,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationAsset {
    pub name: String,
    pub reference: AssetReference,
    pub categories: Vec<AllocationAssetCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationAssetCategory {
    pub name: String,
    pub values: Vec<AllocationCategoryValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationCategoryValue {
    pub name: String,
    pub ratio: f64,
}