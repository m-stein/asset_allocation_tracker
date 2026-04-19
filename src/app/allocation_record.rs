use jiff::civil::Date;

#[derive(Debug, Clone)]
pub struct AllocationRecord {
    pub date: Date,
    pub positions: Vec<AllocationPosition>,
}

#[derive(Debug, Clone)]
pub struct AllocationPosition {
    pub asset_id: i64,
    pub amount: i64,
}

impl AllocationRecord {
    pub fn new(date: Date, positions: Vec<AllocationPosition>) -> Result<Self, String> {
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