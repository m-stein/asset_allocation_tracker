#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetReferenceType {
    Iban,
    Isin,
    Ticker,
}