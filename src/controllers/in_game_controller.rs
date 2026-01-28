use std::{
    collections::{HashMap, HashSet},
    process::Command,
    thread,
    time::Duration,
};

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use redis::{AsyncCommands, FromRedisValue, pipe};
use serde_json::json;

use crate::{
    app_state::AppState, auth::AuthUser, global_vars::USERNAME_REGEX,
    models::in_game::CharacterInfo,
};

// fn create_character_info_hash_fields(character_info: &CharacterInfo) -> Vec<(&str, String)> {
//     return vec![
//         ("max_hp", character_info.max_hp.clone().to_string()),
//         ("hp", character_info.hp.clone().to_string()),
//         (
//             "max_stamina",
//             character_info.max_stamina.clone().to_string(),
//         ),
//         (
//             "health_potion_quant",
//             character_info.health_potion_quant.clone().to_string(),
//         ),
//         ("state", character_info.state.to_string().clone()),
//     ];
// }

pub async fn save_character_stats(
    State(app_state_): State<AppState>,
    auth_user: AuthUser,
    Json(character_info_option): Json<Option<CharacterInfo>>,
) -> impl IntoResponse {
    if let None = character_info_option {
        return (StatusCode::BAD_REQUEST, "Character info empty !").into_response();
    }

    let character_info = character_info_option.unwrap();
    let character_info_json = json!(character_info).to_string();
    let key_list = format!("character_info:{}", auth_user.username);
    let mut redis_conn = app_state_.redis_conn.clone();
    if let Ok(current_lobby_id) = AsyncCommands::get::<_, String>(
        &mut redis_conn,
        format!("user:{}:lobby", auth_user.username),
    )
    .await
    {
        let lobby_key_list = format!("lobby:{}", current_lobby_id);
        if let Ok(lobby_status) =
            AsyncCommands::hget::<_, _, String>(&mut redis_conn, lobby_key_list, "status").await
        {
            if lobby_status == "In_Match" {
                if let Ok(_) =
                    AsyncCommands::set::<_, _, ()>(&mut redis_conn, key_list, character_info_json)
                        .await
                {
                    return (StatusCode::CREATED, Json(character_info)).into_response();
                }
            }
        }
    }
    return (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Error finishing the request, please try again !",
    )
        .into_response();
}

pub async fn get_character_stats(
    State(app_state_): State<AppState>,
    auth_user: AuthUser,
) -> impl IntoResponse {
    let key_list = format!("character_info:{}", auth_user.username);
    let mut redis_conn = app_state_.redis_conn.clone();
    if let Ok(character_info) = AsyncCommands::get::<_, String>(&mut redis_conn, key_list).await {
        return (StatusCode::OK, character_info).into_response();
    }
    return (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Error finishing the request, please try again !",
    )
        .into_response();
}

pub async fn remove_character_stats(
    State(app_state_): State<AppState>,
    auth_user: AuthUser,
) -> impl IntoResponse {
    let key_list = format!("character_info:{}", auth_user.username);
    let mut redis_conn = app_state_.redis_conn.clone();
    if let Ok(_) = AsyncCommands::del::<_, ()>(&mut redis_conn, key_list).await {
        return StatusCode::CREATED.into_response();
    }
    return (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Error finishing the request, please try again !",
    )
        .into_response();
}
