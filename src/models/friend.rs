use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

#[derive(Clone, Debug, Deserialize, Serialize, FromRow)]
pub struct Friends {
    pub player1: String,
    pub player2: String,
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

#[derive(Clone, Debug, Deserialize, Serialize, FromRow)]
pub struct FriendRequest {
    pub sender: String,
    pub receiver: String,
}

impl FriendRequest {
    pub fn new(player1_username: &str, player2_username: &str) -> Self {
        Self {
            sender: player1_username.to_string(),
            receiver: player2_username.to_string(),
        }
    }
    pub fn get_sender(&self) -> String {
        self.sender.clone()
    }
    pub fn get_receiver(&self) -> String {
        self.receiver.clone()
    }
}
