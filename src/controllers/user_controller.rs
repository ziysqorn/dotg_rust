use jsonwebtoken::{EncodingKey, Header, encode};
use redis::AsyncCommands;
use serde_json::{Map, Value, json};
use std::{
    collections::HashMap,
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
    global_vars::USERNAME_REGEX,
    models::user::User,
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
    State(connection_pool): State<PgPool>,
    Json(payload): Json<HashMap<String, String>>,
) -> impl IntoResponse {
    if !payload.is_empty() {
        if let (Some(in_username), Some(in_password)) =
            (payload.get("username"), payload.get("user_password"))
        {
            if let Ok(username_regex) = Regex::new("^[a-zA-Z0-9@]{1,12}$")
                && !username_regex.is_match(in_username)
            {
                return (StatusCode::BAD_REQUEST, "Invalid username format !").into_response();
            }
            if let Ok(password_regex) = Regex::new("^[a-zA-Z0-9*@]{1,12}$")
                && !password_regex.is_match(in_password)
            {
                return (StatusCode::BAD_REQUEST, "Invalid password format !").into_response();
            }
            let login_user = User::new(in_username, in_password, &true);
            if let Ok(found_user) = sqlx::query("Select username from users where username = $1")
                .bind(login_user.get_username())
                .fetch_one(&connection_pool)
                .await
            {
                println!("{:?}", found_user);
                let result = sqlx::query(
                    "Select username, status from users where username = $1 and user_password = $2",
                )
                .bind(login_user.get_username())
                .bind(login_user.get_password())
                .fetch_one(&connection_pool)
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
                        if let Err(update_status_error) =
                            sqlx::query("Update users set status = true where username = $1")
                                .bind(login_user.get_username())
                                .execute(&connection_pool)
                                .await
                        {
                            println!("{:?}", update_status_error);
                            return (StatusCode::UNAUTHORIZED, "Error logging in").into_response();
                        }
                        //Create JWT------------------------------------------------------------------//
                        let expiration = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs() as usize
                            + (24 * 3600);

                        let claims = Claims {
                            subject: in_username.clone(),
                            exp: expiration,
                        };

                        if let Ok(token) = encode(
                            &Header::default(),
                            &claims,
                            &EncodingKey::from_secret(get_jwt_secret()),
                        ) {
                            return (
                                StatusCode::OK,
                                Json(json!({
                                    "token": token,
                                    "username": found_row.get::<String, _>("username"),
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
        } else {
            return (StatusCode::BAD_REQUEST, "Missing login information !").into_response();
        }
    } else {
        (StatusCode::BAD_REQUEST, "Login information empty !").into_response()
    }
}

pub async fn logout(State(app_state_): State<AppState>, auth_user: AuthUser) -> impl IntoResponse {
    if !USERNAME_REGEX.is_match(&auth_user.username) {
        return (StatusCode::BAD_REQUEST, "Invalid username format !").into_response();
    }
    if let Err(update_status_error) =
        sqlx::query("Update users set status = false where username = $1")
            .bind(&auth_user.username)
            .execute(&app_state_.connection_pool)
            .await
    {
        println!("{:?}", update_status_error);
        return (StatusCode::CONFLICT, "Error logging out !").into_response();
    }

    {
        let mut clients_map = app_state_.clients_map.write().await;
        clients_map.remove(&auth_user.username);
    }

    (StatusCode::OK, "Logout successful").into_response()
}
