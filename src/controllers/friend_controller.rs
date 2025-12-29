use redis::AsyncCommands;
use serde_json::{Map, Value, json};
use std::collections::HashMap;

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use sqlx::{PgPool, Row, postgres::PgRow};

use crate::{
    auth::AuthUser,
    models::friend::{FriendRequest, Friends},
};

use crate::app_state::AppState;
use crate::global_vars::USERNAME_REGEX;

pub async fn get_friend_request(
    State(app_state_): State<AppState>,
    claims: AuthUser,
) -> impl IntoResponse {
    let username = &claims.username;

    if !USERNAME_REGEX.is_match(username) {
        return (StatusCode::BAD_REQUEST, "Invalid sender username format !").into_response();
    }

    if let Ok(request_list) = sqlx::query_as::<_, FriendRequest>(
        "Select sender, receiver from FriendRequest where receiver = $1",
    )
    .bind(username)
    .fetch_all(&app_state_.connection_pool)
    .await
    {
        return (StatusCode::OK, Json(request_list)).into_response();
    }
    return (
        StatusCode::BAD_GATEWAY,
        "Error finishing the request, please try again !",
    )
        .into_response();
}
pub async fn send_friend_request(
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

    if let Some(_) = sqlx::query(
        "Select sender, receiver from FriendRequests where sender = $1 and receiver = $2",
    )
    .bind(request_sender)
    .bind(request_receiver)
    .fetch_optional(&app_state_.connection_pool)
    .await
    .expect("Error executing query")
    {
        return (StatusCode::BAD_REQUEST, "Request has already been sent !").into_response();
    }

    if let Ok(affected_rows) =
        sqlx::query("Insert into FriendRequests (sender, receiver) values ($1, $2)")
            .bind(request_sender)
            .bind(request_receiver)
            .execute(&app_state_.connection_pool)
            .await
    {
        let mut redis_conn = app_state_.redis_conn.clone();
        let data_to_receiver = json!({
            "resource": "friend_request",
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
            return (StatusCode::CREATED, "Request sent successfully !").into_response();
        }
    }
    return (
        StatusCode::BAD_GATEWAY,
        "Error finishing the request, please try again !",
    )
        .into_response();
}

pub async fn accept_friend_request(
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

    if let Ok(mut transaction) = app_state_.connection_pool.begin().await {
        if let Err(err) = sqlx::query("Insert into friends (player1, player2) values ($1, $2)")
            .bind(request_sender)
            .bind(request_receiver)
            .execute(&mut *transaction)
            .await
        {
            return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
        }
        if let Err(err) =
            sqlx::query("Delete from FriendRequest where sender = $1 and receiver = $2")
                .bind(request_sender)
                .bind(request_receiver)
                .execute(&mut *transaction)
                .await
        {
            return (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()).into_response();
        }
        if let Ok(_) = transaction.commit().await {
            let mut redis_conn = app_state_.redis_conn.clone();
            let data_to_sender = json!({
                "resource": "friend_request",
                "action": "accept",
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
                return (StatusCode::CREATED, "Request accepted !").into_response();
            }
        }
    }

    return (
        StatusCode::BAD_GATEWAY,
        "Error finishing the request, please try again !",
    )
        .into_response();
}

pub async fn decline_friend_request(
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

    if let Ok(affected_rows) =
        sqlx::query("Delete from FriendRequest where sender = $1 and receiver = $2")
            .bind(request_sender)
            .bind(request_receiver)
            .execute(&app_state_.connection_pool)
            .await
    {
        let mut redis_conn = app_state_.redis_conn.clone();
        let data_to_sender = json!({
            "resource": "friend_request",
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
            return (StatusCode::CREATED, "Request declined !").into_response();
        }
    }

    return (
        StatusCode::BAD_GATEWAY,
        "Error finishing the request, please try again !",
    )
        .into_response();
}

pub async fn get_friendlist(
    State(app_state_): State<AppState>,
    claims: AuthUser,
) -> impl IntoResponse {
    let username = &claims.username;

    if !USERNAME_REGEX.is_match(username) {
        return (StatusCode::BAD_REQUEST, "Invalid sender username format !").into_response();
    }

    let mut result_friendlist: Vec<PgRow> = Vec::new();
    if let Ok(mut friend_list1) = sqlx::query(
                "Select u.username, u.status from users u inner join friends f on u.username = f.player2 where f.player1 = $1"
            )
            .bind(username)
            .fetch_all(&app_state_.connection_pool)
            .await
            {
                result_friendlist.append(&mut friend_list1);
            }
    if let Ok(mut friend_list2) = sqlx::query(
                "Select u.username, u.status from users u inner join friends f on u.username = f.player1 where f.player2 = $1"
            )
            .bind(username)
            .fetch_all(&app_state_.connection_pool)
            .await
            {
                result_friendlist.append(&mut friend_list2);
            }

    if result_friendlist.is_empty() {
        return (StatusCode::NOT_FOUND, "Friendlist Empty !").into_response();
    } else {
        let final_friendlist: Vec<serde_json::Value> = result_friendlist.iter().map(|row| json!({"username": row.get::<String, _>("username"), "status": row.get::<bool, _>("status")})).collect();
        return (StatusCode::OK, Json(final_friendlist)).into_response();
    }
}
