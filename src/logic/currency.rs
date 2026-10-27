use serde::{Deserialize, Serialize};

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

/// Standard internal exchange rates (referenced to 1 USD base).
pub fn get_exchange_rate_to_usd(currency: &str) -> f64 {
    match currency {
        "USD" => 1.0,
        "EUR" => 1.08,    // 1 EUR = 1.08 USD
        "GBP" => 1.30,    // 1 GBP = 1.30 USD
        "CAD" => 0.72,    // 1 CAD = 0.72 USD
        "AUD" => 0.66,    // 1 AUD = 0.66 USD
        "JPY" => 0.0066,  // 1 JPY = 0.0066 USD (152 JPY/USD)
        "BDT" => 0.0083,  // 1 BDT = 0.0083 USD (120.5 BDT/USD)
        "INR" => 0.012,   // 1 INR = 0.012 USD
        "CHF" => 1.15,    // 1 CHF = 1.15 USD
        "CNY" => 0.14,    // 1 CNY = 0.14 USD
        _ => 1.0,
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
}
