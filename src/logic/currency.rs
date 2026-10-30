use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredExchangeRates {
    pub time_last_update_unix: u64,
    pub rates: HashMap<String, f64>,
}

static DYNAMIC_RATES: LazyLock<RwLock<Option<StoredExchangeRates>>> =
    LazyLock::new(|| RwLock::new(None));

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CurrencyInfo {
    pub code: &'static str,
    pub symbol: &'static str,
    pub label: &'static str,
}

pub const DEFAULT_CURRENCY: &str = "BDT";

pub const SUPPORTED_CURRENCIES: &[CurrencyInfo] = &[
    CurrencyInfo { code: "BDT", symbol: "৳", label: "BDT (৳)" },
    CurrencyInfo { code: "USD", symbol: "$", label: "USD ($)" },
    CurrencyInfo { code: "EUR", symbol: "€", label: "EUR (€)" },
    CurrencyInfo { code: "GBP", symbol: "£", label: "GBP (£)" },
    CurrencyInfo { code: "INR", symbol: "₹", label: "INR (₹)" },
    CurrencyInfo { code: "CAD", symbol: "CA$", label: "CAD (CA$)" },
    CurrencyInfo { code: "AUD", symbol: "A$", label: "AUD (AU$)" },
    CurrencyInfo { code: "JPY", symbol: "¥", label: "JPY (¥)" },
];

/// Returns the currency symbol for a standard currency code.
pub fn get_currency_symbol(currency: &str) -> &'static str {
    match currency {
        "USD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        "JPY" => "¥",
        "BDT" => "৳",
        "INR" => "₹",
        "CAD" => "CA$",
        "AUD" => "A$",
        "CHF" => "CHF ",
        "CNY" => "¥",
        _ => "$",
    }
}

/// Formats a monetary amount into a clean localized currency string.
/// If `privacy_mode` is enabled, returns `••••••`.
pub fn format_currency(amount: f64, currency: &str, privacy_mode: bool) -> String {
    if privacy_mode {
        return "••••••".to_string();
    }

    let is_negative = amount < 0.0;
    let abs_amount = amount.abs();

    let formatted_number = match currency {
        "JPY" | "KRW" => format!("{:.0}", abs_amount),
        _ => format!("{:.2}", abs_amount),
    };

    // Add thousands separators
    let formatted_with_commas = add_thousands_separators(&formatted_number);

    let symbol = match currency {
        "USD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        "JPY" => "¥",
        "BDT" => "৳",
        "INR" => "₹",
        "CAD" => "CA$",
        "AUD" => "A$",
        "CHF" => "CHF ",
        "CNY" => "¥",
        _ => "",
    };

    if symbol.is_empty() {
        if is_negative {
            format!("-{} {}", formatted_with_commas, currency)
        } else {
            format!("{} {}", formatted_with_commas, currency)
        }
    } else if is_negative {
        format!("-{}{}", symbol, formatted_with_commas)
    } else {
        format!("{}{}", symbol, formatted_with_commas)
    }
}

/// Helper function to insert commas as thousands separators.
fn add_thousands_separators(num_str: &str) -> String {
    let (integer_part, decimal_part) = match num_str.split_once('.') {
        Some((int, dec)) => (int, Some(dec)),
        None => (num_str, None),
    };

    let len = integer_part.len();
    let mut result = String::with_capacity(len + (len / 3) + decimal_part.map_or(0, |d| d.len() + 1));

    for (i, ch) in integer_part.chars().enumerate() {
        result.push(ch);
        let remaining = len - 1 - i;
        if remaining > 0 && remaining.is_multiple_of(3) {
            result.push(',');
        }
    }

    if let Some(dec) = decimal_part {
        result.push('.');
        result.push_str(dec);
    }

    result
}

/// Default hardcoded fallback rate: returns the USD value of 1 unit of `currency`.
pub fn get_default_exchange_rate_to_usd(currency: &str) -> f64 {
    match currency {
        "USD" => 1.0,
        "EUR" => 1.08,    // 1 EUR = 1.08 USD
        "GBP" => 1.30,    // 1 GBP = 1.30 USD
        "CAD" => 0.72,    // 1 CAD = 0.72 USD
        "AUD" => 0.66,    // 1 AUD = 0.66 USD
        "JPY" => 0.0066,  // 1 JPY = 0.0066 USD
        "BDT" => 0.0083,  // 1 BDT = 0.0083 USD
        "INR" => 0.012,   // 1 INR = 0.012 USD
        "CHF" => 1.15,    // 1 CHF = 1.15 USD
        "CNY" => 0.14,    // 1 CNY = 0.14 USD
        _ => 1.0,
    }
}

