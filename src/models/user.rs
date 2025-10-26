use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

#[derive(Clone, Debug, Deserialize, Serialize, FromRow)]
pub struct User {
    username: String,
    user_password: String,
    status: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, FromRow)]
pub struct Friends {
    pub player1: String,
    pub player2: String,
}

impl User {
    pub fn new(in_username: &str, in_password: &str, in_status: &bool) -> Self {
        Self {
            username: in_username.to_string(),
            user_password: in_password.to_string(),
            status: *in_status,
        }
    }
    pub fn get_username(&self) -> String {
        self.username.clone()
    }
    pub fn get_password(&self) -> String {
        self.user_password.clone()
    }
    pub fn get_status(&self) -> bool {
        self.status
    }
}

impl Friends {
    pub fn new(player1_username: &str, player2_username: &str) -> Self {
        Self {
            player1: player1_username.to_string(),
            player2: player2_username.to_string(),
        }
    }
    pub fn get_player1(&self) -> String {
        self.player1.clone()
    }
    pub fn get_player2(&self) -> String {
        self.player2.clone()
    }
}
