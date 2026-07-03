use crate::TransactionType;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListedTransaction {
    pub date: String,
    pub r#type: TransactionType,
    pub asset_name: Option<String>,
    pub isin: String,
    pub quantity: String,
    pub share_price: String,
    pub order_value: String,
    pub currency: String,
}
