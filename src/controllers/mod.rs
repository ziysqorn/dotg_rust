mod friend_controller;
mod game_server_controller;
mod in_game_controller;
mod lobby_controller;
mod user_controller;
mod web_socket_controller;
pub mod controllers_center {
    use axum::Router;

    use crate::app_state::AppState;
    use crate::controllers::friend_controller;
    use crate::controllers::game_server_controller;
    use crate::controllers::in_game_controller;
    use crate::controllers::lobby_controller;
    use crate::controllers::user_controller;
    use crate::controllers::web_socket_controller;

    pub fn create_app_router() -> Router<AppState> {
        Router::new()
            .route(
                "/HPD",
                axum::routing::get(|| async { "Hello HỘI PHONG ĐỘ" }),
            )
            .route(
                "/user/create",
                axum::routing::post(user_controller::create_user),
            )
            .route("/user/login", axum::routing::post(user_controller::login))
            .route("/user/logout", axum::routing::post(user_controller::logout))
            .route(
                "/friendlist/get",
                axum::routing::get(friend_controller::get_friendlist),
            )
            .route(
                "/friend_request/get",
                axum::routing::get(friend_controller::get_friend_request),
            )
            .route(
                "/friend_request/send",
                axum::routing::post(friend_controller::send_friend_request),
            )
            .route(
                "/friend_request/accept",
                axum::routing::post(friend_controller::accept_friend_request),
            )
            .route(
                "/friend_request/decline",
                axum::routing::post(friend_controller::decline_friend_request),
            )
            .route(
                "/friend/remove",
                axum::routing::post(friend_controller::remove_friend),
            )
            .route(
                "/lobby/create",
                axum::routing::post(lobby_controller::create_lobby),
            )
            .route(
                "/lobby/invite",
                axum::routing::post(lobby_controller::invite_to_lobby),
            )
            .route(
                "/lobby/make_leader",
                axum::routing::post(lobby_controller::make_leader),
            )
            .route(
                "/lobby/accept",
                axum::routing::post(lobby_controller::accept_lobby_invitation),
            )
            .route(
                "/lobby/decline",
                axum::routing::post(lobby_controller::decline_lobby_invitation),
            )
            .route(
                "/lobby/leave",
                axum::routing::post(lobby_controller::leave_lobby),
            )
            .route(
                "/lobby/kick",
                axum::routing::post(lobby_controller::kick_member),
            )
            .route(
                "/game_server/create",
                axum::routing::post(game_server_controller::create_game_server),
            )
            .route(
                "/game_server/drop",
                axum::routing::post(game_server_controller::drop_game_server),
            )
            .route(
                "/in_game/character_stats/get",
                axum::routing::get(in_game_controller::get_character_stats),
            )
            .route(
                "/in_game/character_stats/save",
                axum::routing::post(in_game_controller::save_character_stats),
            )
            .route(
                "/in_game/character_stats/remove",
                axum::routing::post(in_game_controller::remove_character_stats),
            )
            .route(
                "/ws",
                axum::routing::get(web_socket_controller::handle_web_socket_request),
            )
    }
}
