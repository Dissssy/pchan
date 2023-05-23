lazy_static::lazy_static!(
    pub static ref TRIM_TIME: u64 = env!("TRIM_TIME").parse::<u64>().expect("TRIM_TIME must be a valid u64");
    pub static ref TOKEN: String = env!("TOKEN").to_owned();
    pub static ref HASH_SALT: String = env!("HASH_SALT").to_owned();
    pub static ref DELETE_TIME: u64 = env!("DELETE_TIME").parse::<u64>().expect("DELETE_TIME must be a valid u64");
    pub static ref TOKEN_SALT: String = env!("TOKEN_SALT").to_owned();
    pub static ref KNOWN_SCRAPERS: Vec<&'static str> = vec!["Mozilla/5.0 (compatible; Discordbot/2.0; +https://discordapp.com)"];
    pub static ref BASE_THUMBNAIL: &'static [u8] = include_bytes!(env!("BASE_THUMBNAIL_PATH"));
    pub static ref BASE_THUMBNAIL_LARGE: &'static [u8] = include_bytes!(env!("BASE_THUMBNAIL_LARGE_PATH"));
    pub static ref FONT: &'static [u8] = include_bytes!(env!("FONT_PATH"));
);
