use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use axum::{extract::Json, response::IntoResponse};
use axum::http::StatusCode;
use rust_decimal::{Decimal, prelude::FromPrimitive};
use structure::{CardInfo, BuyStock, Symbol, Stock, SellStock, TransactionType};
use function::{get_json_map, write_json_info, check_balance};
use yahoo_finance_api as yahoo;
use std::error::Error as StdError;
use rust_decimal::prelude::ToPrimitive;
use tokio::task;

pub async fn buy_stock(Json(stock): Json<BuyStock>) -> impl IntoResponse {
    let mut card_map: HashMap<u64, CardInfo> = match get_json_map("card.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let data = match get_verified_card(&mut card_map, &stock.card_holder, &stock.target, &stock.token) {
        Ok(d) => d,
        Err(e) => return e.into_response(),
    };

    let price = match get_stock_price(stock.symbol.as_str()).await {
        Ok(p) => p,
        Err(e) => {
            println!("Failed to get price: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get price!").into_response();
        }
    };

    let total_cost = price * Decimal::from(stock.hand) / Decimal::new(stock.leverage.to_i64().unwrap(), 2);

    if !check_balance(&data.balance, total_cost) {
        return (StatusCode::BAD_REQUEST, "Insufficient balance").into_response();
    }

    data.balance -= total_cost;
    let transaction_map = data.transaction.get_or_insert_with(HashMap::new);
    let transaction_type = transaction_map.entry(String::from("Transaction")).or_insert_with(Vec::new);
    transaction_type.push(TransactionType::Debit { amount: total_cost.to_f64().unwrap() });

    let stock_map = data.stock.get_or_insert_with(HashMap::new);
    let buy_stocks = stock_map.entry(String::from("Buy")).or_insert_with(Vec::new);

    buy_stocks.push(Stock {
        symbol: stock.symbol.clone(),
        hand: stock.hand,
        leverage: stock.leverage,
        price,
    });

    if let Err(e) = write_json_info("card.json", &card_map) {
        println!("Error in writing card json: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    format!("Buy {} hands {} leverage {} times, profit {} USD", stock.hand, stock.symbol, stock.leverage, total_cost).into_response()
}

pub async fn sell_stock(Json(stock): Json<SellStock>) -> impl IntoResponse {
    let mut card_map: HashMap<u64, CardInfo> = match get_json_map("card.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let data = match get_verified_card(&mut card_map, &stock.card_holder, &stock.target, &stock.token) {
        Ok(d) => d,
        Err(e) => return e.into_response(),
    };

    let price = match get_stock_price(stock.symbol.as_str()).await {
        Ok(p) => p,
        Err(e) => {
            println!("Failed to get price: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get price!").into_response();
        }
    };

    let stock_map = match &mut data.stock {
        Some(map) => map,
        None => return (StatusCode::BAD_REQUEST, "No stocks held yet").into_response(),
    };

    let buy_vec = match stock_map.get_mut("Buy") {
        Some(vec) => vec,
        None => return (StatusCode::BAD_REQUEST, "No stocks bought yet").into_response(),
    };

    let pos = buy_vec.iter().position(|s| s.symbol == stock.symbol);
    let buy_data = match pos {
        Some(i) => buy_vec.remove(i),
        None => return (StatusCode::BAD_REQUEST, "No stock holdings found").into_response(),
    };

    if buy_data.leverage == Decimal::from(0) {
        return (StatusCode::BAD_REQUEST, "Leverage cannot be zero").into_response();
    }

    let hand = buy_data.hand;
    let leverage = buy_data.leverage;
    let buy_price = buy_data.price;
    let sell_price = price;
    let earning = (sell_price - buy_price) * hand * leverage;
    let principal = buy_price * Decimal::from(hand) / Decimal::new(leverage.to_i64().unwrap(), 2);
    let total_money = principal + earning;

    data.balance += total_money;
    let transaction_map = data.transaction.get_or_insert_with(HashMap::new);
    let transaction_type = transaction_map.entry(String::from("Transaction")).or_insert_with(Vec::new);
    transaction_type.push(TransactionType::Credit { amount: total_money.to_f64().unwrap() });

    let stock_map = data.stock.get_or_insert_with(HashMap::new);
    let sell_stocks = stock_map.entry(String::from("Sold")).or_insert_with(Vec::new);

    sell_stocks.push(Stock {
        symbol: stock.symbol.clone(),
        hand,
        leverage,
        price
    });

    if let Err(e) = write_json_info("card.json", &card_map) {
        println!("Error in writing card json: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    format!("Sold {} hands {} leverage {} times, profit {} USD", hand, stock.symbol, leverage, earning).into_response()
}

pub async fn get_last_price(Json(name): Json<Symbol>) -> impl IntoResponse {
    let symbol = match search_stock_name(name.symbol.as_str()).await {
        Ok(s) => s,
        Err(_) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, "No stock symbol or name found").into_response();
        }
    };
    let price = match get_stock_price(symbol.as_str()).await {
        Ok(price) => price,
        Err(_) => return String::from("Failed to obtain price!").into_response(),
    };
    format!("{} current price is {} USD", symbol, price.round_dp(2)).into_response()
}

pub async fn search_stock_name(name: &str) -> Result<String, Error> {
    let name = name.to_string();
    task::spawn_blocking(move || {
        let provider = yahoo::YahooConnector::new().unwrap();
        let resp = provider.search_ticker(&name).unwrap();
        match resp.quotes.get(0) {
            Some(quote) => Ok(quote.symbol.clone()),
            None => Err(Error::new(ErrorKind::NotFound, "No stock symbol found")),
        }
    })
        .await?
}

pub async fn get_stock_price(name: &str) -> Result<Decimal, Box<dyn StdError + Send + Sync>> {
    let symbol = match search_stock_name(name).await {
        Ok(s) => s,
        Err(_) => {
           return Err(Box::new(Error::new(ErrorKind::Other, "No stock symbol or name found")));
        }
    };
    task::spawn_blocking(move || {
        let provider = yahoo::YahooConnector::new()?;
        let response = provider.get_latest_quotes(&symbol, "1d")?;
        let quote = response.last_quote()?;
        let price_f64 = quote.close;
        let price = Decimal::from_f64(price_f64)
            .ok_or_else(|| Box::new(Error::new(ErrorKind::Other, format!("Failed to transform {} to Decimal", price_f64))))?;
        Ok(price.round_dp(2))
    })
        .await?
}

pub fn get_verified_card<'a>(
    card_map: &'a mut HashMap<u64, CardInfo>,
    card_holder: &str,
    target: &str,
    token: &str,
) -> Result<&'a mut CardInfo, (StatusCode, &'static str)> {
    let data = card_map.values_mut().find(|data| data.card_holder == card_holder)
        .ok_or((StatusCode::BAD_REQUEST, "No card holder found"))?;

    let connection_map = data.connection.as_ref()
        .ok_or((StatusCode::BAD_REQUEST, "This card is not connected to any platform"))?;

    let stored_token_vec = connection_map.get(target)
        .ok_or((StatusCode::BAD_REQUEST, "No platform connection record found"))?;

    let matched = stored_token_vec.iter().any(|t| t.target == target && t.token == token);
    if !matched {
        return Err((StatusCode::BAD_REQUEST, "Failed to verify"));
    }

    Ok(data)
}