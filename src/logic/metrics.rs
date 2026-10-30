use std::collections::HashMap;
use crate::db::models::{Account, TransactionType, TransactionWithDetails};
use crate::logic::currency::convert_currency;

#[derive(Debug, Clone, PartialEq)]
pub struct CashFlowSummary {
    pub income: f64,
    pub expense: f64,
    pub net_delta: f64,
    pub savings_rate: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CategorySpendingItem {
    pub category_id: String,
    pub category_name: String,
    pub category_color: String,
    pub category_icon: String,
    pub total: f64,
    pub percentage: f64,
}

/// Calculates the total net worth across all non-archived accounts converted to `base_currency`.
pub fn calculate_net_worth(accounts: &[Account], base_currency: &str) -> f64 {
    let mut sum = 0.0;
    for acc in accounts {
        if !acc.is_archived {
            sum += convert_currency(acc.balance, &acc.currency, base_currency);
        }
    }
    (sum * 100.0).round() / 100.0
}

/// Computes monthly cash flow metrics (income, expenses, net savings, savings rate) converted to `base_currency`.
pub fn calculate_cash_flow(
    transactions: &[TransactionWithDetails],
    month_prefix: &str, // e.g. "2026-10"
    base_currency: &str,
) -> CashFlowSummary {
    let mut income = 0.0;
    let mut expense = 0.0;

    for item in transactions {
        let tx = &item.transaction;
        if tx.date.starts_with(month_prefix) {
            let tx_curr = item.account_currency.as_deref().unwrap_or(&tx.currency);
            let converted = convert_currency(tx.amount, tx_curr, base_currency);
            match tx.tx_type {
                TransactionType::Income => income += converted,
                TransactionType::Expense => expense += converted,
                TransactionType::Transfer => {} // Transfers between own accounts do not impact cash flow
            }
        }
    }

    let net_delta = income - expense;
    let savings_rate = if income > 0.0 && net_delta > 0.0 {
        ((net_delta / income) * 100.0).round()
    } else {
        0.0
    };

    CashFlowSummary {
        income: (income * 100.0).round() / 100.0,
        expense: (expense * 100.0).round() / 100.0,
        net_delta: (net_delta * 100.0).round() / 100.0,
        savings_rate,
    }
}

/// Computes category breakdown for expense transactions in a given month converted to `base_currency`.
pub fn calculate_category_breakdown(
    transactions: &[TransactionWithDetails],
    month_prefix: &str,
    base_currency: &str,
) -> Vec<CategorySpendingItem> {
    struct CatAccumulator {
        name: String,
        color: String,
        icon: String,
        total: f64,
    }

    let mut map: HashMap<String, CatAccumulator> = HashMap::new();
    let mut total_expense = 0.0;

    for item in transactions {
        let tx = &item.transaction;
        if tx.date.starts_with(month_prefix) && tx.tx_type == TransactionType::Expense {
            let cat_id = tx.category_id.clone().unwrap_or_else(|| "uncategorized".into());
            let name = item.category_name.clone().unwrap_or_else(|| "Other / Uncategorized".into());
            let color = item.category_color.clone().unwrap_or_else(|| "#64748B".into());
            let icon = item.category_icon.clone().unwrap_or_else(|| "CircleDollarSign".into());

            let tx_curr = item.account_currency.as_deref().unwrap_or(&tx.currency);
            let converted = convert_currency(tx.amount, tx_curr, base_currency);

            total_expense += converted;

            let entry = map.entry(cat_id).or_insert(CatAccumulator {
                name,
                color,
                icon,
                total: 0.0,
            });
            entry.total += converted;
        }
    }

    let mut list: Vec<CategorySpendingItem> = map
        .into_iter()
        .map(|(cat_id, acc)| {
            let percentage = if total_expense > 0.0 {
                ((acc.total / total_expense) * 100.0).round()
            } else {
                0.0
            };

            CategorySpendingItem {
                category_id: cat_id,
                category_name: acc.name,
                category_color: acc.color,
                category_icon: acc.icon,
                total: (acc.total * 100.0).round() / 100.0,
                percentage,
            }
        })
        .collect();

    // Sort descending by total spent
    list.sort_by(|a, b| b.total.partial_cmp(&a.total).unwrap_or(std::cmp::Ordering::Equal));
    list
}

#[derive(Debug, Clone, PartialEq)]
pub struct WeeklyFlow {
    pub week_number: usize,
    pub label: String,
    pub date_range: String,
    pub income: f64,
    pub expense: f64,
    pub net: f64,
    pub is_positive: bool,
    pub income_ratio: f64,
    pub expense_ratio: f64,
}

/// Calculates income, expenses, and net pacing broken down by week for a specific month converted to `base_currency`.
pub fn calculate_weekly_flow(
    transactions: &[TransactionWithDetails],
    year: i32,
    month: u32,
    base_currency: &str,
) -> Vec<WeeklyFlow> {
    let days_in_month = match month {
        2 => {
            let is_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
            if is_leap { 29 } else { 28 }
        }
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };

    let num_weeks = if days_in_month > 28 { 5 } else { 4 };
    let mut weekly_income = vec![0.0; num_weeks];
    let mut weekly_expense = vec![0.0; num_weeks];

    let prefix = format!("{:04}-{:02}-", year, month);

    for item in transactions {
        let tx = &item.transaction;
        if tx.date.starts_with(&prefix) {
            if let Some(day_str) = tx.date.get(8..10) {
                if let Ok(day) = day_str.parse::<u32>() {
                    let week_idx = match day {
                        1..=7 => 0,
                        8..=14 => 1,
                        15..=21 => 2,
                        22..=28 => 3,
                        _ => 4,
                    };
                    if week_idx < num_weeks {
                        let tx_curr = item.account_currency.as_deref().unwrap_or(&tx.currency);
                        let converted = convert_currency(tx.amount, tx_curr, base_currency);
                        match tx.tx_type {
                            TransactionType::Income => weekly_income[week_idx] += converted,
                            TransactionType::Expense => weekly_expense[week_idx] += converted,
                            TransactionType::Transfer => {}
                        }
                    }
                }
            }
        }
    }

    let mut max_val = 0.0f64;
    for i in 0..num_weeks {
        if weekly_income[i] > max_val {
            max_val = weekly_income[i];
        }
        if weekly_expense[i] > max_val {
            max_val = weekly_expense[i];
        }
    }
    if max_val <= 0.0 {
        max_val = 1.0;
    }

    let mut result = Vec::with_capacity(num_weeks);
    for i in 0..num_weeks {
        let date_range = match i {
            0 => "1st - 7th".to_string(),
            1 => "8th - 14th".to_string(),
            2 => "15th - 21st".to_string(),
            3 => "22nd - 28th".to_string(),
            _ => format!("29th - {}th", days_in_month),
        };
        let inc = (weekly_income[i] * 100.0).round() / 100.0;
        let exp = (weekly_expense[i] * 100.0).round() / 100.0;
        let net = (inc - exp) * 100.0 / 100.0;
        result.push(WeeklyFlow {
            week_number: i + 1,
            label: format!("Week {}", i + 1),
            date_range,
            income: inc,
            expense: exp,
            net,
            is_positive: net >= 0.0,
            income_ratio: (inc / max_val).clamp(0.0, 1.0),
            expense_ratio: (exp / max_val).clamp(0.0, 1.0),
        });
    }

    result
}

#[derive(Debug, Clone, PartialEq)]
pub struct MonthlyTrend {
    pub month_name: String,
    pub month_prefix: String,
    pub income: f64,
    pub expense: f64,
    pub net: f64,
    pub is_positive: bool,
    pub income_ratio: f64,
    pub expense_ratio: f64,
}

/// Calculates monthly performance trajectory across the past N months converted to `base_currency`.
pub fn calculate_monthly_trends(
    transactions: &[TransactionWithDetails],
    current_year: i32,
    current_month: u32,
    months_count: usize,
    base_currency: &str,
) -> Vec<MonthlyTrend> {
    let mut months = Vec::new();
    let mut y = current_year;
    let mut m = current_month as i32;

    for _ in 0..months_count {
        months.push((y, m as u32));
        m -= 1;
        if m < 1 {
            m = 12;
            y -= 1;
        }
    }
    months.reverse();

    let month_names = [
        "", "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    let mut trends = Vec::with_capacity(months.len());
    let mut max_val = 0.0f64;

    for (year, month) in months {
        let prefix = format!("{:04}-{:02}", year, month);
        let cf = calculate_cash_flow(transactions, &prefix, base_currency);
        if cf.income > max_val {
            max_val = cf.income;
        }
        if cf.expense > max_val {
            max_val = cf.expense;
        }
        let m_name = month_names.get(month as usize).unwrap_or(&"");
        trends.push((format!("{} '{:02}", m_name, year % 100), prefix, cf));
    }

    if max_val <= 0.0 {
        max_val = 1.0;
    }

    trends
        .into_iter()
        .map(|(label, prefix, cf)| MonthlyTrend {
            month_name: label,
            month_prefix: prefix,
            income: cf.income,
            expense: cf.expense,
            net: cf.net_delta,
            is_positive: cf.net_delta >= 0.0,
            income_ratio: (cf.income / max_val).clamp(0.0, 1.0),
            expense_ratio: (cf.expense / max_val).clamp(0.0, 1.0),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{Account, AccountType, PaymentType, Transaction, TransactionStatus};

    #[test]
    fn test_calculate_net_worth() {
        let accounts = vec![
            Account {
                id: "acc_1".into(),
                name: "Checking".into(),
                account_type: AccountType::Bank,
                currency: "USD".into(),
                balance: 5200.0,
                color: "".into(),
                icon: "".into(),
                is_archived: false,
                created_at: "".into(),
                updated_at: "".into(),
            },
            Account {
                id: "acc_2".into(),
                name: "Credit Card".into(),
                account_type: AccountType::CreditCard,
                currency: "USD".into(),
                balance: -350.0,
                color: "".into(),
                icon: "".into(),
                is_archived: false,
                created_at: "".into(),
                updated_at: "".into(),
            },
            Account {
                id: "acc_3".into(),
                name: "Old Savings".into(),
                account_type: AccountType::Savings,
                currency: "USD".into(),
                balance: 2000.0,
                color: "".into(),
                icon: "".into(),
                is_archived: true, // Should be excluded
                created_at: "".into(),
                updated_at: "".into(),
            },
        ];

        // 5200 - 350 = 4850.0
        assert_eq!(calculate_net_worth(&accounts, "USD"), 4850.0);

        // Multi-currency test: 100 USD + 100 EUR in USD base (1 EUR = 1.08 USD) -> 100 + 108 = 208 USD
        let multi_accounts = vec![
            Account {
                id: "acc_usd".into(),
                name: "USD Acc".into(),
                account_type: AccountType::Bank,
                currency: "USD".into(),
                balance: 100.0,
                color: "".into(),
                icon: "".into(),
                is_archived: false,
                created_at: "".into(),
                updated_at: "".into(),
            },
            Account {
                id: "acc_eur".into(),
                name: "EUR Acc".into(),
                account_type: AccountType::Bank,
                currency: "EUR".into(),
                balance: 100.0,
                color: "".into(),
                icon: "".into(),
                is_archived: false,
                created_at: "".into(),
                updated_at: "".into(),
            },
        ];
        assert_eq!(calculate_net_worth(&multi_accounts, "USD"), 208.0);
    }

    #[test]
    fn test_calculate_cash_flow() {
        let txs = vec![
            TransactionWithDetails {
                transaction: Transaction {
                    id: "tx1".into(),
                    account_id: "a1".into(),
                    to_account_id: None,
                    category_id: Some("cat1".into()),
                    tx_type: TransactionType::Income,
                    amount: 4000.0,
                    currency: "USD".into(),
                    date: "2026-10-01".into(),
                    time: None,
                    payee: None,
                    note: None,
                    status: TransactionStatus::Cleared,
                    payment_type: PaymentType::Transfer,
                    created_at: "".into(),
                },
                category_name: Some("Salary".into()),
                category_icon: None,
                category_color: None,
                account_name: None,
                account_color: None,
                account_icon: None,
                account_currency: None,
                to_account_name: None,
                to_account_currency: None,
            },
            TransactionWithDetails {
                transaction: Transaction {
                    id: "tx2".into(),
                    account_id: "a1".into(),
                    to_account_id: None,
                    category_id: Some("cat2".into()),
                    tx_type: TransactionType::Expense,
                    amount: 1000.0,
                    currency: "USD".into(),
                    date: "2026-10-05".into(),
                    time: None,
                    payee: None,
                    note: None,
                    status: TransactionStatus::Cleared,
                    payment_type: PaymentType::DebitCard,
                    created_at: "".into(),
                },
                category_name: Some("Rent".into()),
                category_icon: None,
                category_color: None,
                account_name: None,
                account_color: None,
                account_icon: None,
                account_currency: None,
                to_account_name: None,
                to_account_currency: None,
            },
        ];

        let summary = calculate_cash_flow(&txs, "2026-10", "USD");
        assert_eq!(summary.income, 4000.0);
        assert_eq!(summary.expense, 1000.0);
        assert_eq!(summary.net_delta, 3000.0);
        // (3000 / 4000) * 100 = 75%
        assert_eq!(summary.savings_rate, 75.0);
    }

    #[test]
    fn test_calculate_weekly_flow() {
        let txs = vec![
            TransactionWithDetails {
                transaction: Transaction {
                    id: "tx1".into(),
                    account_id: "a1".into(),
                    to_account_id: None,
                    category_id: None,
                    tx_type: TransactionType::Income,
                    amount: 2000.0,
                    currency: "USD".into(),
                    date: "2026-10-02".into(), // Week 1
                    time: None,
                    payee: None,
                    note: None,
                    status: TransactionStatus::Cleared,
                    payment_type: PaymentType::Transfer,
                    created_at: "".into(),
                },
                category_name: None,
                category_icon: None,
                category_color: None,
                account_name: None,
                account_color: None,
                account_icon: None,
                account_currency: None,
                to_account_name: None,
                to_account_currency: None,
            },
            TransactionWithDetails {
                transaction: Transaction {
                    id: "tx2".into(),
                    account_id: "a1".into(),
                    to_account_id: None,
                    category_id: None,
                    tx_type: TransactionType::Expense,
                    amount: 500.0,
                    currency: "USD".into(),
                    date: "2026-10-10".into(), // Week 2
                    time: None,
                    payee: None,
                    note: None,
                    status: TransactionStatus::Cleared,
                    payment_type: PaymentType::Cash,
                    created_at: "".into(),
                },
                category_name: None,
                category_icon: None,
                category_color: None,
                account_name: None,
                account_color: None,
                account_icon: None,
                account_currency: None,
                to_account_name: None,
                to_account_currency: None,
            },
        ];

        let flow = calculate_weekly_flow(&txs, 2026, 10, "USD");
        assert_eq!(flow.len(), 5);
        assert_eq!(flow[0].income, 2000.0);
        assert_eq!(flow[0].expense, 0.0);
        assert_eq!(flow[0].net, 2000.0);
        assert!(flow[0].is_positive);

        assert_eq!(flow[1].income, 0.0);
        assert_eq!(flow[1].expense, 500.0);
        assert_eq!(flow[1].net, -500.0);
        assert!(!flow[1].is_positive);
    }

    #[test]
    fn test_calculate_monthly_trends() {
        let txs = vec![
            TransactionWithDetails {
                transaction: Transaction {
                    id: "tx1".into(),
                    account_id: "a1".into(),
                    to_account_id: None,
                    category_id: None,
                    tx_type: TransactionType::Income,
                    amount: 3000.0,
                    currency: "USD".into(),
                    date: "2026-09-15".into(),
                    time: None,
                    payee: None,
                    note: None,
                    status: TransactionStatus::Cleared,
                    payment_type: PaymentType::Transfer,
                    created_at: "".into(),
                },
                category_name: None,
                category_icon: None,
                category_color: None,
                account_name: None,
                account_color: None,
                account_icon: None,
                account_currency: None,
                to_account_name: None,
                to_account_currency: None,
            },
        ];

        let trends = calculate_monthly_trends(&txs, 2026, 10, 3, "USD");
        assert_eq!(trends.len(), 3); // Aug, Sep, Oct
        assert_eq!(trends[1].month_prefix, "2026-09");
        assert_eq!(trends[1].income, 3000.0);
    }
}
