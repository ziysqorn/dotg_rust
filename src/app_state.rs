use redis::aio::MultiplexedConnection;
use sqlx::PgPool;
use tokio::sync::RwLock;

use std::{collections::HashMap, process::Child, sync::Arc};

pub type ClientSender = tokio::sync::mpsc::UnboundedSender<String>;

//Map to store a mpsc Sender of the coresponding user
pub type ClientsMap = Arc<RwLock<HashMap<String, ClientSender>>>;

pub type GameServerExeMap = Arc<RwLock<HashMap<String, Child>>>;

//AppState that contains Connection Pool and Clients Map for Web Socket
#[derive(Clone)]
pub struct AppState {
    pub connection_pool: PgPool,
    pub clients_map: ClientsMap,
    pub game_server_exe_map: GameServerExeMap,
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

impl axum::extract::FromRef<AppState> for GameServerExeMap {
    fn from_ref(state: &AppState) -> Self {
        state.game_server_exe_map.clone()
    }
}

impl axum::extract::FromRef<AppState> for MultiplexedConnection {
    fn from_ref(state: &AppState) -> Self {
        state.redis_conn.clone()
    }
}
