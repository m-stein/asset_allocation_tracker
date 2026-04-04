#[derive(Debug, Clone)]
pub struct NewAsset {
    pub name: String,
}

impl NewAsset {
    pub fn new(name: String) -> Self {
        Self { name }
    }
}