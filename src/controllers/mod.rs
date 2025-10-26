mod user_controller;
mod web_socket_controller;
pub mod controllers_center {
    use axum::Router;
    use sqlx::PgPool;

    use crate::controllers::user_controller;
    use crate::controllers::web_socket_controller;

    pub fn create_app_router() -> Router<PgPool> {
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
                "/user/friendlist/get",
                axum::routing::get(user_controller::get_friendlist),
            )
            .route(
                "/user/friendlist/add",
                axum::routing::post(user_controller::add_friend),
            )
            .route(
                "/user/friendlist/remove",
                axum::routing::post(user_controller::remove_friend),
            )
            .route(
                "/ws",
                axum::routing::get(web_socket_controller::handle_web_socket_request),
            )
    }
}
