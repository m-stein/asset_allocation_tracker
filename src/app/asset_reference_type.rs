use serde::{Serialize, Deserialize};
use strum_macros::{EnumIter, EnumString, Display};

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, Serialize, Deserialize, EnumString, Display)]
pub enum AssetReferenceType {
    Iban,
    Isin,
    Ticker,
}