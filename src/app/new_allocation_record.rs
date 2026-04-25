use jiff::civil::Date;

#[derive(Debug, Clone)]
pub struct NewAllocationRecord {
    pub date: Date,
    pub positions: Vec<NewAllocationPosition>,
}

#[derive(Debug, Clone)]
pub struct NewAllocationPosition {
    pub asset_id: i64,
    pub amount: i64,
}

impl NewAllocationRecord {
    pub fn new(date: Date, positions: Vec<NewAllocationPosition>) -> Result<Self, String> {
        if positions.is_empty() {
            return Err("At least one position must be added".into());
        }
        for position in &positions {
            if position.amount <= 0 {
                return Err("Position amount must be positive".into());
            }
        }
        Ok(Self { date, positions })
    }
}