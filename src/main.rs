use axum;
mod controllers;
mod models;
use controllers::controllers_center;
use dotenvy::dotenv;
use sqlx::PgPool;

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    dotenv().expect("Error loading .env file");

    let connection_str = std::env::var("DATABASE_URL").expect("DATABASE_URL not found !");
    let connection_pool = PgPool::connect(connection_str.as_str()).await?;
    let app_routers = controllers_center::create_app_router().with_state(connection_pool);
    // run our app with hyper, listening globally on port 3000
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app_routers).await?;
    Ok(())
}