/// Dynamic exchange rate: returns the USD value of 1 unit of `currency`.
/// If dynamic rates are loaded from cache or live API, uses them; otherwise uses default rates.
pub fn get_exchange_rate_to_usd(currency: &str) -> f64 {
    if currency == "USD" {
        return 1.0;
    }

    if let Ok(guard) = DYNAMIC_RATES.read() {
        if let Some(ref stored) = *guard {
            if let Some(&rate_per_usd) = stored.rates.get(currency) {
                if rate_per_usd > 0.0 {
                    return 1.0 / rate_per_usd;
                }
            }
        }
    }

    get_default_exchange_rate_to_usd(currency)
}

/// Loads cached exchange rates from exchange_rates.json into memory if present.
pub fn load_cached_exchange_rates() {
    let path = crate::db::config::AppConfig::exchange_rates_file_path();
    if let Ok(content) = std::fs::read_to_string(&path) {
        if let Ok(stored) = serde_json::from_str::<StoredExchangeRates>(&content) {
            if let Ok(mut guard) = DYNAMIC_RATES.write() {
                *guard = Some(stored);
            }
            return;
        }
    }

    // Seed defaults and persist
    let mut default_rates = HashMap::new();
    for code in &["USD", "EUR", "GBP", "CAD", "AUD", "JPY", "BDT", "INR", "CHF", "CNY"] {
        let rate_to_usd = get_default_exchange_rate_to_usd(code);
        let per_usd = if rate_to_usd > 0.0 { 1.0 / rate_to_usd } else { 1.0 };
        default_rates.insert(code.to_string(), per_usd);
    }
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let initial = StoredExchangeRates {
        time_last_update_unix: now,
        rates: default_rates,
    };
    save_exchange_rates_to_file(&initial);
    if let Ok(mut guard) = DYNAMIC_RATES.write() {
        *guard = Some(initial);
    }
}

/// Saves exchange rates to exchange_rates.json.
pub fn save_exchange_rates_to_file(rates: &StoredExchangeRates) {
    let path = crate::db::config::AppConfig::exchange_rates_file_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(rates) {
        let _ = std::fs::write(&path, json);
    }
}

#[derive(Deserialize)]
struct OpenErApiResponse {
    result: String,
    time_last_update_unix: Option<u64>,
    rates: HashMap<String, f64>,
}

#[derive(Deserialize)]
struct ExchangeRateApiV4Response {
    time_last_updated: Option<u64>,
    rates: HashMap<String, f64>,
}

#[derive(Deserialize)]
struct FawazCurrencyResponse {
    #[serde(default)]
    usd: HashMap<String, f64>,
}

fn fetch_from_open_er_api() -> Result<(u64, HashMap<String, f64>), String> {
    let resp = ureq::get("https://open.er-api.com/v6/latest/USD")
        .timeout(Duration::from_secs(10))
        .call()
        .map_err(|e| format!("open.er-api.com request failed: {e}"))?;

    let body: OpenErApiResponse = resp
        .into_json()
        .map_err(|e| format!("open.er-api.com JSON parse failed: {e}"))?;

    if body.result != "success" || body.rates.is_empty() {
        return Err("open.er-api.com returned non-success result or empty rates".into());
    }

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let update_time = body.time_last_update_unix.unwrap_or(now);
    Ok((update_time, body.rates))
}

fn fetch_from_exchangerate_api_v4() -> Result<(u64, HashMap<String, f64>), String> {
    let resp = ureq::get("https://api.exchangerate-api.com/v4/latest/USD")
        .timeout(Duration::from_secs(10))
        .call()
        .map_err(|e| format!("exchangerate-api.com request failed: {e}"))?;

    let body: ExchangeRateApiV4Response = resp
        .into_json()
        .map_err(|e| format!("exchangerate-api.com JSON parse failed: {e}"))?;

    if body.rates.is_empty() {
        return Err("exchangerate-api.com returned empty rates".into());
    }

    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
    let update_time = body.time_last_updated.unwrap_or(now);
    Ok((update_time, body.rates))
}

