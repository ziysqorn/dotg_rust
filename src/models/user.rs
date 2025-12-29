use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

#[derive(Clone, Debug, Deserialize, Serialize, FromRow)]
pub struct User {
    username: String,
    user_password: String,
    status: bool,
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
