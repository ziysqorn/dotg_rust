use redis::{AsyncConnectionConfig, ConnectionAddr, FromRedisValue, PushInfo, RedisConnectionInfo};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{
    RwLock,
    mpsc::{self, UnboundedReceiver},
};

mod app_state;
mod auth;
mod controllers;
mod global_vars;
mod models;

use app_state::{AppState, ClientSender, ClientsMap};
use controllers::controllers_center;
use dotenvy::dotenv;
use sqlx::PgPool;

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    dotenv().expect("Error loading .env file");

    let connection_str = std::env::var("DATABASE_URL").expect("DATABASE_URL not found !");
    let redis_url = std::env::var("REDIS_URL").expect("REDIS_URL not found !");
    //Setup Redis Client
    let client_redis = redis::Client::open(redis_url).expect("Can't connect to Redis");

    let connection_pool = PgPool::connect(connection_str.as_str()).await?;
    //Setup Redis connection
    let (tx, rx) = mpsc::unbounded_channel();
    let config = AsyncConnectionConfig::new().set_push_sender(tx.clone());
    let redis_conn = client_redis
        .get_multiplexed_async_connection_with_config(&config)
        .await
        .expect("Error getting Redis connection");
    //
    let clients_map: ClientsMap = Arc::new(RwLock::new(HashMap::<String, ClientSender>::new()));

    let app_state_ = AppState {
        connection_pool,
        clients_map,
        redis_conn,
    };

    let redis_app_state = app_state_.clone();

    tokio::spawn(async {
        subcribe_to_channel(redis_app_state, rx).await;
    });

    let app_routers = controllers_center::create_app_router().with_state(app_state_);
    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app_routers).await?;
    Ok(())
}

async fn subcribe_to_channel(app_state_: AppState, mut rx: UnboundedReceiver<PushInfo>) {
    let mut conn = app_state_.redis_conn.clone();
    if let Ok(()) = conn.subscribe(&["web_socket_events"]).await {
        loop {
            if let Some(redis_message) = rx.recv().await {
                if let Ok(payload) = String::from_redis_value(redis_message.data[1].clone()) {
                    if let Ok(payload_json) = serde_json::from_str::<serde_json::Value>(&payload) {
                        let user_id = payload_json.get("username").unwrap().to_string();
                        let data = payload_json.get("data").unwrap();
                        let clients_map = app_state_.clients_map.read().await;
                        if let Some(sender) = clients_map.get(&user_id) {
                            let _ = sender.send(data.to_string());
                        }
                    }
                }
            }
        }
    }
}
