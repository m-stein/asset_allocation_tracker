#[derive(Debug, Clone)]
pub struct AssetCategoryValue {
    pub id: i64,
    pub asset_category_id: i64,
    pub name: String,
}

impl AssetCategoryValue {
    pub fn new(asset_category_id: i64, name: String) -> Result<Self, String> {
        let trimmed = name.trim();

        if asset_category_id <= 0 {
            return Err("Category id must be greater than 0".into());
        }

        if trimmed.is_empty() {
            return Err("Category value name must not be empty".into());
        }

        Ok(Self {
            id: 0,
            asset_category_id,
            name: trimmed.to_string(),
        })
    }
}

pub struct CategoryDistribution {
    pub value_name: String,
    pub percentage: f64,
}