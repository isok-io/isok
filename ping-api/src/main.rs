pub mod routes;
pub mod auth;
pub mod users;
pub mod checks;

#[tokio::main]
async fn main() {
    let app = routes::app();

    let listener =
        tokio::net::TcpListener::bind("localhost:8080").await.expect("Unable to bind");

    axum::serve(listener, app).await.unwrap()
}