fn fetch_from_fawaz_currency_api() -> Result<(u64, HashMap<String, f64>), String> {
    // Try Cloudflare Pages endpoint first, then jsDelivr CDN
    let urls = [
        "https://latest.currency-api.pages.dev/v1/currencies/usd.json",
        "https://cdn.jsdelivr.net/npm/@fawazahmed0/currency-api@latest/v1/currencies/usd.json",
    ];

    let mut last_err = String::new();
    for url in urls {
        match ureq::get(url).timeout(Duration::from_secs(10)).call() {
            Ok(resp) => {
                match resp.into_json::<FawazCurrencyResponse>() {
                    Ok(body) if !body.usd.is_empty() => {
                        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                        let mut rates = HashMap::with_capacity(body.usd.len());
                        for (k, v) in body.usd {
                            rates.insert(k.to_uppercase(), v);
                        }
                        return Ok((now, rates));
                    }
                    Ok(_) => {
                        last_err = format!("{url} returned empty rates");
                    }
                    Err(e) => {
                        last_err = format!("{url} JSON parse failed: {e}");
                    }
                }
            }
            Err(e) => {
                last_err = format!("{url} request failed: {e}");
            }
        }
    }

    Err(last_err)
}

fn apply_and_persist_rates(update_time: u64, rates: HashMap<String, f64>) -> Result<(), String> {
    let stored = StoredExchangeRates {
        time_last_update_unix: update_time,
        rates,
    };

    save_exchange_rates_to_file(&stored);

    if let Ok(mut guard) = DYNAMIC_RATES.write() {
        *guard = Some(stored);
    }

    Ok(())
}

/// Fetches latest exchange rates using a resilient multi-provider strategy.
/// Attempts primary (open.er-api.com), then secondary (api.exchangerate-api.com),
/// then tertiary (latest.currency-api.pages.dev / jsDelivr CDN).
/// Persists the successful result to exchange_rates.json and updates in-memory cache.
pub fn fetch_and_update_exchange_rates() -> Result<(), String> {
    let mut errors = Vec::new();

    // 1. Primary: open.er-api.com
    match fetch_from_open_er_api() {
        Ok((update_time, rates)) => {
            println!("[ExchangeRates] Fetched latest rates from open.er-api.com");
            return apply_and_persist_rates(update_time, rates);
        }
        Err(e) => {
            eprintln!("[ExchangeRates] Provider 1 (open.er-api.com) failed: {e}. Trying fallback...");
            errors.push(e);
        }
    }

    // 2. Secondary: api.exchangerate-api.com
    match fetch_from_exchangerate_api_v4() {
        Ok((update_time, rates)) => {
            println!("[ExchangeRates] Fetched latest rates from exchangerate-api.com (fallback 1)");
            return apply_and_persist_rates(update_time, rates);
        }
        Err(e) => {
            eprintln!("[ExchangeRates] Provider 2 (exchangerate-api.com) failed: {e}. Trying fallback...");
            errors.push(e);
        }
    }

    // 3. Tertiary: fawaz currency-api (Cloudflare / jsDelivr)
    match fetch_from_fawaz_currency_api() {
        Ok((update_time, rates)) => {
            println!("[ExchangeRates] Fetched latest rates from currency-api pages.dev (fallback 2)");
            return apply_and_persist_rates(update_time, rates);
        }
        Err(e) => {
            eprintln!("[ExchangeRates] Provider 3 (currency-api) failed: {e}. All providers failed.");
            errors.push(e);
        }
    }

    Err(format!("All exchange rate providers failed: {}", errors.join("; ")))
}

/// Checks whether exchange rates are older than 24 hours (86400s), and spawns a background thread to update.
pub fn check_and_spawn_rates_update() {
    check_and_spawn_rates_update_with_callback::<fn()>(None);
}

/// Checks whether exchange rates are older than 24 hours (86400s), and spawns a background thread to update.
/// If updated successfully, invokes `on_updated`.
pub fn check_and_spawn_rates_update_with_callback<F>(on_updated: Option<F>)
where
    F: FnOnce() + Send + 'static,
{
    let needs_update = {
        if let Ok(guard) = DYNAMIC_RATES.read() {
            if let Some(ref stored) = *guard {
                let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
                now.saturating_sub(stored.time_last_update_unix) >= 86400
            } else {
                true
            }
        } else {
            true
        }
    };

    if needs_update {
        std::thread::spawn(move || {
            if let Err(e) = fetch_and_update_exchange_rates() {
                eprintln!("[ExchangeRates] Could not fetch live rates (will retry): {e}");
            } else {
                println!("[ExchangeRates] Live exchange rates successfully refreshed and cached.");
                if let Some(cb) = on_updated {
                    cb();
                }
            }
        });
    }
}

