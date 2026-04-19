use strum_macros::EnumIter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum AssetReferenceType {
    Iban,
    Isin,
    Ticker,
}