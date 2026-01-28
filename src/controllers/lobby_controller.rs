use redis::{AsyncCommands, FromRedisValue, aio::MultiplexedConnection};
use serde_json::json;
use std::collections::{HashMap, HashSet};

use axum::{
    Json,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};

use crate::{auth::AuthUser, models::lobby::LobbyInfo};

use crate::global_vars::USERNAME_REGEX;

use crate::app_state::AppState;

fn create_lobby_info_hash_fields(lobby_info: &LobbyInfo) -> Vec<(&str, String)> {
    return vec![
        ("lobby_name", lobby_info.lobby_name.clone()),
        ("leader", lobby_info.leader.clone()),
        ("limit_num", lobby_info.limit_num.to_string().clone()),
        ("status", lobby_info.status.clone()),
    ];
}

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
    let lobby_info = LobbyInfo::new(
        format!("{}'s lobby", username).as_str(),
        username,
        5,
        "Ready",
    );
    let mut pipe = redis::pipe();
    //lobby:lobby_haha - {name: "", leader: ""}
    //user:haha:lobby - lobby_haha
    //active_lobbies - lobby_haha
    //lobby:lobby_haha:members - haha
    pipe.atomic()
        .hset_multiple(&key_list, &create_lobby_info_hash_fields(&lobby_info))
        .set(format!("user:{}:lobby", username), &lobby_id)
        .sadd("active_lobbies", &lobby_id)
        .sadd(format!("{}:members", &key_list), username);

    if let Ok(()) = pipe.query_async(&mut redis_conn).await {
        return (StatusCode::CREATED, Json(lobby_info)).into_response();
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
    let mut pipe = redis::pipe();

    if let Ok(current_lobby_id) =
        AsyncCommands::get::<_, String>(&mut redis_conn, format!("user:{}:lobby", &request_sender))
            .await
    {
        let key_list = format!("lobby:{}", current_lobby_id);
        if let Ok(mut member_set) = AsyncCommands::smembers::<_, HashSet<String>>(
            &mut redis_conn,
            format!("{}:members", &key_list),
        )
        .await
        {
            if member_set.contains(request_receiver) {
                return (StatusCode::BAD_REQUEST, "Already in lobby !").into_response();
            }
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
        }
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
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "sender": "",
                "message": "Params empty !"
            })),
        )
            .into_response();
    }

    let mut request_sender = "";
    let request_receiver = &claims.username;
    if let Some(sender_username) = query_params.get("sender") {
        request_sender = &sender_username;
        if !USERNAME_REGEX.is_match(request_sender) {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "sender": request_sender,
                    "message": "Invalid sender username format !"
                })),
            )
                .into_response();
        }
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "sender": request_sender,
                "message": "Missing sender !"
            })),
        )
            .into_response();
    }

    if !USERNAME_REGEX.is_match(request_receiver) {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "sender": request_sender,
                "message": "Invalid receiver username format !"
            })),
        )
            .into_response();
    }
    //lobby:lobby_haha - {name: "", leader: ""}
    //user:haha:lobby - lobby_haha
    //active_lobbies - [lobby_haha]
    //lobby:lobby_haha:members - [haha]
    let mut redis_conn = app_state_.redis_conn.clone();
    let mut pipe = redis::pipe();
    if let Ok(target_lobby_id) =
        AsyncCommands::get::<_, String>(&mut redis_conn, format!("user:{}:lobby", &request_sender))
            .await
    {
        let key_list = format!("lobby:{}", target_lobby_id);
        if let Ok(lobby_info) =
            AsyncCommands::hgetall::<_, HashMap<String, redis::Value>>(&mut redis_conn, &key_list)
                .await
        {
            if let Some(lobby_status_redis) = lobby_info.get("status") {
                if let Ok(lobby_status) = String::from_redis_value_ref(lobby_status_redis) {
                    if lobby_status != "Ready" {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(json!({
                                "sender": request_sender,
                                "message": "Lobby busy !"
                            })),
                        )
                            .into_response();
                    }
                }
            }
            //Get lobby members set
            if let Ok(mut member_set) = AsyncCommands::smembers::<_, HashSet<String>>(
                &mut redis_conn,
                format!("{}:members", &key_list),
            )
            .await
            {
                if member_set.len() == 5 {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({
                            "sender": request_sender,
                            "message": "Lobby full !"
                        })),
                    )
                        .into_response();
                }
                //Receiver leave current lobby first then join the new lobby
                leave_lobby_proccess(request_receiver, redis_conn.clone()).await;
                pipe.atomic()
                    .set(format!("user:{}:lobby", request_receiver), &target_lobby_id)
                    .sadd(format!("{}:members", &key_list), request_receiver);
                if let Ok(_) = pipe.query_async::<()>(&mut redis_conn).await {
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
                    member_set.insert(request_receiver.clone());
                    let lobby_info_response = LobbyInfo::new(
                        &String::from_redis_value(lobby_info.get("lobby_name").unwrap().clone())
                            .unwrap(),
                        &String::from_redis_value(lobby_info.get("leader").unwrap().clone())
                            .unwrap(),
                        usize::from_redis_value(lobby_info.get("limit_num").unwrap().clone())
                            .unwrap(),
                        &String::from_redis_value(lobby_info.get("status").unwrap().clone())
                            .unwrap(),
                    );
                    let response = json!({
                        "sender": request_sender,
                        "lobby": {
                            "lobby_name": lobby_info_response.lobby_name,
                            "leader": lobby_info_response.leader,
                            "limit_num": lobby_info_response.limit_num,
                            "status": lobby_info_response.status,
                            "members": member_set
                        },
                    });
                    return (StatusCode::CREATED, response.to_string()).into_response();
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

pub async fn decline_lobby_invitation(
    State(app_state_): State<AppState>,
    claims: AuthUser,
    query_params: Query<HashMap<String, String>>,
) -> impl IntoResponse {
    if query_params.is_empty() {
        return (StatusCode::BAD_REQUEST, "Params empty !").into_response();
    }

    let request_sender: String;
    let request_receiver = &claims.username;
    if let Some(sender_username) = query_params.get("sender") {
        request_sender = sender_username.clone();
        if !USERNAME_REGEX.is_match(&request_sender) {
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
        return (StatusCode::CREATED, request_sender).into_response();
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

    //Receiver leave current lobby first then join self lobby
    leave_lobby_proccess(username, redis_conn.clone()).await;
    let mut pipe = redis::pipe();
    //Set current removed player's lobby to that player's self lobby, remove user from lobby's member set and add removed player's current lobby
    //to active lobbies
    //lobby:lobby_haha - {name: "", leader: ""}
    //user:haha:lobby - lobby_haha
    //active_lobbies - [lobby_haha]
    //lobby:lobby_haha:members - [haha]
    let new_keylist = format!("lobby:{}", format!("lobby_{}", username));
    let lobby_info_response =
        LobbyInfo::new(&format!("{}'s lobby", username), username, 5, "Ready");
    pipe.atomic()
        .hset_multiple(
            &new_keylist,
            &create_lobby_info_hash_fields(&lobby_info_response),
        )
        .set(
            format!("user:{}:lobby", &username),
            format!("lobby_{}", username),
        )
        .sadd("active_lobbies", format!("lobby_{}", username))
        .sadd(format!("{}:members", new_keylist), &username);

    if let Ok(()) = pipe.query_async(&mut redis_conn).await {
        let response = json!({
            "lobby_name": lobby_info_response.lobby_name,
            "leader": lobby_info_response.leader,
            "limit_num": lobby_info_response.limit_num,
            "status": lobby_info_response.status,
            "members": [username]
        });
        return (StatusCode::CREATED, Json(response)).into_response();
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
    let mut request_receiver: String;

    if !USERNAME_REGEX.is_match(request_sender) {
        return (StatusCode::BAD_REQUEST, "Invalid sender username format !").into_response();
    }

    if let Some(receiver_username) = query_params.get("receiver") {
        request_receiver = receiver_username.clone();
        if !USERNAME_REGEX.is_match(&request_receiver) {
            return (
                StatusCode::BAD_REQUEST,
                "Invalid receiver username format !",
            )
                .into_response();
        }
    } else {
        return (StatusCode::BAD_REQUEST, "Missing receiver !").into_response();
    }

    //lobby:lobby_haha - {name: "", leader: ""}
    //user:haha:lobby - lobby_haha
    //active_lobbies - [lobby_haha]
    //lobby:lobby_haha:members - [haha]
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
                    if !member_set.contains(&request_receiver) {
                        return (StatusCode::BAD_REQUEST, "Target doesn't exist in lobby !")
                            .into_response();
                    }
                    let mut pipe = redis::pipe();
                    let new_lobby_id = format!("lobby_{}", request_receiver);
                    let new_key_list = format!("lobby:{}", new_lobby_id);
                    let lobby_info_response = LobbyInfo::new(
                        &format!("{}'s lobby", request_receiver),
                        &request_receiver,
                        5,
                        "Ready",
                    );
                    pipe.atomic()
                        .del(&key_list)
                        .hset_multiple(
                            &new_key_list,
                            &create_lobby_info_hash_fields(&lobby_info_response),
                        )
                        .srem("active_lobbies", &lobby_id)
                        .sadd("active_lobbies", &new_lobby_id)
                        .del(format!("{}:members", &key_list));
                    if let Ok(_) = pipe.query_async::<()>(&mut redis_conn).await {
                        let response = json!({
                            "lobby_name": lobby_info_response.lobby_name,
                            "leader": lobby_info_response.leader,
                            "limit_num": lobby_info_response.limit_num,
                            "status": lobby_info_response.status,
                            "members": member_set
                        });
                        for member in member_set.iter() {
                            pipe.atomic()
                                .set(format!("user:{}:lobby", member), &new_lobby_id)
                                .sadd(format!("{}:members", &new_key_list), &member);

                            if let Ok(_) = pipe.query_async::<()>(&mut redis_conn).await {
                                if member == request_sender {
                                    continue;
                                }
                                let mut redis_conn = app_state_.redis_conn.clone();
                                let data_to_lobby = json!({
                                    "resource": "lobby",
                                    "action": "make_leader",
                                    "payload": {
                                        "lobby": response
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
                        return (StatusCode::CREATED, Json(response)).into_response();
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
    let mut request_receiver: String;

    if !USERNAME_REGEX.is_match(request_sender) {
        return (StatusCode::BAD_REQUEST, "Invalid sender username format !").into_response();
    }

    if let Some(receiver_username) = query_params.get("receiver") {
        request_receiver = receiver_username.clone();
        if !USERNAME_REGEX.is_match(&request_receiver) {
            return (
                StatusCode::BAD_REQUEST,
                "Invalid receiver username format !",
            )
                .into_response();
        }
    } else {
        return (StatusCode::BAD_REQUEST, "Missing receiver !").into_response();
    }

    //lobby:lobby_haha - {name: "", leader: ""}
    //user:haha:lobby - lobby_haha
    //active_lobbies - [lobby_haha]
    //lobby:lobby_haha:members - [haha]
    let mut redis_conn = app_state_.redis_conn.clone();
    let mut pipe = redis::pipe();
    if let Ok(lobby_id) =
        AsyncCommands::get::<_, String>(&mut redis_conn, format!("user:{}:lobby", request_sender))
            .await
    {
        let key_list = format!("lobby:{}", &lobby_id);
        if let Ok(lobby_leader) =
            AsyncCommands::hget::<_, _, String>(&mut redis_conn, &key_list, "leader").await
        {
            if &lobby_leader == request_sender {
                if let Ok(mut member_set) = AsyncCommands::smembers::<_, HashSet<String>>(
                    &mut redis_conn,
                    format!("{}:members", &key_list),
                )
                .await
                {
                    if !member_set.contains(&request_receiver) {
                        return (StatusCode::BAD_REQUEST, "Target doesn't exist in lobby !")
                            .into_response();
                    }

                    let new_keylist_for_removed =
                        format!("lobby:{}", format!("lobby_{}", request_receiver));
                    let lobby_info_response = LobbyInfo::new(
                        &format!("{}'s lobby", request_receiver),
                        &request_receiver,
                        5,
                        "Ready",
                    );
                    pipe.atomic()
                        .srem(format!("{}:members", &key_list), &request_receiver)
                        .hset_multiple(
                            &new_keylist_for_removed,
                            &create_lobby_info_hash_fields(&lobby_info_response),
                        )
                        .set(
                            format!("user:{}:lobby", &request_receiver),
                            format!("lobby_{}", request_receiver),
                        )
                        .sadd("active_lobbies", format!("lobby_{}", request_receiver))
                        .sadd(
                            format!("{}:members", &new_keylist_for_removed),
                            &request_receiver,
                        );
                    if let Ok(_) = pipe.query_async::<()>(&mut redis_conn).await {
                        member_set.remove(&request_receiver);
                        let data_to_removed = json!({
                            "resource": "lobby",
                            "action": "is_kick",
                            "payload": {
                                "lobby": {
                                    "lobby_name": lobby_info_response.lobby_name,
                                    "leader": lobby_info_response.leader,
                                    "limit_num": lobby_info_response.limit_num,
                                    "status": lobby_info_response.status,
                                    "members": [request_receiver]
                                }
                            }
                        });
                        let pub_sub_data_to_removed = json!({
                            "username": request_receiver,
                            "data": data_to_removed
                        });
                        let _ = AsyncCommands::publish::<_, _, ()>(
                            &mut redis_conn,
                            "web_socket_events",
                            pub_sub_data_to_removed.to_string(),
                        )
                        .await;
                        for member in member_set.iter() {
                            if member == request_sender || member == &request_receiver {
                                continue;
                            }
                            let lobby_info_for_member = LobbyInfo::new(
                                &format!("{}'s lobby", request_sender),
                                &request_sender,
                                5,
                                "Ready",
                            );
                            let data_to_lobby = json!({
                                "resource": "lobby",
                                "action": "kick_member",
                                "payload": {
                                    "left_user": request_receiver,
                                    "lobby": {
                                        "lobby_name": lobby_info_for_member.lobby_name,
                                        "leader": lobby_info_for_member.leader,
                                        "limit_num": lobby_info_for_member.limit_num,
                                        "status": lobby_info_for_member.status,
                                        "members": member_set
                                    }
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
                        return (StatusCode::CREATED, request_receiver).into_response();
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

pub async fn leave_lobby_proccess(username: &String, mut redis_conn: MultiplexedConnection) {
    let mut pipe = redis::pipe();
    //lobby:lobby_haha - {name: "", leader: ""}
    //user:haha:lobby - lobby_haha
    //active_lobbies - [lobby_haha]
    //lobby:lobby_haha:members - [haha]
    if let Ok(current_lobby_id) =
        AsyncCommands::get::<_, String>(&mut redis_conn, format!("user:{}:lobby", username)).await
    {
        let current_key_list = format!("lobby:{}", current_lobby_id);
        pipe.atomic()
            //.set(format!("user:{}:lobby", username), "")
            .srem(format!("{}:members", &current_key_list), username);
        if let Ok(_) = pipe.query_async::<()>(&mut redis_conn).await {
            //Get lobby members set
            if let Ok(mut member_set) = AsyncCommands::smembers::<_, HashSet<String>>(
                &mut redis_conn,
                format!("{}:members", &current_key_list),
            )
            .await
            {
                if member_set.len() == 0 {
                    pipe.atomic()
                        .srem("active_lobbies", &current_lobby_id)
                        .del(&current_key_list)
                        .del(format!("user:{}:lobby", username))
                        .del(format!("{}:members", &current_key_list));
                    let _ = pipe.query_async::<()>(&mut redis_conn).await;
                    return;
                }
                pipe.atomic()
                    .hget(&current_key_list, "leader")
                    .get(format!("game_server:{}", current_lobby_id));
                if let Ok((lobby_leader, game_server_info)) =
                    pipe.query_async::<(String, String)>(&mut redis_conn).await
                {
                    //If lobby's leader is left user, grant leader lobby to the first member of the lobby set
                    let mut new_leader = &lobby_leader;
                    if &lobby_leader == username {
                        for member in member_set.iter() {
                            if member != username {
                                new_leader = member;
                                break;
                            }
                        }
                    }
                    let new_lobby_id = format!("lobby_{}", new_leader);
                    let new_key_list = format!("lobby:{}", new_lobby_id);
                    let lobby_info_for_member =
                        LobbyInfo::new(&format!("{}'s lobby", new_leader), &new_leader, 5, "Ready");
                    if new_leader != &lobby_leader {
                        pipe.atomic()
                            .del(&current_key_list)
                            .hset_multiple(
                                &new_key_list,
                                &create_lobby_info_hash_fields(&lobby_info_for_member),
                            )
                            .srem("active_lobbies", &current_lobby_id)
                            .sadd("active_lobbies", &new_lobby_id)
                            .del(format!("{}:members", &current_key_list))
                            .set(format!("user:{}:lobby", username), &new_lobby_id)
                            .set(format!("game_server:{}", new_lobby_id), game_server_info);
                        let _ = pipe.query_async::<()>(&mut redis_conn).await;
                    }
                    for member in member_set.iter() {
                        if new_leader != &lobby_leader {
                            pipe.atomic()
                                .set(format!("user:{}:lobby", member), &new_lobby_id)
                                .sadd(format!("{}:members", &new_key_list), &member);

                            let _ = pipe.query_async::<()>(&mut redis_conn).await;
                        }
                        let data_to_lobby = json!({
                            "resource": "lobby",
                            "action": "leave",
                            "payload": {
                                "left_user": username,
                                "lobby": {
                                    "lobby_id": new_lobby_id,
                                    "lobby_name": lobby_info_for_member.lobby_name,
                                    "leader": lobby_info_for_member.leader,
                                    "limit_num": lobby_info_for_member.limit_num,
                                    "status": lobby_info_for_member.status,
                                    "members": member_set
                                }
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
