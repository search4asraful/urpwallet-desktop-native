use chrono::{Datelike, NaiveDate};
use crate::db::models::BudgetWithProgress;

#[derive(Debug, Clone, PartialEq)]
pub struct BudgetPacing {
    pub budget_id: String,
    pub budget_name: String,
    pub amount: f64,
    pub spent: f64,
    pub remaining: f64,
    pub daily_safe_to_spend: f64,
    pub spent_percentage: f64,
    pub is_over_budget: bool,
    pub category_name: Option<String>,
    pub category_color: Option<String>,
    pub category_icon: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SafeToSpendResult {
    /// Safe amount to spend per day for the rest of the current month
    pub daily_safe_to_spend: f64,
    /// Total discretionary budget allocated for this month
    pub total_budgeted: f64,
    /// Total spent across all budgets this month
    pub total_spent: f64,
    /// Remaining unspent discretionary budget
    pub total_remaining: f64,
    /// Number of days left in the current month (inclusive of today)
    pub days_left_in_month: u32,
    /// Total days in the current month
    pub days_in_month: u32,
    /// Whether total spending has exceeded the total budget
    pub is_over_budget: bool,
    /// Individual pacing breakdown per budget
    pub budget_pacing: Vec<BudgetPacing>,
}

/// Computes the number of days in the month of the given date.
pub fn days_in_month(date: NaiveDate) -> u32 {
    let year = date.year();
    let month = date.month();
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let first_of_next_month = NaiveDate::from_ymd_opt(next_year, next_month, 1)
        .unwrap_or(date);
    let last_of_current_month = first_of_next_month.pred_opt().unwrap_or(date);
    last_of_current_month.day()
}

/// Computes the remaining days in the month inclusive of the current date.
/// For example, on Oct 25th in a 31-day month: 31 - 25 + 1 = 7 days left.
pub fn days_remaining_in_month(date: NaiveDate) -> u32 {
    let total_days = days_in_month(date);
    let current_day = date.day();
    if current_day <= total_days {
        (total_days - current_day) + 1
    } else {
        1
    }
}

/// Dynamic Daily Safe-to-Spend formula:
/// `Safe-to-Spend = (Total Remaining Discretionary Budget) / (Days Left in Current Month)`
pub fn calculate_safe_to_spend(
    budgets: &[BudgetWithProgress],
    current_date: NaiveDate,
) -> SafeToSpendResult {
    let total_days = days_in_month(current_date);
    let days_left = days_remaining_in_month(current_date);

    let mut total_budgeted = 0.0;
    let mut total_spent = 0.0;
    let mut pacing_list = Vec::new();

    for b in budgets {
        total_budgeted += b.budget.amount;
        total_spent += b.spent;

        let remaining = b.budget.amount - b.spent;
        let is_over = b.spent > b.budget.amount;
        let daily = if is_over || remaining <= 0.0 {
            0.0
        } else {
            remaining / (days_left as f64)
        };

        pacing_list.push(BudgetPacing {
            budget_id: b.budget.id.clone(),
            budget_name: b.budget.name.clone(),
            amount: b.budget.amount,
            spent: b.spent,
            remaining,
            daily_safe_to_spend: (daily * 100.0).round() / 100.0,
            spent_percentage: b.spent_percentage,
            is_over_budget: is_over,
            category_name: b.category_name.clone(),
            category_color: b.category_color.clone(),
            category_icon: b.category_icon.clone(),
        });
    }

    let total_remaining = total_budgeted - total_spent;
    let is_over_budget = total_remaining < 0.0;
    let daily_safe_to_spend = if is_over_budget || total_remaining <= 0.0 || days_left == 0 {
        0.0
    } else {
        let raw = total_remaining / (days_left as f64);
        (raw * 100.0).round() / 100.0
    };

    SafeToSpendResult {
        daily_safe_to_spend,
        total_budgeted,
        total_spent,
        total_remaining,
        days_left_in_month: days_left,
        days_in_month: total_days,
        is_over_budget,
        budget_pacing: pacing_list,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::{Budget, BudgetPeriod};

    #[test]
    fn test_days_in_month_and_remaining() {
        // Oct 2026 has 31 days
        let oct_25 = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        assert_eq!(days_in_month(oct_25), 31);
        // 31 - 25 + 1 = 7 days remaining (25, 26, 27, 28, 29, 30, 31)
        assert_eq!(days_remaining_in_month(oct_25), 7);

        // Last day of month
        let oct_31 = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();
        assert_eq!(days_remaining_in_month(oct_31), 1);

        // Feb in leap year 2024 (29 days)
        let feb_leap = NaiveDate::from_ymd_opt(2024, 2, 10).unwrap();
        assert_eq!(days_in_month(feb_leap), 29);
        assert_eq!(days_remaining_in_month(feb_leap), 20);

        // Feb in non-leap year 2025 (28 days)
        let feb_non_leap = NaiveDate::from_ymd_opt(2025, 2, 1).unwrap();
        assert_eq!(days_in_month(feb_non_leap), 28);
        assert_eq!(days_remaining_in_month(feb_non_leap), 28);
    }

    #[test]
    fn test_safe_to_spend_calculation() {
        let oct_25 = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();
        // 7 days left in Oct

        let budgets = vec![
            BudgetWithProgress {
                budget: Budget {
                    id: "b1".into(),
                    name: "Food".into(),
                    amount: 500.0,
                    period: BudgetPeriod::Monthly,
                    category_id: None,
                    start_date: "2026-10-01".into(),
                    end_date: "2026-10-31".into(),
                    created_at: "".into(),
                },
                spent: 220.0, // 280 remaining
                remaining: 280.0,
                spent_percentage: 44.0,
                category_name: Some("Food".into()),
                category_color: None,
                category_icon: None,
            },
            BudgetWithProgress {
                budget: Budget {
                    id: "b2".into(),
                    name: "Entertainment".into(),
                    amount: 140.0,
                    period: BudgetPeriod::Monthly,
                    category_id: None,
                    start_date: "2026-10-01".into(),
                    end_date: "2026-10-31".into(),
                    created_at: "".into(),
                },
                spent: 70.0, // 70 remaining
                remaining: 70.0,
                spent_percentage: 50.0,
                category_name: Some("Fun".into()),
                category_color: None,
                category_icon: None,
            },
        ];

        let result = calculate_safe_to_spend(&budgets, oct_25);

        assert_eq!(result.total_budgeted, 640.0);
        assert_eq!(result.total_spent, 290.0);
        assert_eq!(result.total_remaining, 350.0);
        assert_eq!(result.days_left_in_month, 7);
        // 350 remaining / 7 days left = 50.00 / day
        assert_eq!(result.daily_safe_to_spend, 50.0);
        assert!(!result.is_over_budget);

        // Budget 1 daily: 280 / 7 = 40.0
        assert_eq!(result.budget_pacing[0].daily_safe_to_spend, 40.0);
        // Budget 2 daily: 70 / 7 = 10.0
        assert_eq!(result.budget_pacing[1].daily_safe_to_spend, 10.0);
    }

    #[test]
    fn test_safe_to_spend_over_budget() {
        let oct_25 = NaiveDate::from_ymd_opt(2026, 10, 25).unwrap();

        let budgets = vec![BudgetWithProgress {
            budget: Budget {
                id: "b1".into(),
                name: "Shopping".into(),
                amount: 200.0,
                period: BudgetPeriod::Monthly,
                category_id: None,
                start_date: "2026-10-01".into(),
                end_date: "2026-10-31".into(),
                created_at: "".into(),
            },
            spent: 250.0, // Over by 50
            remaining: -50.0,
            spent_percentage: 125.0,
            category_name: None,
            category_color: None,
            category_icon: None,
        }];

        let result = calculate_safe_to_spend(&budgets, oct_25);
        assert_eq!(result.daily_safe_to_spend, 0.0);
        assert!(result.is_over_budget);
        assert_eq!(result.total_remaining, -50.0);
    }
}
