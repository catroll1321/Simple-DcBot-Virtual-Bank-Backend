use std::{collections::HashMap, time::{SystemTime, UNIX_EPOCH}, fs, io, hash::{DefaultHasher, Hash, Hasher}};
use base64::Engine;
use base64::engine::general_purpose;
use chrono::{Datelike, Local, TimeZone};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use hmac::{Hmac, Mac};
use rust_decimal::{Decimal, prelude::Zero};
use rust_decimal::prelude::FromPrimitive;
use sha2::Sha256;
use structure::{CardInfo, DiscordTrade, StockHold, TradeHistory, TransactionType};

type HmacSha256 = Hmac<Sha256>;

pub fn generate_token(secret: &str, card_number: &str, good_thru: &str, verify_number: &str) -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    let payload = format!("{}|{}|{}|{}", card_number, good_thru, verify_number, now);
    let payload_encoded = general_purpose::STANDARD.encode(&payload);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
    mac.update(payload.as_bytes());
    let signature = hex::encode(mac.finalize().into_bytes());
    format!("{}.{}", payload_encoded, signature)
}

pub fn generate_n_digit(seed: u64, digits: u32) -> u64 {
    let mut seed_bytes = [0u8; 32];
    seed_bytes[..8].copy_from_slice(&seed.to_le_bytes());
    let mut rng = ChaCha8Rng::from_seed(seed_bytes);
    let lower = 10u64.pow(digits - 1);
    let upper = 10u64.pow(digits);
    rng.random_range(lower..upper)
}

//I have no idea bruh :(
pub fn generate_yymm(seed: u64) -> u16 {
    let mut seed_bytes = [0u8; 32];
    seed_bytes[..8].copy_from_slice(&seed.to_le_bytes());
    let mut rng = ChaCha8Rng::from_seed(seed_bytes);
    let year = rng.random_range(20..100);
    let month = rng.random_range(1..13);
    year * 100 + month as u16
}

pub fn get_card_map(path: &str) -> Result<HashMap<u64, CardInfo>, String> {
    let read = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {} ：{}", path, e))?;
    let parsed: HashMap<u64, CardInfo> = serde_json::from_str(&read)
        .map_err(|e| format!("Failed to analysis of {} ：{}", path, e))?;
    Ok(parsed)
}

pub fn get_trade_map(path: &str) -> Result<HashMap<i64, TradeHistory>, String> {
    let read = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {} ：{}", path, e))?;
    let parsed: HashMap<i64, TradeHistory> = serde_json::from_str(&read)
        .map_err(|e| format!("Failed to analysis of {} ：{}", path, e))?;
    Ok(parsed)
}

pub fn get_stock_map(path: &str) -> Result<HashMap<String, Vec<StockHold>>, String> {
    let read = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {} ：{}", path, e))?;
    let parsed: HashMap<String, Vec<StockHold>> = serde_json::from_str(&read)
        .map_err(|e| format!("Failed to analysis of {} ：{}", path, e))?;
    Ok(parsed)
}

pub fn write_card_info(path: &str, input: &HashMap<u64, CardInfo>) -> Result<(), io::Error> {
    let json_str = serde_json::to_string_pretty(&input)?;
    fs::write(path, &json_str)?;
    Ok(())
}

pub fn write_trade_info(path: &str, input: &HashMap<i64, TradeHistory>) -> Result<(), io::Error> {
    let json_str = serde_json::to_string_pretty(&input)?;
    fs::write(path, &json_str)?;
    Ok(())
}

pub fn write_stock_info(path: &str, input: &HashMap<String, Vec<StockHold>>) -> Result<(), io::Error> {
    let json_str = serde_json::to_string_pretty(&input)?;
    fs::write(path, &json_str)?;
    Ok(())
}

pub fn check_balance(balance: &Decimal, price: Decimal) -> bool {
    *balance >= price && price > Decimal::zero()
}

