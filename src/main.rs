use axum::{routing::post, Router};
use std::net::SocketAddr;
use handler::{sign_up_discord, connect_verify, check_target_exist, discord_transaction, get_balance, check_trade_history};
use stock::{get_last_price, buy_stock, sell_stock, check_stock_hold};

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/signup", post(sign_up_discord))
        .route("/get_balance", post(get_balance))
        .route("/get_price", post(get_last_price))
        .route("/dc_trade", post(discord_transaction))
        .route("/connect", post(connect_verify))
        .route("/buy_stock", post(buy_stock))
        .route("/check_stock", post(check_stock_hold))
        .route("/check_trade", post(check_trade_history))
        .route("/sell_stock", post(sell_stock))
        .route("/check_target", post(check_target_exist));
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Server successfully run at {}", addr);
    axum::serve(tokio::net::TcpListener::bind(addr).await.unwrap(), app)
        .await
        .unwrap();
}