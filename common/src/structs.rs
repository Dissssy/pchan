use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub cash: f64,
    pub shares: HashMap<String, ShareData>,
    pub share_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShareData {
    pub amount: u64,
    pub value: f64,
}