pub fn hash_str_to_u64(s: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

pub fn gen_card_num(mixture: u64) -> u64 {
    let card_issuer: u64 = 4878;
    let card_last12 = generate_n_digit(mixture, 12);
    let card_number = card_issuer*1000000000000 + card_last12;
    card_number
}
pub fn gen_card(mixture: u64, id: &str) -> CardInfo {
    let card_number = gen_card_num(mixture);
    let verify_number = generate_n_digit(mixture, 3);
    let good_thru = generate_yymm(mixture);
    CardInfo {
        card_holder: id.to_string(),
        card_number,
        good_thru,
        verify_number: verify_number as u16,
        balance: Decimal::zero(),
        connection: None,
        transaction: None,
    }
}

pub fn get_day_end(unix_time: i64) -> i64 {
    // for trade history
    let chrono_time = Local.timestamp_opt(unix_time, 0).unwrap();
    let year = chrono_time.year();
    let month = chrono_time.month();
    let day = chrono_time.day();
    let end_of_day = Local.with_ymd_and_hms(year, month, day, 23, 59, 59).unwrap();
    end_of_day.timestamp()
}

pub fn handler_transaction(id: DiscordTrade, card_map: &mut HashMap<u64, CardInfo>) -> Result<String, String> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
    let last_trade: i64;

    let mut trade_map: HashMap<i64, TradeHistory> = match get_trade_map("trade.json") {
        Ok(map) => map,
        Err(e) => {
            eprintln!("Error： {}", e);
            return Err(String::from("Server error, please call admin fixing!"));
        }
    };

    let data = match card_map.values_mut().find(|data| data.card_holder == id.card_holder) {
        Some(card) => card,
        None => return Err(String::from("No card found!")),
    };

    if trade_map.is_empty() {
        last_trade = 0;
    } else {
        let Some(last) = trade_map.keys().max() else { todo!() };
        last_trade = *last;
    };

    let new_balance = match id.transaction_type {
        TransactionType::Credit { amount } => {
            match Decimal::from_f64(amount) {
                Some(price) => {
                    data.balance += price;
                    let transaction_map = data.transaction.get_or_insert_with(HashMap::new);
                    transaction_map.insert(now, last_trade + 1);

                    let trade_info = TradeHistory {
                        timestamp: now,
                        transaction_type: TransactionType::Credit { amount },
                        target_user: id.target_user,
                    };

                    trade_map.insert(last_trade + 1, trade_info);
                    if let Err(e) = write_trade_info("trade.json", &trade_map) {
                        println!("Error in writing trade json: {}", e);
                        return Err(String::from("Server error, please call admin fixing!"));
                    }
                    Some(data.balance)
                }
                _ => None,
            }
        }
        TransactionType::Debit { amount } => {
            match Decimal::from_f64(amount) {
                Some(price) if check_balance(&data.balance, price) => {
                    data.balance -= price;
                    let transaction_map = data.transaction.get_or_insert_with(HashMap::new);
                    transaction_map.insert(now, last_trade + 1);

                    let trade_info = TradeHistory {
                        timestamp: now,
                        transaction_type: TransactionType::Debit { amount },
                        target_user: id.target_user,
                    };

                    trade_map.insert(last_trade + 1, trade_info);
                    if let Err(e) = write_trade_info("trade.json", &trade_map) {
                        println!("Error in writing trade json: {}", e);
                        return Err(String::from("Server error, please call admin fixing!"));
                    }
                    Some(data.balance)
                }
                _ => None,
            }
        }
    };

    let balance = match new_balance {
        Some(b) => b,
        None => return Err(String::from("Transaction failed, please check the amount format")),
    };

    if let Err(e) = write_card_info("account.json", &card_map) {
        println!("Error in writing card json: {}", e);
        return Err(String::from("Server error, please call admin fixing!"));
    }

    let message = format!("Transaction successful! Balance : {} USD", balance);
    Ok(message)
}