use jiff::civil::Date;

#[derive(Debug, Clone)]
pub struct AllocationRecord {
    pub date: Date,
    pub asset_ids: Vec<i64>,
}