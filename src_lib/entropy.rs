pub struct RangeCoder {
    pub symbol_counts: Vec<usize>,
}

impl RangeCoder {
    pub fn new() -> Self {
        Self {
            symbol_counts: Vec::new(),
        }
    }
}
