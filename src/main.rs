use redis::{
    AsyncConnectionConfig, ConnectionAddr, FromRedisValue, PushInfo, PushKind, RedisConnectionInfo,
};
use std::{collections::HashMap, io::Write, process::Child, sync::Arc};
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

use crate::app_state::GameServerExeMap;

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    dotenv().ok();

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

    let game_server_exe_map: GameServerExeMap =
        Arc::new(RwLock::new(HashMap::<String, Child>::new()));

    let app_state_ = AppState {
        connection_pool,
        clients_map,
        game_server_exe_map,
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
    if let Ok(()) = conn
        .subscribe(&["web_socket_events", "drop_game_server_event"])
        .await
    {
        loop {
            if let Some(redis_message) = rx.recv().await {
                if redis_message.kind == PushKind::Message
                    || redis_message.kind == PushKind::PMessage
                {
                    //println!("{:?}", redis_message);
                    if let Ok(redis_event) = String::from_redis_value(redis_message.data[0].clone())
                    {
                        match redis_event.as_str() {
                            "web_socket_events" => {
                                if let Ok(payload) =
                                    String::from_redis_value(redis_message.data[1].clone())
                                {
                                    if let Ok(payload_json) =
                                        serde_json::from_str::<serde_json::Value>(&payload)
                                    {
                                        //println!("{:?}", payload_json);
                                        let user_id =
                                            payload_json.get("username").unwrap().as_str().unwrap();
                                        let data = payload_json.get("data").unwrap();
                                        // println!("{:?}", user_id);
                                        // println!("{:?}", data);
                                        let clients_map = app_state_.clients_map.read().await;
                                        if let Some(sender) = clients_map.get(user_id) {
                                            let _ = sender.send(data.to_string());
                                        }
                                    }
                                }
                            }
                            "drop_game_server_event" => {
                                if let Ok(game_server_id) =
                                    String::from_redis_value(redis_message.data[1].clone())
                                {
                                    {
                                        // let mut game_server_exe_map =
                                        //     app_state_.game_server_exe_map.write().await;
                                        // if let Some(game_server_proccess) =
                                        //     game_server_exe_map.get_mut(&game_server_id)
                                        // {
                                        //     if let Ok(_) = game_server_proccess.kill() {
                                        //         game_server_exe_map.remove(&game_server_id);
                                        //     }
                                        // }
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}
