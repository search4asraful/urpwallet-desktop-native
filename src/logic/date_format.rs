use chrono::NaiveDate;

/// Formats a canonical ISO date string (`YYYY-MM-DD`) into the requested presentation format.
///
/// Supported formats:
/// - `"YYYY-MM-DD"` (ISO): e.g. `2026-10-29`
/// - `"DD/MM/YYYY"` (EU):  e.g. `29/10/2026`
/// - `"MM/DD/YYYY"` (US):  e.g. `10/29/2026`
///
/// If parsing fails, falls back gracefully to the original string.
pub fn format_date(iso_date_str: &str, target_format: &str) -> String {
    let trimmed = iso_date_str.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    match NaiveDate::parse_from_str(trimmed, "%Y-%m-%d") {
        Ok(date) => format_naive_date(date, target_format),
        Err(_) => trimmed.to_string(),
    }
}

/// Formats a `chrono::NaiveDate` according to the presentation format specification.
pub fn format_naive_date(date: NaiveDate, target_format: &str) -> String {
    match target_format {
        "DD/MM/YYYY" => date.format("%d/%m/%Y").to_string(),
        "MM/DD/YYYY" => date.format("%m/%d/%Y").to_string(),
        _ => date.format("%Y-%m-%d").to_string(),
    }
}

/// Parses a date string input from the user or UI.
///
/// Attempts to parse first according to the active presentation format,
/// then falls back to ISO `YYYY-MM-DD` and other standard common formats
/// so that users can input dates naturally or paste ISO strings.
pub fn parse_date_input(input_str: &str, active_format: &str) -> Result<NaiveDate, String> {
    let trimmed = input_str.trim();
    if trimmed.is_empty() {
        return Err("Date cannot be empty".to_string());
    }

    let formats_to_try: Vec<&str> = match active_format {
        "DD/MM/YYYY" => vec!["%d/%m/%Y", "%d-%m-%Y", "%Y-%m-%d", "%m/%d/%Y"],
        "MM/DD/YYYY" => vec!["%m/%d/%Y", "%m-%d-%Y", "%Y-%m-%d", "%d/%m/%Y"],
        _ => vec!["%Y-%m-%d", "%d/%m/%Y", "%m/%d/%Y", "%d-%m-%Y"],
    };

    for fmt in formats_to_try {
        if let Ok(date) = NaiveDate::parse_from_str(trimmed, fmt) {
            return Ok(date);
        }
    }

    Err(format!(
        "Please enter a valid date in {active_format} format"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_date() {
        let iso = "2026-10-29";

        assert_eq!(format_date(iso, "YYYY-MM-DD"), "2026-10-29");
        assert_eq!(format_date(iso, "DD/MM/YYYY"), "29/10/2026");
        assert_eq!(format_date(iso, "MM/DD/YYYY"), "10/29/2026");

        // Graceful fallback on empty or invalid input
        assert_eq!(format_date("", "DD/MM/YYYY"), "");
        assert_eq!(format_date("not-a-date", "DD/MM/YYYY"), "not-a-date");
    }

    #[test]
    fn test_parse_date_input() {
        // ISO parsing
        let d1 = parse_date_input("2026-10-29", "YYYY-MM-DD").unwrap();
        assert_eq!(d1, NaiveDate::from_ymd_opt(2026, 10, 29).unwrap());

        // EU parsing
        let d2 = parse_date_input("29/10/2026", "DD/MM/YYYY").unwrap();
        assert_eq!(d2, NaiveDate::from_ymd_opt(2026, 10, 29).unwrap());

        // US parsing
        let d3 = parse_date_input("10/29/2026", "MM/DD/YYYY").unwrap();
        assert_eq!(d3, NaiveDate::from_ymd_opt(2026, 10, 29).unwrap());

        // Cross-format tolerance: ISO date when active is EU
        let d4 = parse_date_input("2026-10-29", "DD/MM/YYYY").unwrap();
        assert_eq!(d4, NaiveDate::from_ymd_opt(2026, 10, 29).unwrap());

        // Invalid format rejection
        assert!(parse_date_input("invalid", "YYYY-MM-DD").is_err());
        assert!(parse_date_input("", "YYYY-MM-DD").is_err());
    }
}
