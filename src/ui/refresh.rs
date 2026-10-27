use std::rc::Rc;
use chrono::Utc;
use slint::{ModelRc, VecModel};

use urpwallet::db::models::{AccountType, DebtType, TransactionFilter, TransactionType, TransactionWithDetails};
use urpwallet::db::Database;
use urpwallet::logic::{
    calculate_cash_flow, calculate_category_breakdown, calculate_net_worth, calculate_safe_to_spend,
    format_currency,
};

use crate::{
    AccountItem, BillItem, BudgetItem, CategoryItem, CategorySpendingItem, DebtItem, MainWindow,
    TransactionItem,
};
use crate::ui::mappers::map_tx_item;

/// Refreshes and updates account state and applies active filter tab.
pub fn refresh_accounts(ui: &MainWindow, db: &Database) -> Vec<AccountItem> {
    let privacy = ui.get_privacy_mode();
    let base_currency = db
        .get_setting("base_currency")
        .ok()
        .flatten()
        .unwrap_or_else(|| "BDT".to_string());

    let accounts = db.list_accounts(false).unwrap_or_default();
    let net_worth = calculate_net_worth(&accounts);

    let mut total_assets = 0.0;
    let mut total_liabilities = 0.0;

    let account_items: Vec<AccountItem> = accounts
        .iter()
        .map(|acc| {
            let is_liability =
                acc.account_type == AccountType::CreditCard || acc.account_type == AccountType::Loan;
            if is_liability {
                total_liabilities += acc.balance.abs();
            } else {
                total_assets += acc.balance.max(0.0);
            }

            AccountItem {
                id: acc.id.clone().into(),
                name: acc.name.clone().into(),
                acc_type: acc.account_type.as_str().replace('_', " ").to_uppercase().into(),
                currency: acc.currency.clone().into(),
                balance: format_currency(acc.balance, &acc.currency, privacy).into(),
                color: acc.color.clone().into(),
                is_liability,
            }
        })
        .collect();

    ui.set_net_worth(format_currency(net_worth, &base_currency, privacy).into());
    ui.set_total_assets(format_currency(total_assets, &base_currency, privacy).into());
    ui.set_total_liabilities(format_currency(total_liabilities, &base_currency, privacy).into());
    ui.set_accounts(ModelRc::from(Rc::new(VecModel::from(account_items.clone()))));

    apply_accounts_filter(ui, &account_items);

    // Set default modal accounts if not yet set
    if let Some(first_acc) = accounts.first()
        && ui.get_modal_account_id().is_empty()
    {
        ui.set_modal_account_id(first_acc.id.clone().into());
        ui.set_modal_account_name(first_acc.name.clone().into());
    }
    if accounts.len() > 1 && ui.get_modal_to_account_id().is_empty() {
        ui.set_modal_to_account_id(accounts[1].id.clone().into());
        ui.set_modal_to_account_name(accounts[1].name.clone().into());
    }

    account_items
}

/// Applies active accounts filter tab (0: All, 1: Assets, 2: Liabilities).
pub fn apply_accounts_filter(ui: &MainWindow, account_items: &[AccountItem]) {
    let tab = ui.get_accounts_filter_tab();
    let filtered: Vec<AccountItem> = account_items
        .iter()
        .filter(|acc| match tab {
            1 => !acc.is_liability,
            2 => acc.is_liability,
            _ => true,
        })
        .cloned()
        .collect();
    ui.set_filtered_accounts(ModelRc::from(Rc::new(VecModel::from(filtered))));
}

/// Refreshes and updates debts state and applies active filter tab.
pub fn refresh_debts(ui: &MainWindow, db: &Database) -> Vec<DebtItem> {
    let privacy = ui.get_privacy_mode();
    let base_currency = db
        .get_setting("base_currency")
        .ok()
        .flatten()
        .unwrap_or_else(|| "BDT".to_string());

    let debts = db.list_debts(false).unwrap_or_default();
    let debt_items: Vec<DebtItem> = debts
        .into_iter()
        .map(|d| {
            let rem = (d.amount - d.settled_amount).max(0.0);
            DebtItem {
                id: d.id.into(),
                person: d.person_name.into(),
                amount: format_currency(d.amount, &base_currency, privacy).into(),
                amount_raw: d.amount.to_string().into(),
                settled: format_currency(d.settled_amount, &base_currency, privacy).into(),
                remaining: format_currency(rem, &base_currency, privacy).into(),
                status: d.status.as_str().into(),
                is_loan: d.debt_type == DebtType::LoanOwedToMe,
                due_date: d.due_date.unwrap_or_default().into(),
                notes: d.notes.unwrap_or_default().into(),
            }
        })
        .collect();

    ui.set_debts(ModelRc::from(Rc::new(VecModel::from(debt_items.clone()))));
    apply_debts_filter(ui, &debt_items);
    debt_items
}