/// Converts an amount between two currencies using base USD rates.
pub fn convert_currency(amount: f64, from_currency: &str, to_currency: &str) -> f64 {
    if from_currency == to_currency {
        return amount;
    }

    let from_to_usd = get_exchange_rate_to_usd(from_currency);
    let to_to_usd = get_exchange_rate_to_usd(to_currency);

    if to_to_usd == 0.0 {
        return amount;
    }

    let in_usd = amount * from_to_usd;
    let converted = in_usd / to_to_usd;
    (converted * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_and_supported_currencies() {
        assert_eq!(DEFAULT_CURRENCY, "BDT");
        assert_eq!(SUPPORTED_CURRENCIES[0].code, "BDT");
        assert!(SUPPORTED_CURRENCIES.iter().any(|c| c.code == "USD"));
        assert!(SUPPORTED_CURRENCIES.iter().any(|c| c.code == "EUR"));
        assert!(SUPPORTED_CURRENCIES.iter().any(|c| c.code == "BDT"));
    }

    #[test]
    fn test_get_currency_symbol() {
        assert_eq!(get_currency_symbol("BDT"), "৳");
        assert_eq!(get_currency_symbol("USD"), "$");
        assert_eq!(get_currency_symbol("EUR"), "€");
        assert_eq!(get_currency_symbol("GBP"), "£");
        assert_eq!(get_currency_symbol("INR"), "₹");
        assert_eq!(get_currency_symbol("CAD"), "CA$");
        assert_eq!(get_currency_symbol("AUD"), "A$");
        assert_eq!(get_currency_symbol("JPY"), "¥");
    }

    #[test]
    fn test_currency_formatting() {
        assert_eq!(format_currency(1250.50, "USD", false), "$1,250.50");
        assert_eq!(format_currency(-385.40, "USD", false), "-$385.40");
        assert_eq!(format_currency(0.0, "EUR", false), "€0.00");
        assert_eq!(format_currency(15000.0, "JPY", false), "¥15,000");
        assert_eq!(format_currency(450.0, "BDT", false), "৳450.00");
        assert_eq!(format_currency(99.99, "XYZ", false), "99.99 XYZ");
    }

    #[test]
    fn test_privacy_mode_masking() {
        assert_eq!(format_currency(5240.50, "USD", true), "••••••");
        assert_eq!(format_currency(-100.0, "EUR", true), "••••••");
    }

    #[test]
    fn test_currency_conversion() {
        // Identity
        assert_eq!(convert_currency(100.0, "USD", "USD"), 100.0);

        // 100 EUR in USD -> 100 * 1.08 = 108.0
        assert_eq!(convert_currency(100.0, "EUR", "USD"), 108.0);
    }

    #[test]
    fn test_multi_provider_payload_parsing() {
        // Provider 1: open.er-api.com
        let open_er_json = r#"{
            "result": "success",
            "time_last_update_unix": 1730246401,
            "rates": {"USD": 1.0, "EUR": 0.92, "BDT": 120.5}
        }"#;
        let p1: OpenErApiResponse = serde_json::from_str(open_er_json).unwrap();
        assert_eq!(p1.result, "success");
        assert_eq!(p1.rates.get("BDT"), Some(&120.5));

        // Provider 2: exchangerate-api.com v4
        let exchangerate_json = r#"{
            "base": "USD",
            "time_last_updated": 1730246401,
            "rates": {"USD": 1.0, "EUR": 0.92, "BDT": 120.5}
        }"#;
        let p2: ExchangeRateApiV4Response = serde_json::from_str(exchangerate_json).unwrap();
        assert_eq!(p2.rates.get("BDT"), Some(&120.5));

        // Provider 3: fawaz currency-api (lowercase keys)
        let fawaz_json = r#"{
            "date": "2026-10-30",
            "usd": {"eur": 0.92, "bdt": 120.5}
        }"#;
        let p3: FawazCurrencyResponse = serde_json::from_str(fawaz_json).unwrap();
        let mut p3_uppercase = HashMap::new();
        for (k, v) in p3.usd {
            p3_uppercase.insert(k.to_uppercase(), v);
        }
        assert_eq!(p3_uppercase.get("BDT"), Some(&120.5));
    }
}
