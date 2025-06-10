use std::collections::HashMap;
use rust_decimal::Decimal;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
pub struct CardInfo {
    pub card_holder: String,
    pub card_number: String,
    pub good_thru: String,
    pub verify_number: String,
    pub scheme: String,
    pub card_type: String,
    pub balance: Decimal,
    pub connection: Option<HashMap<String, Vec<TargetInfo>>>,
    pub transaction: Option<HashMap<i64, i64>>,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(tag = "action", rename_all = "lowercase")]
pub enum TransactionType {
    Credit { amount: f64 },
    Debit { amount: f64 },
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TradeHistory {
    pub timestamp: i64,
    pub transaction_type: TransactionType,
    pub target_user: String,
}

#[derive(Serialize, Deserialize)]
pub struct DiscordTrade {
    pub card_holder: String,
    pub target_user: String,
    pub transaction_type: TransactionType,
}

#[derive(Serialize, Deserialize)]
pub struct RegisterInfo {
    pub discord_id: String,
    pub scheme: String,
    pub card_type: String,
}

#[derive(Serialize, Deserialize)]
pub struct Identification {
    pub card_holder: String,
}

#[derive(Serialize, Deserialize)]
pub struct TargetVerify {
    pub card_holder: String,
    pub target: String,
}

#[derive(Serialize, Deserialize)]
pub struct TargetInfo {
    pub target: String,
    pub token: String,
}

#[derive(Serialize, Deserialize)]
pub struct Symbol {
    pub symbol: String,
}

#[derive(Serialize, Deserialize)]
pub struct Stock {
    pub buy_type: String,
    pub symbol: String,
    pub hand: Decimal,
    pub leverage: Decimal,
    pub price: Decimal,
}

#[derive(Serialize, Deserialize)]
pub struct BuyStock {
    pub buy_type: String,
    pub symbol: String,
    pub hand: Decimal,
    pub leverage: Decimal,
    pub token: String,
    pub target: String,
    pub card_holder: String,
}

#[derive(Serialize, Deserialize)]
pub struct SellStock {
    pub symbol: String,
    pub timestamp: i64,
    pub token: String,
    pub target: String,
    pub card_holder: String,
}

#[derive(Serialize, Deserialize)]
pub struct StockHold {
    pub timestamp: i64,
    pub stock: Stock,
}

#[derive(Serialize, Deserialize)]
pub struct StockHistory {
    pub symbol: String,
    pub period: String,
    pub interval: String,
}
