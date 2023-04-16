lazy_static::lazy_static!(
    pub static ref TRIM_TIME: u64 = env!("TRIM_TIME").parse::<u64>().unwrap();
    pub static ref TOKEN: String = env!("TOKEN").to_owned();
    pub static ref HASH_SALT: String = env!("HASH_SALT").to_owned();
    pub static ref DELETE_TIME: u64 = env!("DELETE_TIME").parse::<u64>().unwrap();
);
