pub struct NamedDistribution {
    pub name: String,
    pub amount: f64,
}

pub struct DatedDistribution {
    pub date: String,
    pub values: Vec<NamedDistribution>,
}