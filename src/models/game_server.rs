use redis_macros::{FromRedisValue, ToRedisArgs};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

#[derive(Clone, Debug, Deserialize, Serialize, FromRow, FromRedisValue, ToRedisArgs)]
pub struct GameServer {
    pub address: String,
    pub host: String,
}
impl GameServer {
    pub fn new(in_address: &str, in_host: &str) -> Self {
        Self {
            address: in_address.to_string(),
            host: in_host.to_string(),
        }
    }
}