/// Applies active debts filter tab (0: All, 1: Lent, 2: Borrowed).
pub fn apply_debts_filter(ui: &MainWindow, debt_items: &[DebtItem]) {
    let tab = ui.get_debts_filter_tab();
    let filtered: Vec<DebtItem> = debt_items
        .iter()
        .filter(|d| match tab {
            1 => d.is_loan,
            2 => !d.is_loan,
            _ => true,
        })
        .cloned()
        .collect();
    ui.set_filtered_debts(ModelRc::from(Rc::new(VecModel::from(filtered))));
}

/// Refreshes and updates categories state for settings and transaction modal.
pub fn refresh_categories(ui: &MainWindow, db: &Database) -> Vec<CategoryItem> {
    let categories = db.list_categories_with_counts().unwrap_or_default();
    let mut expense_count = 0;
    let mut income_count = 0;

    let cat_items: Vec<CategoryItem> = categories
        .into_iter()
        .map(|(c, count)| {
            let cat_type_str = c.category_type.as_str();
            if cat_type_str == "expense" {
                expense_count += 1;
            } else if cat_type_str == "income" {
                income_count += 1;
            }

            CategoryItem {
                id: c.id.into(),
                name: c.name.into(),
                cat_type: cat_type_str.into(),
                color: c.color.into(),
                tx_count: count as i32,
            }
        })
        .collect();

    ui.set_cat_expense_count(expense_count);
    ui.set_cat_income_count(income_count);
    ui.set_categories(ModelRc::from(Rc::new(VecModel::from(cat_items.clone()))));

    apply_categories_filter(ui, &cat_items);
    apply_modal_categories(ui, &cat_items);

    cat_items
}

/// Applies active category catalog filter tab (0: All, 1: Expense, 2: Income).
pub fn apply_categories_filter(ui: &MainWindow, cat_items: &[CategoryItem]) {
    let tab = ui.get_cat_catalog_filter();
    let filtered: Vec<CategoryItem> = cat_items
        .iter()
        .filter(|c| match tab {
            1 => c.cat_type == "expense",
            2 => c.cat_type == "income",
            _ => true,
        })
        .cloned()
        .collect();
    ui.set_filtered_categories(ModelRc::from(Rc::new(VecModel::from(filtered))));
}

/// Applies modal category filtering based on modal tx type and show_all toggle.
pub fn apply_modal_categories(ui: &MainWindow, cat_items: &[CategoryItem]) {
    let show_all = ui.get_modal_cat_show_all();
    let tx_type = ui.get_modal_tx_type();

    let filtered: Vec<CategoryItem> = cat_items
        .iter()
        .filter(|c| {
            if show_all {
                true
            } else if tx_type == 0 {
                c.cat_type == "expense" || c.cat_type.is_empty()
            } else if tx_type == 1 {
                c.cat_type == "income" || c.cat_type.is_empty()
            } else {
                true
            }
        })
        .cloned()
        .collect();
    ui.set_modal_categories(ModelRc::from(Rc::new(VecModel::from(filtered))));
}

/// Refreshes transactions list respecting the active filter tab and search query.
pub fn refresh_transactions(
    ui: &MainWindow,
    db: &Database,
) -> (Vec<TransactionItem>, Vec<TransactionWithDetails>) {
    let privacy = ui.get_privacy_mode();
    let base_currency = db
        .get_setting("base_currency")
        .ok()
        .flatten()
        .unwrap_or_else(|| "BDT".to_string());
    let current_month = Utc::now().format("%Y-%m").to_string();

    let default_filter = TransactionFilter {
        limit: Some(250),
        ..Default::default()
    };
    let all_txs = db.list_transactions(&default_filter).unwrap_or_default();
    let cash_flow = calculate_cash_flow(&all_txs, &current_month);

    ui.set_monthly_income(format_currency(cash_flow.income, &base_currency, privacy).into());
    ui.set_monthly_expense(format_currency(cash_flow.expense, &base_currency, privacy).into());
    ui.set_balance_change(format_currency(cash_flow.net_delta, &base_currency, privacy).into());
    ui.set_savings_rate(format!("{}%", cash_flow.savings_rate).into());

    let recent: Vec<TransactionItem> = all_txs.iter().take(8).map(|item| map_tx_item(item, privacy)).collect();
    ui.set_recent_transactions(ModelRc::from(Rc::new(VecModel::from(recent))));

    // Build filter for all-transactions respecting active search query and filter tab
    let tx_tab = ui.get_tx_filter_type();
    let query = ui.get_tx_search_query();

    let tx_type = match tx_tab {
        1 => Some(TransactionType::Expense),
        2 => Some(TransactionType::Income),
        3 => Some(TransactionType::Transfer),
        _ => None,
    };

    let active_filter = TransactionFilter {
        tx_type,
        search: if query.trim().is_empty() { None } else { Some(query.to_string()) },
        limit: Some(150),
        ..Default::default()
    };

    let display_txs = if tx_tab != 0 || !query.trim().is_empty() {
        db.list_transactions(&active_filter).unwrap_or_default()
    } else {
        all_txs.clone()
    };

    let all_items: Vec<TransactionItem> = display_txs.iter().map(|item| map_tx_item(item, privacy)).collect();
    ui.set_all_transactions(ModelRc::from(Rc::new(VecModel::from(all_items.clone()))));

    (all_items, all_txs)
}

