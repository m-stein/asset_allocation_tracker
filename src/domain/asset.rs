use crate::domain::asset_reference::AssetReference;

#[derive(Debug, Clone)]
pub struct Asset {
    pub id: i64,
    pub name: String,
    pub reference: AssetReference,
}