use jiff::{Zoned, civil::Date};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use strum_macros::{Display, EnumIter};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, EnumIter, Display)]
pub enum Currency {
    #[strum(serialize = "EUR")]
    Eur,
    #[strum(serialize = "USD")]
    Usd,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogBuyTransactionInput {
    pub currency: Currency,
    pub isin: String,
    pub quantity: String,
    pub share_price: String,
    pub order_value: String,
    pub date: Date,
    pub client_today: Date,
}

impl Default for LogBuyTransactionInput {
    fn default() -> Self {
        let today = Zoned::now().date();
        Self {
            currency: Currency::Eur,
            date: today,
            client_today: today,
            isin: String::new(),
            quantity: String::new(),
            share_price: String::new(),
            order_value: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSellTransactionInput {
    pub currency: Currency,
    pub isin: String,
    pub portfolio_item_id_to_quantity: HashMap<i64, String>,
    pub share_price: String,
    pub order_value: String,
    pub date: Date,
    pub client_today: Date,
}

impl Default for LogSellTransactionInput {
    fn default() -> Self {
        let today = Zoned::now().date();
        Self {
            currency: Currency::Eur,
            isin: String::new(),
            portfolio_item_id_to_quantity: HashMap::new(),
            share_price: String::new(),
            order_value: String::new(),
            date: today,
            client_today: today,
        }
    }
}
