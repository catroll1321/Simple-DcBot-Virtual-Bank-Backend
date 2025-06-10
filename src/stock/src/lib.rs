use std::collections::HashMap;
use std::io::{Error, ErrorKind};
use axum::{extract::Json, response::IntoResponse};
use axum::http::StatusCode;
use rust_decimal::{Decimal, prelude::FromPrimitive};
use structure::{CardInfo, BuyStock, Symbol, Stock, SellStock, TradeHistory, TransactionType, StockHold, Identification, StockHistory};
use function::{check_balance, write_json_to_file, get_map};
use yahoo_finance_api as yahoo;
use std::error::Error as StdError;
use std::time::{SystemTime, UNIX_EPOCH};
use rust_decimal::prelude::ToPrimitive;
use serde_json::json;
use tokio::task;
use yahoo_finance_api::Quote;

pub async fn buy_stock(Json(stock): Json<BuyStock>) -> impl IntoResponse {
    let mut card_map: HashMap<u64, CardInfo> = match get_map("account.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let mut stock_map: HashMap<String, Vec<StockHold>> = match get_map("stockhold.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Server error, please call admin fixing!").into_response();
        }
    };

    let mut trade_map: HashMap<i64, TradeHistory> = match get_map("trade.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Server error, please call admin fixing!").into_response();
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

    let last_trade: i64;

    if trade_map.is_empty() {
        last_trade = 0;
    } else {
        let Some(last) = trade_map.keys().max() else { todo!() };
        last_trade = *last;
    };

    let total_cost = price * Decimal::from(stock.hand) / Decimal::new(stock.leverage.to_i64().unwrap(), 2);

    if !check_balance(&data.balance, total_cost) {
        return (StatusCode::BAD_REQUEST, "Insufficient balance").into_response();
    }

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    data.balance -= total_cost;
    let transaction_map = data.transaction.get_or_insert_with(HashMap::new);
    transaction_map.insert(now, last_trade + 1);

    if let Err(e) = write_json_to_file("account.json", &card_map) {
        println!("Error in writing card json: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let stock_info = stock_map.entry(stock.card_holder.clone()).or_insert_with(Vec::new);
    stock_info.push(StockHold {
        timestamp: now,
        stock: Stock {
            buy_type: stock.buy_type,
            symbol: stock.symbol.clone(),
            hand: stock.hand,
            leverage: stock.leverage,
            price,
        },
    });

    if let Err(e) = write_json_to_file("stockhold.json", &stock_map) {
        println!("Error in writing trade json: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Server error, please call admin fixing!").into_response();
    }

    let trade_info = TradeHistory {
        timestamp: now,
        transaction_type: TransactionType::Debit { amount: total_cost.to_f64().unwrap() },
        target_user: String::from("Stock! Bot"),
    };

    trade_map.insert(last_trade + 1, trade_info);

    if let Err(e) = write_json_to_file("trade.json", &trade_map) {
        println!("Error in writing trade json: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Server error, please call admin fixing!").into_response();
    }

    (StatusCode::OK, Json(json!({
        "symbol": stock.symbol,
        "hand": stock.hand,
        "leverage": stock.leverage,
        "cost": total_cost
    }))).into_response()
}

pub async fn sell_stock(Json(stock): Json<SellStock>) -> impl IntoResponse {
    let mut card_map: HashMap<u64, CardInfo> = match get_map("account.json") {
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

    let mut stock_map: HashMap<String, Vec<StockHold>> = match get_map("stockhold.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Server error, please call admin fixing!").into_response();
        }
    };

    let mut trade_map: HashMap<i64, TradeHistory> = match get_map("trade.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Server error, please call admin fixing!").into_response();
        }
    };

    let last_trade: i64;

    if trade_map.is_empty() {
        last_trade = 0;
    } else {
        let Some(last) = trade_map.keys().max() else { todo!() };
        last_trade = *last;
    };

    let buy_vec = match stock_map.get_mut(&stock.card_holder) {
        Some(vec) => vec,
        None => return (StatusCode::BAD_REQUEST, "No stocks bought yet").into_response(),
    };

    let pos = buy_vec.iter().position(|s| s.timestamp == stock.timestamp && s.stock.symbol == stock.symbol);

    let buy_data = match pos {
        Some(i) => buy_vec.remove(i),
        None => return (StatusCode::BAD_REQUEST, "No stock holdings found").into_response(),
    };

    let hand = buy_data.stock.hand;
    let leverage = buy_data.stock.leverage;
    let buy_price = buy_data.stock.price;
    let buy_type = buy_data.stock.buy_type.as_str();
    let sell_price = price;
    let earning: Decimal;

    if buy_type == "Long" {
        earning = (sell_price - buy_price) * hand * leverage;
    } else if buy_type == "Short" {
        earning = (buy_price - sell_price) * hand * leverage;
    } else {
        return (StatusCode::BAD_REQUEST, "Wrong buy type").into_response();
    }

    let principal = buy_price * Decimal::from(hand) / Decimal::new(leverage.to_i64().unwrap(), 2);
    let total_money = principal + earning;

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    data.balance += total_money;
    let transaction_map = data.transaction.get_or_insert_with(HashMap::new);
    transaction_map.insert(now, last_trade + 1);

    if let Err(e) = write_json_to_file("stockhold.json", &stock_map) {
        println!("Error in writing trade json: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Server error, please call admin fixing!").into_response();
    }

    if let Err(e) = write_json_to_file("account.json", &card_map) {
        println!("Error in writing card json: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    let trade_info = TradeHistory {
        timestamp: now,
        transaction_type: TransactionType::Credit { amount: total_money.to_f64().unwrap() },
        target_user: String::from("Stock! Bot"),
    };

    trade_map.insert(last_trade + 1, trade_info);
    if let Err(e) = write_json_to_file("trade.json", &trade_map) {
        println!("Error in writing trade json: {}", e);
        return (StatusCode::INTERNAL_SERVER_ERROR, "Server error, please call admin fixing!").into_response();
    }

    (StatusCode::OK, Json(json!({
        "symbol": stock.symbol,
        "hand": hand,
        "leverage": leverage,
        "earning": earning
    }))).into_response()
}

