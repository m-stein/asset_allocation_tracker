#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceType {
    Iban,
    Isin,
    Ticker,
}