use axum::{routing::post, Router};
use std::net::SocketAddr;
use handler::{sign_up_discord, connect_verify, check_target_exist, discord_transaction, get_balance};
use stock::{get_last_price, buy_stock, sell_stock};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/signup", post(sign_up_discord))
        .route("/get_balance", post(get_balance))
        .route("/get_price", post(get_last_price))
        .route("/dc_trade", post(discord_transaction))
        .route("/connect", post(connect_verify))
        .route("/buy_stock", post(buy_stock))
        .route("/sell_stock", post(sell_stock))
        .route("/check_target", post(check_target_exist));
    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    println!("Server succeed running on {}", addr);
    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}