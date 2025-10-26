use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};

use futures_util::{SinkExt, stream::StreamExt};
use regex::Regex;
use serde_json::{Map, Value, json};
use sqlx::{PgPool, Row, postgres::PgRow};

pub async fn handle_web_socket_request(
    web_socket_upgrade: WebSocketUpgrade,
    State(connection_pool): State<PgPool>,
) -> impl IntoResponse {
    return web_socket_upgrade
        .on_upgrade(|socket| async move { handle_socket(socket, connection_pool.clone()).await });
}

pub async fn handle_socket(socket: WebSocket, connection_pool: PgPool) {
    println!("Connected to a client !");
    let (mut sender, mut receiver) = socket.split();

    let (sender_in_channel, mut receiver_in_chanel) =
        tokio::sync::mpsc::unbounded_channel::<String>();

    let passive_channel_sender = sender_in_channel.clone();
    let receive_task = tokio::spawn(async move {
        while let Some(Ok(message)) = receiver.next().await {
            match message {
                Message::Text(text) => {
                    if let Ok(text_as_json) = serde_json::from_str::<Value>(text.as_str()) {
                        if let Some(resource_field) = text_as_json.get("resource")
                            && let Some(action_field) = text_as_json.get("action")
                            && let Some(payload_field) = text_as_json.get("payload")
                        {
                            if let Some(resource_str) = resource_field.as_str()
                                && let Some(action_str) = action_field.as_str()
                                && let Some(payload) = payload_field.as_object()
                            {
                                if let Some(result) = get_friendlist(
                                    &connection_pool,
                                    resource_str,
                                    action_str,
                                    payload,
                                )
                                .await
                                {
                                    if let Err(e) = passive_channel_sender.send(result) {
                                        println!("Error sending message to mpsc chancel: {}", e);
                                    }
                                }
                            }
                        }
                    }
                }
                Message::Close(close_frame) => {
                    println!("Client closed connection !");
                }
                _ => {}
            }
        }
    });

    let sender_task = tokio::spawn(async move {
        while let Some(message) = receiver_in_chanel.recv().await {
            let temp = message.clone();
            if let Err(e) = sender.send(Message::Text(message.into())).await {
                println!("{}", e);
            } else {
                println!("{}", temp);
            }
        }
    });
}

pub async fn get_friendlist(
    connection_pool: &PgPool,
    resource: &str,
    action: &str,
    payload: &Map<String, Value>,
) -> Option<String> {
    if resource == "friendlist" && action == "get" {
        if !payload.is_empty() {
            if let Some(username) = payload.get("username") {
                if let Some(username_str) = username.as_str() {
                    if let Ok(username_regex) = Regex::new("^[a-zA-Z0-9@]{1,12}$")
                        && !username_regex.is_match(username_str)
                    {
                        return Some("Invalid username format !".to_string());
                    }
                    let mut result_friendlist: Vec<PgRow> = Vec::new();
                    if let Ok(mut friend_list1) = sqlx::query(
                "Select u.username, u.status from users u inner join friends f on u.username = f.player2 where f.player1 = $1"
            )
            .bind(username_str)
            .fetch_all(connection_pool)
            .await
            {
                result_friendlist.append(&mut friend_list1);
            }
                    if let Ok(mut friend_list2) = sqlx::query(
                "Select u.username, u.status from users u inner join friends f on u.username = f.player1 where f.player2 = $1"
            )
            .bind(username_str)
            .fetch_all(connection_pool)
            .await
            {
                result_friendlist.append(&mut friend_list2);
            }
                    if result_friendlist.is_empty() {
                        return Some("Friendlist Empty !".to_string());
                    } else {
                        let final_friendlist: Vec<serde_json::Value> = result_friendlist.iter().map(|row| json!({"username": row.get::<String, _>("username"), "status": row.get::<bool, _>("status")})).collect();
                        if let Ok(final_friendlist_json) = serde_json::to_string(&final_friendlist)
                        {
                            Some(final_friendlist_json)
                        } else {
                            Some("Error parsing friendlist !".to_string())
                        }
                    }
                } else {
                    Some("Username not a string !".to_string())
                }
            } else {
                Some("Missing username !".to_string())
            }
        } else {
            Some("Missing payload !".to_string())
        }
    } else {
        None
    }
}