/// Refreshes all metrics, calculations, and list models across all tabs.
pub fn refresh_ui(ui: &MainWindow, db: &Database) {
    let privacy = ui.get_privacy_mode();
    let base_currency = db
        .get_setting("base_currency")
        .ok()
        .flatten()
        .unwrap_or_else(|| "BDT".to_string());
    ui.set_base_currency(base_currency.clone().into());

    ui.set_db_storage_path(db.path().to_string_lossy().to_string().into());
    let export_path = Database::get_configured_export_path();
    ui.set_export_storage_path(export_path.to_string_lossy().to_string().into());
    let date_fmt = db
        .get_setting("date_format")
        .ok()
        .flatten()
        .unwrap_or_else(|| "YYYY-MM-DD".to_string());
    ui.set_date_format(date_fmt.into());

    let current_date = Utc::now().date_naive();
    let current_month = Utc::now().format("%Y-%m").to_string();

    // 1. Accounts
    refresh_accounts(ui, db);

    // 2. Transactions & Cash Flow Metrics
    let (_, all_txs) = refresh_transactions(ui, db);

    // 3. Budgets & Dynamic Safe-to-Spend
    let budget_progress = db.list_budgets_with_progress().unwrap_or_default();
    let safe_result = calculate_safe_to_spend(&budget_progress, current_date);

    ui.set_safe_to_spend(format_currency(safe_result.daily_safe_to_spend, &base_currency, privacy).into());
    ui.set_safe_days_left(safe_result.days_left_in_month as i32);

    let budget_items: Vec<BudgetItem> = safe_result
        .budget_pacing
        .into_iter()
        .map(|bp| BudgetItem {
            id: bp.budget_id.into(),
            name: bp.budget_name.into(),
            amount_raw: bp.amount.to_string().into(),
            spent: format_currency(bp.spent, &base_currency, privacy).into(),
            total: format_currency(bp.amount, &base_currency, privacy).into(),
            percentage: bp.spent_percentage as f32,
            daily_safe: format_currency(bp.daily_safe_to_spend, &base_currency, privacy).into(),
            is_over: bp.is_over_budget,
            color: bp.category_color.unwrap_or_else(|| "#10B981".into()).into(),
            category_id: "".into(),
            category_name: bp.category_name.unwrap_or_else(|| "General".into()).into(),
        })
        .collect();
    ui.set_budgets(ModelRc::from(Rc::new(VecModel::from(budget_items))));

    // 4. Recurring Bills & Subscriptions
    let bills = db.list_bills(false).unwrap_or_default();
    let bill_items: Vec<BillItem> = bills
        .into_iter()
        .map(|b| {
            let bill = b.bill;
            BillItem {
                id: bill.id.into(),
                name: bill.name.into(),
                amount_raw: bill.amount.to_string().into(),
                due_date: bill.next_date.into(),
                amount: format_currency(bill.amount, &base_currency, privacy).into(),
                is_expense: bill.bill_type == TransactionType::Expense,
                frequency: bill.frequency.as_str().into(),
                color: b.category_color.unwrap_or_else(|| "#3B82F6".into()).into(),
                account_id: bill.account_id.into(),
                account_name: b.account_name.unwrap_or_default().into(),
                category_id: bill.category_id.unwrap_or_default().into(),
                payee: bill.payee.unwrap_or_default().into(),
            }
        })
        .collect();
    ui.set_bills(ModelRc::from(Rc::new(VecModel::from(bill_items))));

    // 5. Debts & Loans
    refresh_debts(ui, db);

    // 6. Category Breakdown for Reports View
    let breakdown = calculate_category_breakdown(&all_txs, &current_month);
    let breakdown_items: Vec<CategorySpendingItem> = breakdown
        .into_iter()
        .map(|cat| CategorySpendingItem {
            name: cat.category_name.into(),
            color: cat.category_color.into(),
            total: format_currency(cat.total, &base_currency, privacy).into(),
            percentage: cat.percentage as f32,
        })
        .collect();
    ui.set_category_breakdown(ModelRc::from(Rc::new(VecModel::from(breakdown_items))));

    // 7. Categories for Modals & Settings Catalog
    refresh_categories(ui, db);
}
