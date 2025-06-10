use std::collections::HashMap;
use std::collections::HashSet;
use std::time::{SystemTime, UNIX_EPOCH};
use axum::{extract::Json, response::IntoResponse, http::StatusCode};
use serde_json::json;
use structure::{Identification, CardInfo, TargetVerify, TargetInfo, DiscordTrade, TradeHistory, RegisterInfo};
use function::{generate_token, gen_card, hash_str_to_u64, handler_transaction, get_day_end, write_json_to_file, get_map, get_card_name};

pub async fn sign_up_discord(Json(info): Json<RegisterInfo>) -> impl IntoResponse {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let hash_id = hash_str_to_u64(&info.discord_id);
    let mixture = now + hash_id;
    let card_account = match gen_card(info.scheme, info.card_type, mixture, &info.discord_id) {
        Ok(card) => card,
        Err(e) => {
            eprintln!("Error： {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let card_number = &card_account.card_number.clone();
    let good_thru = &card_account.good_thru.clone();
    let verify_number = &card_account.verify_number.clone();

    let mut all_data: HashMap<u64, CardInfo> = match get_map("account.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if all_data.contains_key(&hash_id) {
        return (StatusCode::INTERNAL_SERVER_ERROR, "You have already signed up!").into_response();
    }

    all_data.insert(hash_id, card_account);

    if let Err(e) = write_json_to_file("account.json", &all_data) {
        eprintln!("Error in write account： {}", e);
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    (StatusCode::OK, Json(json!({
        "card_number": card_number,
        "good_thru": good_thru,
        "verify_number": verify_number,
    }))).into_response()
}

pub async fn discord_transaction(Json(id): Json<DiscordTrade>) -> impl IntoResponse {
    let mut card_map: HashMap<u64, CardInfo> = match get_map("account.json") {
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
    let mut card_map: HashMap<u64, CardInfo> = match get_map("account.json") {
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

    if let Err(e) = write_json_to_file("account.json", &card_map) {
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

    let card_map: HashMap<u64, CardInfo> = match get_map("account.json") {
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

    let trade_map: HashMap<i64, TradeHistory> = match get_map("trade.json") {
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

    (StatusCode::OK, Json(json!(trades))).into_response()
}

pub async fn check_target_exist(Json(id): Json<Identification>) -> impl IntoResponse {
    let card_map: HashMap<u64, CardInfo> = match get_map("account.json") {
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
    let mut card_map: HashMap<u64, CardInfo> = match get_map("account.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    let data = match card_map.values_mut().find(|data| data.card_holder == id.card_holder) {
        Some(card) => card,
        None => return (StatusCode::BAD_REQUEST, "No card found!").into_response(),
    };

    (StatusCode::OK, Json(json!({ "balance": data.balance }))).into_response()
}

pub async fn get_user_card(Json(id): Json<Identification>) -> impl IntoResponse {
    let mut card_map: HashMap<u64, CardInfo> = match get_map("account.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    let data = match card_map.values_mut().find(|data| data.card_holder == id.card_holder) {
        Some(card) => card,
        None => return (StatusCode::BAD_REQUEST, "No card found!").into_response(),
    };

    let card_type = data.card_type.clone();
    let scheme = data.scheme.clone();
    let card_name = match get_card_name(card_type) {
        Ok(name) => name,
        Err(e) => return (StatusCode::BAD_REQUEST, e).into_response(),
    };
    let cards = vec![format!("{}{}", scheme, card_name)];

    (StatusCode::OK, Json(json!({ "cards": cards }))).into_response()
}
