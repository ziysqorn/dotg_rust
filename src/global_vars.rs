use std::{
    collections::HashMap,
    sync::{Arc, LazyLock, OnceLock},
};

use regex::Regex;

//Global variables
pub static USERNAME_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("^[a-zA-Z0-9@]{1,12}$").expect("Invalid regex !"));

pub static SECRET_KEY: OnceLock<String> = OnceLock::new();
