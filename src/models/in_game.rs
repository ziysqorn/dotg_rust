use redis_macros::{FromRedisValue, ToRedisArgs};
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

#[derive(Clone, Debug, Deserialize, Serialize, FromRow, FromRedisValue, ToRedisArgs)]
pub struct CharacterInfo {
    pub max_hp: f32,
    pub hp: f32,
    pub max_stamina: f32,
    pub health_potion_quant: usize,
    pub state: String,
}
