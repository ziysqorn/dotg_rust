use redis::{AsyncCommands, RedisError, RedisResult};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet};

use axum::{
    Error, Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use sqlx::{PgPool, Row, postgres::PgRow};

use crate::{auth::AuthUser, models::user::User};

use crate::global_vars::USERNAME_REGEX;

use crate::app_state::AppState;

pub async fn create_lobby(
    State(app_state_): State<AppState>,
    claims: AuthUser,
) -> impl IntoResponse {
    let username = &claims.username;

    if !USERNAME_REGEX.is_match(username) {
        return (StatusCode::BAD_REQUEST, "Invalid username format !").into_response();
    }

    let mut redis_conn = app_state_.redis_conn.clone();
    let lobby_id = format!("lobby_{}", username);
    let key_list = format!("lobby:{}", lobby_id);
    let lobby_json = json!({
        "name": format!("{}'s lobby", username),
        "leader": username,
    });
    let mut pipe = redis::pipe();
    pipe.atomic()
        .set(&key_list, &lobby_json.to_string())
        .set(format!("user:{}:lobby", username), &lobby_id)
        .sadd("active_lobbies", &lobby_id)
        .sadd(format!("{}:members", &key_list), username);

    if let Ok(()) = pipe.query_async(&mut redis_conn).await {
        return (StatusCode::CREATED, lobby_json.to_string()).into_response();
    }

    return (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Error finishing the request, please try again !",
    )
        .into_response();
}

pub async fn invite_to_lobby(
    State(app_state_): State<AppState>,
    claims: AuthUser,
    query_params: Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if query_params.is_empty() {
        return (StatusCode::BAD_REQUEST, "Params empty !").into_response();
    }
    let request_sender = &claims.username;
    let mut request_receiver = "";

    if !USERNAME_REGEX.is_match(request_sender) {
        return (StatusCode::BAD_REQUEST, "Invalid sender username format !").into_response();
    }

    if let Some(receiver_username) = query_params.get("receiver") {
        request_receiver = &receiver_username;
        if !USERNAME_REGEX.is_match(request_receiver) {
            return (
                StatusCode::BAD_REQUEST,
                "Invalid receiver username format !",
            )
                .into_response();
        }
    } else {
        return (StatusCode::BAD_REQUEST, "Missing receiver !").into_response();
    }

    let mut redis_conn = app_state_.redis_conn.clone();
    let data_to_receiver = json!({
        "resource": "lobby_invitation",
        "action": "receive",
        "payload": {
            "sender": request_sender,
            "receiver": request_receiver
        }
    });
    let pub_sub_data_json = json!({
        "username": request_receiver,
        "data": data_to_receiver
    });
    if let Ok(()) = AsyncCommands::publish(
        &mut redis_conn,
        "web_socket_events",
        pub_sub_data_json.to_string(),
    )
    .await
    {
        return (StatusCode::CREATED, "Invitation sent successfully !").into_response();
    }
    return (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Error finishing the request, please try again !",
    )
        .into_response();
}

pub async fn accept_lobby_invitation(
    State(app_state_): State<AppState>,
    claims: AuthUser,
    query_params: Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if query_params.is_empty() {
        return (StatusCode::BAD_REQUEST, "Params empty !").into_response();
    }

    let mut request_sender = "";
    let request_receiver = &claims.username;
    if let Some(sender_username) = query_params.get("sender") {
        request_sender = &sender_username;
        if !USERNAME_REGEX.is_match(request_sender) {
            return (StatusCode::BAD_REQUEST, "Invalid sender username format !").into_response();
        }
    } else {
        return (StatusCode::BAD_REQUEST, "Missing sender !").into_response();
    }

    if !USERNAME_REGEX.is_match(request_receiver) {
        return (
            StatusCode::BAD_REQUEST,
            "Invalid receiver username format !",
        )
            .into_response();
    }

    let mut redis_conn = app_state_.redis_conn.clone();
    let lobby_id = format!("lobby_{}", &request_sender);
    let key_list = format!("lobby:{}", lobby_id);
    let mut pipe = redis::pipe();
    pipe.atomic()
        .set(format!("user:{}:lobby", request_receiver), &lobby_id)
        .sadd(format!("{}:members", &key_list), request_receiver);

    if let Ok(()) = pipe.query_async(&mut redis_conn).await {
        //Get lobby members set
        if let Ok(mut member_set) = AsyncCommands::smembers::<_, HashSet<String>>(
            &mut redis_conn,
            format!("{}:members", &key_list),
        )
        .await
        {
            for member in member_set.iter() {
                if member == request_receiver {
                    continue;
                }
                let data_to_lobby = json!({
                    "resource": "lobby_invitation",
                    "action": "accept",
                    "payload": {
                        "sender": request_sender,
                        "receiver": request_receiver
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
            if let Ok(lobby_info) = AsyncCommands::get::<_, String>(&mut redis_conn, lobby_id).await
            {
                let lobby_info_json =
                    serde_json::from_str::<serde_json::Value>(&lobby_info).unwrap();
                return (StatusCode::CREATED, lobby_info_json.to_string()).into_response();
            }
        }
    }

    return (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Error finishing the request, please try again !",
    )
        .into_response();
}

pub async fn decline_lobby_invitation(
    State(app_state_): State<AppState>,
    claims: AuthUser,
    query_params: Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if query_params.is_empty() {
        return (StatusCode::BAD_REQUEST, "Params empty !").into_response();
    }

    let mut request_sender = "";
    let request_receiver = &claims.username;
    if let Some(sender_username) = query_params.get("sender") {
        request_sender = &sender_username;
        if !USERNAME_REGEX.is_match(request_sender) {
            return (StatusCode::BAD_REQUEST, "Invalid sender username format !").into_response();
        }
    } else {
        return (StatusCode::BAD_REQUEST, "Missing sender !").into_response();
    }

    if !USERNAME_REGEX.is_match(request_receiver) {
        return (
            StatusCode::BAD_REQUEST,
            "Invalid receiver username format !",
        )
            .into_response();
    }

    let mut redis_conn = app_state_.redis_conn.clone();
    let data_to_sender = json!({
        "resource": "lobby_invitation",
        "action": "decline",
        "payload": {
            "sender": request_sender,
            "receiver": request_receiver
        }
    });
    let pub_sub_data_json = json!({
        "username": request_sender,
        "data": data_to_sender
    });
    if let Ok(()) = AsyncCommands::publish(
        &mut redis_conn,
        "web_socket_events",
        pub_sub_data_json.to_string(),
    )
    .await
    {
        return (StatusCode::CREATED, "Invitation declined !").into_response();
    }

    return (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Error finishing the request, please try again !",
    )
        .into_response();
}

pub async fn leave_lobby(
    State(app_state_): State<AppState>,
    claims: AuthUser,
) -> impl IntoResponse {
    let mut username = &claims.username;

    if !USERNAME_REGEX.is_match(username) {
        return (StatusCode::BAD_REQUEST, "Invalid username format !").into_response();
    }

    let mut redis_conn = app_state_.redis_conn.clone();
    if let Ok(lobby_id) =
        AsyncCommands::get::<_, String>(&mut redis_conn, format!("user:{}:lobby", username)).await
    {
        let key_list = format!("lobby:{}", &lobby_id);
        let mut pipe = redis::pipe();
        //Remove user from current lobby and remove user from lobby's member set
        pipe.atomic()
            .set(format!("user:{}:lobby", &username), "")
            .srem(format!("{}:members", &key_list), &username);

        if let Ok(()) = pipe.query_async(&mut redis_conn).await {
            //Get lobby members set
            if let Ok(member_set) = AsyncCommands::smembers::<_, HashSet<String>>(
                &mut redis_conn,
                format!("{}:members", &key_list),
            )
            .await
            {
                //Get lobby info
                if let Ok(lobby_leader) =
                    AsyncCommands::hget::<_, _, String>(&mut redis_conn, &key_list, "leader").await
                {
                    //If lobby's leader is left user, grant leader lobby to the first member of the lobby set
                    let mut new_leader = &lobby_leader;
                    if &lobby_leader == username {
                        if let Some(first_member) = member_set.iter().next() {
                            new_leader = first_member;
                            let _ = AsyncCommands::hset::<_, _, _, ()>(
                                &mut redis_conn,
                                &key_list,
                                "leader",
                                first_member,
                            )
                            .await;
                        }
                    }
                    for member in member_set.iter() {
                        if member == username {
                            continue;
                        }
                        let mut redis_conn = app_state_.redis_conn.clone();
                        let data_to_lobby = json!({
                            "resource": "lobby",
                            "action": "leave",
                            "payload": {
                                "username": username,
                                "leader": new_leader
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
                    return (StatusCode::CREATED, "Left lobby !").into_response();
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

pub async fn make_leader(
    State(app_state_): State<AppState>,
    claims: AuthUser,
    query_params: Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if query_params.is_empty() {
        return (StatusCode::BAD_REQUEST, "Params empty !").into_response();
    }

    let mut request_sender = &claims.username;
    let mut request_receiver = "";

    if !USERNAME_REGEX.is_match(request_sender) {
        return (StatusCode::BAD_REQUEST, "Invalid sender username format !").into_response();
    }

    if let Some(receiver_username) = query_params.get("receiver") {
        request_receiver = &receiver_username;
        if !USERNAME_REGEX.is_match(request_receiver) {
            return (
                StatusCode::BAD_REQUEST,
                "Invalid receiver username format !",
            )
                .into_response();
        }
    } else {
        return (StatusCode::BAD_REQUEST, "Missing receiver !").into_response();
    }

    let mut redis_conn = app_state_.redis_conn.clone();
    if let Ok(lobby_id) =
        AsyncCommands::get::<_, String>(&mut redis_conn, format!("user:{}:lobby", request_sender))
            .await
    {
        let key_list = format!("lobby:{}", &lobby_id);
        if let Ok(lobby_leader) =
            AsyncCommands::hget::<_, _, String>(&mut redis_conn, &key_list, "leader").await
        {
            if &lobby_leader == request_sender {
                if let Ok(member_set) = AsyncCommands::smembers::<_, HashSet<String>>(
                    &mut redis_conn,
                    format!("{}:members", &key_list),
                )
                .await
                {
                    if !member_set.contains(request_receiver) {
                        return (StatusCode::BAD_REQUEST, "Target doesn't exist in lobby !")
                            .into_response();
                    }
                    if let Ok(()) = AsyncCommands::hset::<_, _, _, ()>(
                        &mut redis_conn,
                        &key_list,
                        "leader",
                        request_receiver,
                    )
                    .await
                    {
                        for member in member_set.iter() {
                            if member == request_sender {
                                continue;
                            }
                            let mut redis_conn = app_state_.redis_conn.clone();
                            let data_to_lobby = json!({
                                "resource": "lobby",
                                "action": "make_leader",
                                "payload": {
                                    "leader": request_receiver
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
                        return (StatusCode::CREATED, "Make leader successfully !").into_response();
                    }
                }
            } else {
                return (
                    StatusCode::UNAUTHORIZED,
                    "No permission to perform the request !",
                )
                    .into_response();
            }
        }
    }
    return (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Error finishing the request, please try again !",
    )
        .into_response();
}

pub async fn kick_member(
    State(app_state_): State<AppState>,
    claims: AuthUser,
    query_params: Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if query_params.is_empty() {
        return (StatusCode::BAD_REQUEST, "Params empty !").into_response();
    }

    let mut request_sender = &claims.username;
    let mut request_receiver = "";

    if !USERNAME_REGEX.is_match(request_sender) {
        return (StatusCode::BAD_REQUEST, "Invalid sender username format !").into_response();
    }

    if let Some(receiver_username) = query_params.get("receiver") {
        request_receiver = &receiver_username;
        if !USERNAME_REGEX.is_match(request_receiver) {
            return (
                StatusCode::BAD_REQUEST,
                "Invalid receiver username format !",
            )
                .into_response();
        }
    } else {
        return (StatusCode::BAD_REQUEST, "Missing receiver !").into_response();
    }

    let mut redis_conn = app_state_.redis_conn.clone();
    if let Ok(lobby_id) =
        AsyncCommands::get::<_, String>(&mut redis_conn, format!("user:{}:lobby", request_sender))
            .await
    {
        let key_list = format!("lobby:{}", &lobby_id);
        if let Ok(lobby_leader) =
            AsyncCommands::hget::<_, _, String>(&mut redis_conn, &key_list, "leader").await
        {
            if &lobby_leader == request_sender {
                if let Ok(member_set) = AsyncCommands::smembers::<_, HashSet<String>>(
                    &mut redis_conn,
                    format!("{}:members", &key_list),
                )
                .await
                {
                    if !member_set.contains(request_receiver) {
                        return (StatusCode::BAD_REQUEST, "Target doesn't exist in lobby !")
                            .into_response();
                    }
                    if let Ok(()) = AsyncCommands::srem::<_, _, ()>(
                        &mut redis_conn,
                        format!("{}:members", &key_list),
                        request_receiver,
                    )
                    .await
                    {
                        for member in member_set.iter() {
                            if member == request_sender {
                                continue;
                            }
                            let mut redis_conn = app_state_.redis_conn.clone();
                            let data_to_lobby = json!({
                                "resource": "lobby",
                                "action": "kick_member",
                                "payload": {
                                    "sender": request_sender,
                                    "receiver": request_receiver
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
                        return (StatusCode::CREATED, "Kick member successfully !").into_response();
                    }
                }
            } else {
                return (
                    StatusCode::UNAUTHORIZED,
                    "No permission to perform the request !",
                )
                    .into_response();
            }
        }
    }
    return (
        StatusCode::INTERNAL_SERVER_ERROR,
        "Error finishing the request, please try again !",
    )
        .into_response();
}
