use axum::{
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::IntoResponse,
};

use std::collections::HashMap;

use futures_util::{SinkExt, stream::StreamExt};
use serde_json::{Map, Value, json};

use crate::app_state::AppState;
use crate::global_vars::USERNAME_REGEX;

pub async fn handle_web_socket_request(
    web_socket_upgrade: WebSocketUpgrade,
    State(app_state): State<AppState>,
    query_params: Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if query_params.is_empty() {
        return (StatusCode::BAD_REQUEST, "Params empty !").into_response();
    }

    if let Some(tmp_user_id) = query_params.get("username") {
        if !USERNAME_REGEX.is_match(tmp_user_id) {
            return (StatusCode::BAD_REQUEST, "Invalid username format !").into_response();
        }

        let username = tmp_user_id.clone();
        return web_socket_upgrade.on_upgrade(|socket| async move {
            handle_socket(socket, app_state.clone(), &username.clone()).await
        });
    }
    return (StatusCode::BAD_REQUEST, "No username found !").into_response();
}

// The client and server will communicate based on the data format below

// {
//     "resource": "friendlist",
//     "action": "get",
//     "payload": {
//         "username": "abc",
//         "message": "abc",
//         ...
//     }
// }

pub async fn handle_socket(socket: WebSocket, app_state_: AppState, username: &String) {
    println!("Connected to a client !");
    let (mut sender, mut receiver) = socket.split();

    let (sender_in_channel, mut receiver_in_channel) =
        tokio::sync::mpsc::unbounded_channel::<String>();

    let passive_channel_sender = sender_in_channel.clone();

    {
        let mut map = app_state_.clients_map.write().await;
        map.insert(username.clone(), passive_channel_sender.clone());
        println!("User {:?} is online now !", username);
    }

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
                                // if let Some(result) = get_friendlist(
                                //     &app_state_.connection_pool,
                                //     resource_str,
                                //     action_str,
                                //     payload,
                                // )
                                // .await
                                // {
                                //     if let Err(e) = passive_channel_sender.send(result) {
                                //         println!("Error sending message to mpsc chancel: {}", e);
                                //     }
                                // }
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
        while let Some(message) = receiver_in_channel.recv().await {
            let temp = message.clone();
            if let Err(e) = sender.send(Message::Text(message.into())).await {
                println!("{}", e);
            } else {
                //println!("{}", temp);
            }
        }
    });
}
