use serde_json::json;
use std::collections::HashMap;

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use regex::Regex;
use sqlx::{postgres::PgRow, PgPool, Row};

use crate::models::user::{Friends, User};

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
                        let online_status= found_row.get::<bool, _>("status");
                        if online_status {
                            return (StatusCode::UNAUTHORIZED, "User has already logged in another device").into_response();
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
                        return (
                            StatusCode::OK,
                            Json(json!({
                                "username": found_row.get::<String, _>("username"),
                                "status": true
                            })),
                        )
                            .into_response();
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

pub async fn logout(
    State(connection_pool): State<PgPool>,
    payload: Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if !payload.is_empty() {
        if let Some(in_username) = payload.get("username") {
            if let Ok(username_regex) = Regex::new("^[a-zA-Z0-9@]{1,12}$")
                && !username_regex.is_match(in_username)
            {
                return (StatusCode::BAD_REQUEST, "Invalid username format !").into_response();
            }
            if let Err(update_status_error) =
                sqlx::query("Update users set status = false where username = $1")
                    .bind(&in_username)
                    .execute(&connection_pool)
                    .await
            {
                println!("{:?}", update_status_error);
                return (StatusCode::CONFLICT, "Error logging out !").into_response();
            }
        }
    } else {
        return (StatusCode::BAD_REQUEST, "User information empty !").into_response();
    }
    (StatusCode::OK, "Logout successful").into_response()
}

pub async fn get_friendlist(
    State(connection_pool): State<PgPool>,
    query_map: Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if !query_map.is_empty() {
        if let Some(username) = query_map.get("username") {
            if let Ok(username_regex) = Regex::new("^[a-zA-Z0-9@]{1,12}$")
                && !username_regex.is_match(username)
            {
                return (StatusCode::BAD_REQUEST, "Invalid username format !").into_response();
            }
            let mut result_friendlist: Vec<PgRow> = Vec::new();
            if let Ok(mut friend_list1) = sqlx::query(
                "Select u.username, u.status from users u inner join friends f on u.username = f.player2 where f.player1 = $1"
            )
            .bind(&username)
            .fetch_all(&connection_pool)
            .await
            {
                result_friendlist.append(&mut friend_list1);
            }
            if let Ok(mut friend_list2) = sqlx::query(
                "Select u.username, u.status from users u inner join friends f on u.username = f.player1 where f.player2 = $1"
            )
            .bind(&username)
            .fetch_all(&connection_pool)
            .await
            {
                result_friendlist.append(&mut friend_list2);
            }
            if result_friendlist.is_empty(){
                (StatusCode::NOT_FOUND, "Friendlist Empty !").into_response()
            }
            else{
                let final_friendlist: Vec<serde_json::Value> = result_friendlist.iter().map(|row| json!({"username": row.get::<String, _>("username"), "status": row.get::<bool, _>("status")})).collect();
                (StatusCode::OK, Json(final_friendlist)).into_response()
            }
        } else {
            (StatusCode::BAD_REQUEST, "Missing username !").into_response()
        }
    } else {
        (StatusCode::BAD_REQUEST, "User information empty !").into_response()
    }
}

pub async fn add_friend(
    State(connection_pool): State<PgPool>,
    query_map: Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if !query_map.is_empty() {
        if let (Some(player1_username), Some(player2_username)) =
            (query_map.get("player1"), query_map.get("player2"))
        {
            if let Ok(username_regex) = Regex::new("^[a-zA-Z0-9@]{1,12}$") {
                if !username_regex.is_match(player1_username) {
                    return (StatusCode::BAD_REQUEST, "Invalid player1 username format !")
                        .into_response();
                }
                if !username_regex.is_match(player2_username) {
                    return (StatusCode::BAD_REQUEST, "Invalid player2 username format !")
                        .into_response();
                }
            }
            let friend_relationship = Friends::new(player1_username, player2_username);
            if let Ok(found_relationship) = 
                sqlx::query_as::<_, Friends>("Select player1, player2 from friends where player1 = $1 and player2 = $2 or player1 = $2 and player2 = $1")
                    .bind(friend_relationship.get_player1())
                    .bind(friend_relationship.get_player2())
                    .fetch_all(&connection_pool).await{
                        if !found_relationship.is_empty(){
                            return (StatusCode::BAD_REQUEST, "Already friends !").into_response();
                        }
                    }
            if let Ok(result) =
                sqlx::query("Insert into friends (player1, player2) values ($1, $2)")
                    .bind(friend_relationship.get_player1())
                    .bind(friend_relationship.get_player2())
                    .execute(&connection_pool)
                    .await
            {
                print!("{:?}", result);
                (StatusCode::CREATED, "Adding friend successfully !").into_response()
            } else {
                (StatusCode::CONFLICT, "Error finishing the proccess !").into_response()
            }
        } else {
            (StatusCode::BAD_REQUEST, "Missing players' username !").into_response()
        }
    } else {
        (StatusCode::BAD_REQUEST, "Information empty !").into_response()
    }
}

pub async fn remove_friend(
    State(connection_pool): State<PgPool>,
    query_map: Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if !query_map.is_empty() {
        if let (Some(player1_username), Some(player2_username)) =
            (query_map.get("player1"), query_map.get("player2"))
        {
            if let Ok(username_regex) = Regex::new("^[a-zA-Z0-9@]{1,12}$") {
                if !username_regex.is_match(player1_username) {
                    return (StatusCode::BAD_REQUEST, "Invalid player1 username format !")
                        .into_response();
                }
                if !username_regex.is_match(player2_username) {
                    return (StatusCode::BAD_REQUEST, "Invalid player2 username format !")
                        .into_response();
                }
            }
            let friend_relationship = Friends::new(player1_username, player2_username);
            if let Ok(result) =
                sqlx::query("Delete from friends where player1 = $1 and player2 = $2 or player1 = $2 and player2 = $1")
                    .bind(friend_relationship.get_player1())
                    .bind(friend_relationship.get_player2())
                    .execute(&connection_pool)
                    .await
            {
                if result.rows_affected() > 0 {
                    (StatusCode::OK, "Removing friend successfully !").into_response()
                }
                else{
                    (StatusCode::NOT_MODIFIED, "No relationship to remove !").into_response()
                }
            } else {
                (StatusCode::CONFLICT, "Error finishing the proccess !").into_response()
            }
        } else {
            (StatusCode::BAD_REQUEST, "Missing players' username !").into_response()
        }
    } else {
        (StatusCode::BAD_REQUEST, "Information empty !").into_response()
    }
}
