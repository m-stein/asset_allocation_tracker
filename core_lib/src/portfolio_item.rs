use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioIsinItem {
    pub portfolio_item_id: i64,
    pub buy_date: String,
    pub buy_time: String,
    pub quantity: String,
    pub share_price: String,
    pub order_value: String,
    pub currency: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioOverviewItem {
    pub asset_name: Option<String>,
    pub isin: String,
    pub quantity: String,
    pub average_share_price: String,
    pub total_value: String,
    pub currency: String,
}
