pub mod add_asset_args;
pub mod allocation_diagram_data;
pub mod allocation_record_input;
pub mod category;
pub mod category_value;
pub mod configure_categories_input;
pub mod listed_transaction;
pub mod log_transaction_input;
pub mod portfolio_item;
pub mod request_list;
pub mod transaction_asset;

use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter, EnumString};

pub use crate::add_asset_args::{AddAssetArgs, CategoryAssignmentPc};
pub use crate::allocation_diagram_data::AllocationDiagramData;
pub use crate::allocation_record_input::AllocationPositionInput;
pub use crate::category::Category;
pub use crate::configure_categories_input::{
    AdaptCategoryInput, CategoryValueInput, ConfigureCatgoriesInput, NewCategoryInput,
};
pub use crate::listed_transaction::ListedTransaction;
pub use crate::log_transaction_input::{Currency, LogBuyTransactionInput, LogSellTransactionInput};
pub use crate::portfolio_item::{PortfolioIsinItem, PortfolioOverviewItem};
pub use crate::transaction_asset::{ImportTransactionAssetsInput, TransactionAsset};

pub const APP_NAME: &str = "Tallytail";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumString, Display)]
pub enum TransactionType {
    #[strum(serialize = "BUY")]
    Buy,
    #[strum(serialize = "SELL")]
    Sell,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, EnumIter, Serialize, Deserialize, EnumString, Display,
)]
pub enum AssetReferenceType {
    #[strum(serialize = "IBAN")]
    Iban,
    #[strum(serialize = "ISIN")]
    Isin,
    Ticker,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssetReference {
    pub r#type: AssetReferenceType,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationCategoryValue {
    pub name: String,
    pub ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationAssetCategory {
    pub name: String,
    pub values: Vec<AllocationCategoryValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationAsset {
    pub name: String,
    pub reference: AssetReference,
    pub categories: Vec<AllocationAssetCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationPosition {
    pub asset: AllocationAsset,
    pub amount: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllocationRecord {
    pub date: String,
    pub positions: Vec<AllocationPosition>,
}

#[derive(Deserialize, Serialize)]
pub struct GetAllocDiagramDataArgs {
    pub category_id: i64,
    pub days: i64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CategoryAssignment {
    pub value_id: i64,
    pub ratio: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub reference: AssetReference,
}
