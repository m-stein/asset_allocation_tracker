use jiff::civil::Date;

#[derive(Debug, Clone)]
pub struct AllocationRecordInput {
    pub date: Date,
    pub positions: Vec<AllocationPositionInput>,
}

#[derive(Debug, Clone)]
pub struct AllocationPositionInput {
    pub asset_id: i64,
    pub amount: i64,
}

impl AllocationRecordInput {
    pub fn new(date: Date, positions: Vec<AllocationPositionInput>) -> Result<Self, String> {
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