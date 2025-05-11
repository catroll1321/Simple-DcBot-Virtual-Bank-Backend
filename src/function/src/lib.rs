use std::{collections::HashMap, time::{SystemTime, UNIX_EPOCH}, fs, io, hash::{DefaultHasher, Hash, Hasher}};
use base64::Engine;
use base64::engine::general_purpose;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use hmac::{Hmac, Mac};
use rust_decimal::{Decimal, prelude::Zero};
use sha2::Sha256;
use structure::CardInfo;

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

pub fn get_json_map(path: &str) -> Result<HashMap<u64, CardInfo>, String> {
    let read = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {} ：{}", path, e))?;
    let parsed: HashMap<u64, CardInfo> = serde_json::from_str(&read)
        .map_err(|e| format!("Failed to analysis of {} ：{}", path, e))?;
    Ok(parsed)
}

pub fn write_json_info(path: &str, input: &HashMap<u64, CardInfo>) -> Result<(), io::Error> {
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
        stock: None,
        connection: None,
        transaction: None,
    }
}
