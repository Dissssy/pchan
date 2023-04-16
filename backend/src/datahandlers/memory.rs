use crate::statics;
use crate::structs::{StringData, Timed, UserData};
use crate::traits::Strings;
use anyhow::{anyhow, Result};
use common::structs::ShareData;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

#[derive(Clone, Debug, Deserialize, Serialize, Default)]
pub struct StringsMemory {
    strings: HashMap<String, Vec<u64>>,
    shares: HashMap<String, HashMap<String, u64>>,
    users: HashMap<String, f64>,
    history: HashMap<String, Vec<Timed<f64>>>,
    whitelist: Vec<String>,
    total: Vec<u64>,
}

impl StringsMemory {
    async fn save(&mut self) -> Result<()> {
        let mut file = tokio::fs::File::create("data.bin.gz").await?;

        let mut e = GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(&postcard::to_allocvec(&self)?)?;
        file.write_all(e.finish()?.as_slice()).await?;
        Ok(())
    }

    async fn load(&mut self) -> Result<()> {
        let mut file = tokio::fs::File::open("data.bin.gz").await?;

        let mut buf = Vec::new();
        file.read_to_end(&mut buf).await?;
        let mut gz = GzDecoder::new(buf.as_slice());
        let mut newbuf = Vec::new();
        gz.read_to_end(&mut newbuf)?;
        *self = postcard::from_bytes(&newbuf)?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl Strings for StringsMemory {
    async fn close(&mut self) -> Result<()> {
        self.save().await
    }

    async fn open(&mut self) -> Result<()> {
        self.load().await
    }

    async fn add(&mut self, string: &Timed<String>) -> Result<()> {
        let rawstring = &string.thing;
        let data = self
            .strings
            .entry(rawstring.clone())
            .or_insert_with(Vec::new);

        data.push(string.time);

        self.total.push(string.time);
        Ok(())
    }

    async fn add_all(&mut self, strings: &[Timed<String>]) -> Result<()> {
        for string in strings {
            self.add(string).await?;
        }
        Ok(())
    }

    async fn get(&self, string: String) -> Result<StringData> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let data = self
            .strings
            .get(&string)
            .ok_or(anyhow!("String not found"))?;
        let (shares, mut value) = if *statics::SHARE_TIME > *statics::VALUE_TIME {
            (
                data.len() as u64,
                data.iter()
                    .filter(|t| **t + *statics::VALUE_TIME > now)
                    .count() as f64,
            )
        } else {
            (
                data.iter()
                    .filter(|t| **t + *statics::SHARE_TIME > now)
                    .count() as u64,
                data.len() as f64,
            )
        };

        let owned_shares = if let Some(s) = self.shares.get(&string) {
            s.values().sum()
        } else {
            0
        };
        let shares = if shares < owned_shares {
            let percent = shares as f64 / owned_shares as f64;

            value *= percent;
            0
        } else {
            shares - owned_shares
        };

        let history = if let Some(h) = self.history.get(&string) {
            h.clone()
        } else {
            Vec::new()
        };
        Ok(StringData {
            string,
            available_shares: ((shares as f64 / self.total.len() as f64)
                * (self.strings.len() as f64 * 10.))
                .floor() as u64,
            value,
            history,
        })
    }

    async fn get_all(&self) -> Result<Vec<String>> {
        Ok(self.strings.keys().cloned().collect())
    }

    async fn trim(&mut self) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let remove = self
            .strings
            .iter_mut()
            .flat_map(|(k, v)| {
                v.retain(|t| *t + *statics::SHARE_TIME > now || *t + *statics::VALUE_TIME > now);
                if v.is_empty() {
                    Some(k.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<String>>();
        for rm in remove {
            self.strings.remove(&rm);
        }

        self.total.retain(|t| *t + *statics::SHARE_TIME > now);
        Ok(())
    }

    async fn make_history(&mut self) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut history = self.history.clone();
        for (string, _data) in self.strings.iter() {
            let data = self.get(string.clone()).await?;
            let h = history.entry(string.clone()).or_insert_with(Vec::new);

            let last = data.history.iter().max_by_key(|t| t.time).map(|t| t.thing);
            if let Some(last) = last {
                if last == data.value {
                    continue;
                }
            }
            h.push(Timed {
                thing: data.value,
                time: now,
            });
        }
        self.history = history;
        self.save().await?;
        Ok(())
    }

    async fn add_dirty(&mut self, string: Timed<String>) -> Result<()> {
        let (string, t) = string.split();
        let strings = string
            .to_lowercase()
            .chars()
            .filter(|c| !c.is_numeric())
            .map(|c| {
                if c.is_ascii_alphabetic() || c == '\'' {
                    c
                } else {
                    ' '
                }
            })
            .filter(|c| *c != '\'')
            .collect::<String>()
            .split_ascii_whitespace()
            .filter(|s| !s.is_empty())
            .map(|s| Timed {
                thing: s.to_string(),
                time: t,
            })
            .collect::<Vec<Timed<String>>>();
        self.add_all(&strings).await?;
        Ok(())
    }

    async fn get_user(&mut self, user_id: String) -> Result<UserData> {
        let cash = if let Some(u) = self.users.get(&user_id) {
            *u
        } else {
            return Err(anyhow!("User not found"));
        };

        let shares = self
            .shares
            .iter()
            .filter_map(|(string, users)| {
                users.get(&user_id).map(|amount| (string.clone(), *amount))
            })
            .collect::<HashMap<String, u64>>();

        let mut share_data = HashMap::new();
        for (string, amount) in shares.iter() {
            let data = self.get(string.clone()).await?;
            share_data.insert(
                string.clone(),
                ShareData {
                    amount: *amount,
                    value: data.value * *amount as f64,
                },
            );
        }
        let share_value = share_data.values().map(|s| s.value).sum();

        Ok(UserData {
            cash,
            id: user_id,
            shares: share_data,
            share_value,
        })
    }

    async fn buy_shares(&mut self, user_id: String, string: String, amount: u64) -> Result<()> {
        let data = self.get(string.clone()).await?;
        let user = self
            .users
            .get_mut(&user_id)
            .ok_or(anyhow!("User not found"))?;
        let cost = data.value * amount as f64;
        if *user < cost {
            return Err(anyhow!("User does not have enough money"));
        }
        *user -= cost;
        let shares = self
            .shares
            .entry(string)
            .or_insert_with(HashMap::new)
            .entry(user_id)
            .or_insert(0);
        *shares += amount;
        Ok(())
    }

    async fn sell_shares(&mut self, user_id: String, string: String, amount: u64) -> Result<()> {
        let data = self.get(string.clone()).await?;
        let user = self
            .users
            .get_mut(&user_id)
            .ok_or(anyhow!("User not found"))?;
        let shares = self
            .shares
            .entry(string)
            .or_insert_with(HashMap::new)
            .entry(user_id)
            .or_insert(0);
        if *shares < amount {
            return Err(anyhow!("User does not have enough shares"));
        }
        *shares -= amount;
        let cost = data.value * amount as f64;
        *user += cost;
        Ok(())
    }

    async fn verify_user(&mut self, user_id: String) -> Result<()> {
        self.users.entry(user_id).or_insert(*statics::STARTING_CASH);
        Ok(())
    }

    async fn whitelist(&mut self, user_id: String) -> Result<()> {
        if !self.whitelist.contains(&user_id) {
            self.whitelist.push(user_id);
        }
        Ok(())
    }

    async fn unwhitelist(&mut self, user_id: String) -> Result<()> {
        if let Some(i) = self.whitelist.iter().position(|u| *u == user_id) {
            self.whitelist.remove(i);
        }
        Ok(())
    }

    async fn is_whitelisted(&self, user_id: String) -> Result<bool> {
        Ok(self.whitelist.contains(&user_id))
    }

    async fn set_whitelist(&mut self, whitelist: Vec<String>) -> Result<()> {
        self.whitelist = whitelist;
        Ok(())
    }
}
