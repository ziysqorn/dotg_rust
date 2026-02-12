use jsonwebtoken::{EncodingKey, Header, encode};
use redis::{AsyncCommands, FromRedisValue};
use serde_json::{Map, Value, json};
use std::{
    collections::{HashMap, HashSet},
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use regex::Regex;
use sqlx::{PgPool, Row, postgres::PgRow};

use crate::{
    app_state::AppState,
    auth::{AuthUser, Claims, get_jwt_secret},
    controllers::lobby_controller,
    global_vars::USERNAME_REGEX,
    models::{game_server::GameServer, in_game::CharacterInfo, lobby::LobbyInfo, user::User},
};

pub async fn create_user(
    State(connection_pool): State<PgPool>,
    Json(payload): Json<User>,
) -> impl IntoResponse {
    if let Ok(in_username_regex) = regex::Regex::new("^[a-zA-Z0-9@]{1,12}$") {
        if !in_username_regex.is_match(&payload.get_username()) {
            return (StatusCode::BAD_REQUEST, "Invalid username format !").into_response();
        }
    }
    if let Ok(in_password_regex) = regex::Regex::new("^[a-zA-Z0-9*@]{1,12}$") {
        if !in_password_regex.is_match(&payload.get_password()) {
            return (StatusCode::BAD_REQUEST, "Invalid password format !").into_response();
        }
    }
    let query_prompt =
        sqlx::query("Insert into users (username, user_password, status) values ($1, $2, $3)")
            .bind(payload.get_username())
            .bind(payload.get_password())
            .bind(payload.get_status())
            .execute(&connection_pool)
            .await;
    match query_prompt {
        Ok(result) => {
            println!("{:?}", result);
            return (StatusCode::CREATED, "User has been created successfully").into_response();
        }
        Err(_e) => {
            return (StatusCode::CONFLICT, _e.to_string()).into_response();
        }
    }
}

pub async fn login(
    State(app_state_): State<AppState>,
    Json(payload): Json<HashMap<String, String>>,
) -> impl IntoResponse {
    if payload.is_empty() {
        return (StatusCode::BAD_REQUEST, "Login information empty !").into_response();
    }

    let mut in_username = "";
    let mut in_password = "";

    if let Some(temp_username) = payload.get("username") {
        in_username = &temp_username;
        if !USERNAME_REGEX.is_match(&in_username) {
            return (StatusCode::BAD_REQUEST, "Invalid username format !").into_response();
        }
    } else {
        return (StatusCode::BAD_REQUEST, "Missing username !").into_response();
    }

    if let Some(temp_password) = payload.get("user_password") {
        in_password = &temp_password;

        if let Ok(password_regex) = Regex::new("^[a-zA-Z0-9*@]{1,12}$")
            && !password_regex.is_match(in_password)
        {
            return (StatusCode::BAD_REQUEST, "Invalid password format !").into_response();
        }
    } else {
        return (StatusCode::BAD_REQUEST, "Missing password !").into_response();
    }

    let login_user = User::new(in_username, in_password, &true);
    if let Ok(found_user) = sqlx::query("Select username from users where username = $1")
        .bind(login_user.get_username())
        .fetch_one(&app_state_.connection_pool)
        .await
    {
        println!("{:?}", found_user);
        let result = sqlx::query(
            "Select username, status from users where username = $1 and user_password = $2",
        )
        .bind(login_user.get_username())
        .bind(login_user.get_password())
        .fetch_one(&app_state_.connection_pool)
        .await;
        match result {
            Ok(found_row) => {
                let online_status = found_row.get::<bool, _>("status");
                if online_status {
                    return (
                        StatusCode::UNAUTHORIZED,
                        "User has already logged in another device",
                    )
                        .into_response();
                }
                //Create JWT------------------------------------------------------------------//
                let expiration = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs() as usize
                    + (24 * 3600);

                let claims = Claims {
                    subject: in_username.to_string().clone(),
                    exp: expiration,
                };

                if let Ok(token) = encode(
                    &Header::default(),
                    &claims,
                    &EncodingKey::from_secret(get_jwt_secret()),
                ) {
                    if let Err(update_status_error) =
                        sqlx::query("Update users set status = true where username = $1")
                            .bind(login_user.get_username())
                            .execute(&app_state_.connection_pool)
                            .await
                    {
                        println!("{:?}", update_status_error);
                        return (StatusCode::UNAUTHORIZED, "Error logging in").into_response();
                    }
                    let mut redis_conn = app_state_.redis_conn.clone();

                    let mut lobby_info_response: serde_json::Value = json!({});
                    let mut game_server_info = GameServer {
                        address: "".to_string(),
                        host: "".to_string(),
                    };

                    if let Ok(current_lobby_id) = AsyncCommands::get::<_, String>(
                        &mut redis_conn,
                        format!("user:{}:lobby", in_username),
                    )
                    .await
                    {
                        let game_server_info_key = format!("game_server:{}", current_lobby_id);
                        let lobby_info_keylist = format!("lobby:{}", current_lobby_id);
                        if let Ok(game_server_info_opt) = AsyncCommands::get::<_, Option<String>>(
                            &mut redis_conn,
                            game_server_info_key,
                        )
                        .await
                        {
                            let lobby_member_keylist = format!("{}:members", lobby_info_keylist);
                            if let Some(game_server_info_str) = game_server_info_opt {
                                if !game_server_info_str.is_empty() {
                                    let mut pipe = redis::pipe();
                                    pipe.atomic()
                                        .sadd(&lobby_member_keylist, in_username)
                                        .hgetall(lobby_info_keylist);
                                    if let Ok((_, lobby_info_map)) = pipe
                                        .query_async::<((), HashMap<String, String>)>(
                                            &mut redis_conn,
                                        )
                                        .await
                                    {
                                        if !lobby_info_map.is_empty() {
                                            game_server_info = serde_json::from_str::<GameServer>(
                                                &game_server_info_str,
                                            )
                                            .expect("Can't parse string data to struct");
                                            if let Ok(member_set) =
                                                AsyncCommands::smembers::<_, HashSet<String>>(
                                                    &mut redis_conn,
                                                    lobby_member_keylist,
                                                )
                                                .await
                                            {
                                                let lobby_info = LobbyInfo {
                                                    lobby_name: lobby_info_map
                                                        .get("lobby_name")
                                                        .expect("Error getting value from map")
                                                        .clone(),
                                                    leader: lobby_info_map
                                                        .get("leader")
                                                        .expect("Error getting value from map")
                                                        .clone(),
                                                    limit_num: lobby_info_map
                                                        .get("leader")
                                                        .expect("Error getting value from map")
                                                        .clone()
                                                        .parse()
                                                        .unwrap_or(5),
                                                    status: lobby_info_map
                                                        .get("status")
                                                        .expect("Error getting value from map")
                                                        .clone(),
                                                };
                                                lobby_info_response = json!({
                                                    "lobby_name": lobby_info.lobby_name,
                                                    "leader": lobby_info.leader,
                                                    "limit_num": lobby_info.limit_num,
                                                    "status": lobby_info.status,
                                                    "members": member_set
                                                });
                                                for member in member_set {
                                                    if &member == in_username {
                                                        continue;
                                                    }
                                                    let data_to_lobby = json!({
                                                        "resource": "lobby",
                                                        "action": "player_join",
                                                        "payload": {
                                                            "username": in_username
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
                                        }
                                    }
                                }
                            }
                        }
                    }
                    return (
                        StatusCode::OK,
                        Json(json!({
                            "token": token,
                            "username": found_row.get::<String, _>("username"),
                            "game_server": game_server_info,
                            "lobby": lobby_info_response,
                            "status": true
                        })),
                    )
                        .into_response();
                } else {
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Unable to analyze the token",
                    )
                        .into_response();
                }
            }
            Err(_e) => {
                return (StatusCode::UNAUTHORIZED, "Wrong password !").into_response();
            }
        }
    } else {
        return (StatusCode::NOT_FOUND, "User not found !").into_response();
    }
}

pub async fn logout(State(app_state_): State<AppState>, auth_user: AuthUser) -> impl IntoResponse {
    if !USERNAME_REGEX.is_match(&auth_user.username) {
        return (StatusCode::BAD_REQUEST, "Invalid username format !").into_response();
    }

    let mut redis_conn = app_state_.redis_conn.clone();

    if let Err(update_status_error) =
        sqlx::query("Update users set status = false where username = $1")
            .bind(&auth_user.username)
            .execute(&app_state_.connection_pool)
            .await
    {
        println!("{:?}", update_status_error);
        return (StatusCode::CONFLICT, "Error logging out !").into_response();
    }

    lobby_controller::leave_lobby_proccess(&auth_user.username, redis_conn.clone()).await;

    {
        let mut clients_map = app_state_.clients_map.write().await;
        clients_map.remove(&auth_user.username);
    }

    return (StatusCode::OK, "Logout successful").into_response();
}
