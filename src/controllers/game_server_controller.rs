use std::{
    collections::{HashMap, HashSet},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use redis::{AsyncCommands, FromRedisValue, aio::MultiplexedConnection, pipe};
use serde_json::json;

use crate::{
    app_state::AppState,
    auth::AuthUser,
    global_vars::USERNAME_REGEX,
    models::{game_server::GameServer, lobby::LobbyInfo},
};

fn create_game_server_info_hash_fields(game_server_info: &GameServer) -> Vec<(&str, String)> {
    return vec![
        ("address", game_server_info.address.clone()),
        ("host", game_server_info.host.clone()),
    ];
}

pub async fn create_game_server(
    State(app_state_): State<AppState>,
    auth_user: AuthUser,
) -> impl IntoResponse {
    // if query_payload.is_empty() {
    //     return (StatusCode::BAD_REQUEST, "Query empty !").into_response();
    // }
    if !USERNAME_REGEX.is_match(&auth_user.username) {
        return (StatusCode::BAD_REQUEST, "Invalid username format !").into_response();
    }
    let mut redis_conn = app_state_.redis_conn.clone();
    //let mut pipe = redis::pipe();
    if let Ok(current_lobby_id) = AsyncCommands::get::<_, String>(
        &mut redis_conn,
        format!("user:{}:lobby", auth_user.username),
    )
    .await
    {
        let key_list = format!("lobby:{}", current_lobby_id);
        let game_server_info_key = format!("game_server:{}", current_lobby_id);
        if let Ok(leader_opt) =
            AsyncCommands::hget::<_, _, Option<String>>(&mut redis_conn, &key_list, "leader").await
        {
            if let Some(leader) = leader_opt {
                if leader != auth_user.username {
                    return (
                        StatusCode::BAD_REQUEST,
                        "Non lobby leader can't start the request !",
                    )
                        .into_response();
                }
            }
        }
        if let Ok(game_server_info_opt) =
            AsyncCommands::get::<_, Option<String>>(&mut redis_conn, &game_server_info_key).await
        {
            if let Some(game_server_info_str) = game_server_info_opt {
                if !game_server_info_str.is_empty() {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "The server is currently busy. Please try again !",
                    )
                        .into_response();
                }
            }
        }
        if let Ok(listener) = tokio::net::TcpListener::bind("0.0.0.0:0").await {
            if let Ok(address) = listener.local_addr() {
                let port = address.port();
                if let Ok(exec) = Command::new(r"D:\GameBuilds\WindowsServer\BeatHimUpServer.exe")
                    .arg(format!("Level_MainLevel?port={}", port))
                    .arg("-nopause")
                    .arg("-log")
                    .arg(format!("-server_id={}", current_lobby_id))
                    .stdin(Stdio::piped())
                    .spawn()
                {
                    //let mut pipe = redis::pipe();
                    let response_address = format!("127.0.0.1:{}", port);
                    let server_info = GameServer::new(&response_address, &auth_user.username);
                    if let Ok(_) = AsyncCommands::set::<_, _, ()>(
                        &mut redis_conn,
                        &game_server_info_key,
                        json!(server_info).to_string(),
                    )
                    .await
                    {
                        if let Ok(_) = AsyncCommands::hset::<_, _, _, ()>(
                            &mut redis_conn,
                            &key_list,
                            "status",
                            "In_Match",
                        )
                        .await
                        {
                            if let Ok(member_set) = AsyncCommands::smembers::<_, HashSet<String>>(
                                &mut redis_conn,
                                format!("{}:members", &key_list),
                            )
                            .await
                            {
                                drop(listener);
                                for member in member_set.iter() {
                                    let _ = AsyncCommands::del::<_, ()>(
                                        &mut redis_conn,
                                        format!("character_info:{}", member),
                                    )
                                    .await;
                                    if member == &auth_user.username {
                                        continue;
                                    }
                                    let data_to_lobby = json!({
                                        "resource": "game_server",
                                        "action": "create",
                                        "payload": {
                                            "game_server": json!(server_info)
                                        }
                                    });
                                    let pub_sub_data_json = json!({
                                        "username": member,
                                        "data": data_to_lobby
                                    });
                                    let _ = AsyncCommands::publish::<_, _, ()>(
                                        &mut redis_conn,
                                        "web_socket_events",
                                        pub_sub_data_json.to_string(),
                                    )
                                    .await;
                                }
                            }
                            // //Insert a new game server proccess to map
                            // {
                            //     let mut game_server_exe_map_write =
                            //         app_state_.game_server_exe_map.write().await;
                            //     game_server_exe_map_write.insert(current_lobby_id, exec);
                            // }
                            return (StatusCode::CREATED, Json(server_info)).into_response();
                        }
                    }
                } else {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to start new Server process !",
                    )
                        .into_response();
                }
            } else {
                return (StatusCode::SERVICE_UNAVAILABLE, "No port available !").into_response();
            }
        } else {
            return (StatusCode::SERVICE_UNAVAILABLE, "No address available !").into_response();
        }
    }
    return (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Error proccessing the request !",
    )
        .into_response();
}

pub async fn drop_game_server(
    State(app_state_): State<AppState>,
    Query(query_payload): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if query_payload.is_empty() {
        return (StatusCode::BAD_REQUEST, "Query empty !").into_response();
    }
    let mut redis_conn = app_state_.redis_conn.clone();
    if let Some(current_lobby_id) = query_payload.get("server_id") {
        let key_list = format!("lobby:{}", current_lobby_id);
        let game_server_info_key = format!("game_server:{}", current_lobby_id);
        let mut pipe = redis::pipe();
        pipe.atomic()
            .del(game_server_info_key)
            .hset(&key_list, "status", "Ready")
            .smembers(format!("{}:members", &key_list));
        if let Ok((_, _, member_set)) = pipe
            .query_async::<((), (), HashSet<String>)>(&mut redis_conn)
            .await
        {
            for member in member_set {
                let _ = AsyncCommands::del::<_, ()>(
                    &mut redis_conn,
                    format!("character_info:{}", member),
                )
                .await;
            }
            return (StatusCode::CREATED, "Dropped server !").into_response();
        }
    }

    return (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Error proccessing the request !",
    )
        .into_response();
}
