use std::collections::HashMap;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use axum::{extract::Json, response::IntoResponse, http::StatusCode};
use serde_json::json;
use structure::{Identification, CardInfo, TargetVerify, TargetInfo, DiscordTrade, TradeHistory};
use function::{get_card_map, write_card_info, generate_token, gen_card, gen_card_num, hash_str_to_u64, handler_transaction, get_trade_map, get_day_end};

pub async fn sign_up_discord(Json(id): Json<Identification>) -> impl IntoResponse {
    // Generate account info
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let hash_id = hash_str_to_u64(&id.card_holder);
    let mixture = now + hash_id;
    let card_number = gen_card_num(mixture);
    let card_account = gen_card(mixture, &id.card_holder);
    let good_thru = &card_account.good_thru.clone();
    let verify_number = &card_account.verify_number.clone();

    let mut all_data: HashMap<u64, CardInfo> = match get_card_map("account.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if all_data.contains_key(&hash_id) {
        return (StatusCode::BAD_REQUEST, "You have already registered").into_response();
    }

    all_data.insert(hash_id, card_account);

    if let Err(e) = write_card_info("account.json", &all_data) {
        println!("Error in writing card json: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    format!("Register Successfully! Card Number: {}, Good Thru: {}, Verify Number: {}", card_number, good_thru, verify_number).into_response()
}

pub async fn discord_transaction(Json(id): Json<DiscordTrade>) -> impl IntoResponse {
    let mut card_map: HashMap<u64, CardInfo> = match get_card_map("account.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let result = match handler_transaction(id, &mut card_map) {
        Ok(message) => message,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response(),
    };

    (StatusCode::OK, result).into_response()
}

pub async fn connect_verify(Json(target): Json<TargetVerify>) -> impl IntoResponse {
    //I will make a key system later?
    let connect_key: String = String::from("connection_key");
    let mut card_map: HashMap<u64, CardInfo> = match get_card_map("account.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let Some(card) = card_map.values_mut().find(|data| data.card_holder == target.card_holder) else {
        return (StatusCode::BAD_REQUEST, "No card found!").into_response();
    };

    let token = generate_token(
        &connect_key,
        &card.card_number.to_string(),
        &card.good_thru.to_string(),
        &card.verify_number.to_string(),
    );

    let connection_map = card.connection.get_or_insert_with(HashMap::new);
    let connections = connection_map.entry(target.target.clone()).or_insert_with(Vec::new);

    if let Some(existing) = connections.iter().find(|info| info.target == target.target) {
        return Json(json!({
        "status": "exists",
        "token": existing.token
    })).into_response();
    }

    connections.push(TargetInfo { target: target.target.clone(), token: token.clone()});

    if let Err(e) = write_card_info("account.json", &card_map) {
        println!("Error in writing card json: {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    Json(json!({
        "status": "ok",
        "token": token
    })).into_response()
}

pub async fn check_trade_history(Json(id): Json<Identification>) -> impl IntoResponse {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    let day_end = get_day_end(now);

    let card_map: HashMap<u64, CardInfo> = match get_card_map("account.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return String::from("Server error, please call admin fixing!").into_response();
        }
    };

    let data = match card_map.values().find(|data| data.card_holder == id.card_holder) {
        Some(card) => card,
        None => return String::from("No card found!").into_response(),
    };

    let values: Vec<_> = match &data.transaction {
        Some(map) => map.iter()
            .filter(|&(&k, _)| k > day_end - 7 * 86400)
            .map(|(_, v)| *v)
            .collect(),
        None => return ("You didn't have any trade!".to_string()).into_response(),
    };

    let value_set: HashSet<i64> = values.into_iter().collect();

    let trade_map: HashMap<i64, TradeHistory> = match get_trade_map("trade.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return String::from("Server error, please call admin fixing!").into_response();
        }
    };

    let trades: HashMap<i64, TradeHistory> = trade_map.iter()
        .filter(|&(&k, _)| value_set.contains(&k))
        .map(|(&k, v)| (k, v.clone()))
        .collect();

    Json(json!(trades)).into_response()
}

pub async fn check_target_exist(Json(id): Json<Identification>) -> impl IntoResponse {
    let card_map: HashMap<u64, CardInfo> = match get_card_map("account.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return Json(json!({ "status": "error", "reason": "not found" }))
        }
    };

    if card_map.values().any(|data| data.card_holder == id.card_holder) {
        Json(json!({ "status": "ok" }))
    } else {
        Json(json!({ "status": "error", "reason": "not found" }))
    }
}

pub async fn get_balance(Json(id): Json<Identification>) -> impl IntoResponse {
    let mut card_map: HashMap<u64, CardInfo> = match get_card_map("account.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let data = match card_map.values_mut().find(|data| data.card_holder == id.card_holder) {
        Some(card) => card,
        None => return (StatusCode::BAD_REQUEST, "No card found!").into_response(),
    };

    format!("Your Balance: {} USD", data.balance).into_response()
}