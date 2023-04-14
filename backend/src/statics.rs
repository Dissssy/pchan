lazy_static::lazy_static!(
    // pub static ref SHARE_TIME: u64 = env!("SHARE_TIME").parse::<u64>().unwrap();
    // pub static ref VALUE_TIME: u64 = env!("VALUE_TIME").parse::<u64>().unwrap();
    pub static ref TRIM_TIME: u64 = env!("TRIM_TIME").parse::<u64>().unwrap();
    // pub static ref HISTORY_TIME: u64 = env!("HISTORY_TIME").parse::<u64>().unwrap();
    // // pub static ref TOTAL_SHARES: u64 = env!("TOTAL_SHARES").parse::<u64>().unwrap();
    // pub static ref STARTING_CASH: f64 = env!("STARTING_CASH").parse::<f64>().unwrap();
    pub static ref TOKEN: String = env!("TOKEN").to_owned();
    pub static ref HASH_SALT: String = env!("HASH_SALT").to_owned();
);
