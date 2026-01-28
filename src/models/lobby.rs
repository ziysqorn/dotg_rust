use redis_macros::{FromRedisValue, ToRedisArgs};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

#[derive(Clone, Debug, Deserialize, Serialize, FromRow, FromRedisValue, ToRedisArgs)]
pub struct LobbyInfo {
    pub lobby_name: String,
    pub leader: String,
    pub limit_num: usize,
    //Status - Ready | In_Queue | In_Match
    pub status: String,
}

impl LobbyInfo {
    pub fn new(in_name: &str, in_leader: &str, in_limit_num: usize, in_status: &str) -> Self {
        Self {
            lobby_name: in_name.to_string(),
            leader: in_leader.to_string(),
            limit_num: in_limit_num,
            //Status - Ready | In_Queue | In_Match
            status: in_status.to_string(),
        }
    }
}
