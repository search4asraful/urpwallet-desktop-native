use std::collections::HashMap;
use crate::db::models::{Account, TransactionType, TransactionWithDetails};

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

/// Calculates the total net worth across all non-archived accounts.
pub fn calculate_net_worth(accounts: &[Account]) -> f64 {
    let mut sum = 0.0;
    for acc in accounts {
        if !acc.is_archived {
            sum += acc.balance;
        }
    }
    (sum * 100.0).round() / 100.0
}

/// Computes monthly cash flow metrics (income, expenses, net savings, savings rate).
pub fn calculate_cash_flow(
    transactions: &[TransactionWithDetails],
    month_prefix: &str, // e.g. "2026-10"
) -> CashFlowSummary {
    let mut income = 0.0;
    let mut expense = 0.0;

    for item in transactions {
        let tx = &item.transaction;
        if tx.date.starts_with(month_prefix) {
            match tx.tx_type {
                TransactionType::Income => income += tx.amount,
                TransactionType::Expense => expense += tx.amount,
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

/// Computes category breakdown for expense transactions in a given month.
pub fn calculate_category_breakdown(
    transactions: &[TransactionWithDetails],
    month_prefix: &str,
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

            total_expense += tx.amount;

            let entry = map.entry(cat_id).or_insert(CatAccumulator {
                name,
                color,
                icon,
                total: 0.0,
            });
            entry.total += tx.amount;
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
        assert_eq!(calculate_net_worth(&accounts), 4850.0);
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

        let summary = calculate_cash_flow(&txs, "2026-10");
        assert_eq!(summary.income, 4000.0);
        assert_eq!(summary.expense, 1000.0);
        assert_eq!(summary.net_delta, 3000.0);
        // (3000 / 4000) * 100 = 75%
        assert_eq!(summary.savings_rate, 75.0);
    }
}
