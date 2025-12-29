use redis::aio::MultiplexedConnection;
use sqlx::PgPool;
use tokio::sync::RwLock;

use std::{
    collections::HashMap,
    sync::{Arc, LazyLock},
};

pub type ClientSender = tokio::sync::mpsc::UnboundedSender<String>;

pub type ClientsMap = Arc<RwLock<HashMap<String, ClientSender>>>;

//AppState that contains Connection Pool and Clients Map for Web Socket
#[derive(Clone)]
pub struct AppState {
    pub connection_pool: PgPool,
    pub clients_map: ClientsMap,
    pub redis_conn: MultiplexedConnection,
}

impl axum::extract::FromRef<AppState> for PgPool {
    fn from_ref(state: &AppState) -> Self {
        state.connection_pool.clone()
    }
}

impl axum::extract::FromRef<AppState> for ClientsMap {
    fn from_ref(state: &AppState) -> Self {
        state.clients_map.clone()
    }
}

impl axum::extract::FromRef<AppState> for MultiplexedConnection {
    fn from_ref(state: &AppState) -> Self {
        state.redis_conn.clone()
    }
}