pub async fn check_stock_hold(Json(id): Json<Identification>) -> impl IntoResponse {
    let stock_map: HashMap<String, Vec<StockHold>> = match get_map("stockhold.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Server error, please call admin fixing!").into_response();
        }
    };

    let result = match stock_map.get(id.card_holder.as_str()) {
        Some(holds) => holds,
        None => return (StatusCode::INTERNAL_SERVER_ERROR, "You currently do not hold any stocks!").into_response()
    };

    (StatusCode::OK, Json(json!(result))).into_response()
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

    (StatusCode::OK, Json(json!({ "symbol": symbol, "price": price.round_dp(2) }))).into_response()
}


//stock functions
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

pub async fn fetch_stock_history(name: &str, period: String, interval: String) -> Result<Vec<Quote>, Box<dyn StdError + Send + Sync>> {
    let symbol = match search_stock_name(name).await {
        Ok(s) => s,
        Err(_) => {
            return Err(Box::new(Error::new(ErrorKind::Other, "No stock symbol or name found")));
        }
    };
    let period = period.to_string();
    let interval = interval.to_string();

    task::spawn_blocking(move || {
        let provider = yahoo::YahooConnector::new()?;
        let response = provider.get_quote_range(&symbol, &interval, &period)?;
        let quotes = response.quotes()?;
        Ok(quotes)
    }).await?
}

pub async fn get_stock_history(Json(history): Json<StockHistory>) -> impl IntoResponse {
    let quotes = match fetch_stock_history(history.symbol.as_str(), history.period, history.interval).await {
        Ok(quotes) => quotes,
        Err(_) => return String::from("Failed to obtain stock history!").into_response(),
    };
    (StatusCode::OK, Json(json!(quotes))).into_response()
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
