use rusqlite::{params, Connection};
use chrono::Utc;
use crate::db::error::DbResult;

const MIGRATION_1: &str = r#"
CREATE TABLE IF NOT EXISTS accounts (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    type TEXT NOT NULL,
    currency TEXT NOT NULL DEFAULT 'BDT',
    balance REAL NOT NULL DEFAULT 0.0,
    color TEXT NOT NULL DEFAULT '#10B981',
    icon TEXT NOT NULL DEFAULT 'Wallet',
    is_archived INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS categories (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    type TEXT NOT NULL,
    icon TEXT NOT NULL DEFAULT 'CircleDollarSign',
    color TEXT NOT NULL DEFAULT '#64748B',
    parent_id TEXT,
    created_at TEXT NOT NULL,
    FOREIGN KEY (parent_id) REFERENCES categories(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS transactions (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL,
    to_account_id TEXT,
    category_id TEXT,
    type TEXT NOT NULL,
    amount REAL NOT NULL,
    currency TEXT NOT NULL DEFAULT 'BDT',
    date TEXT NOT NULL,
    time TEXT,
    payee TEXT,
    note TEXT,
    status TEXT NOT NULL DEFAULT 'cleared',
    payment_type TEXT NOT NULL DEFAULT 'cash',
    created_at TEXT NOT NULL,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
    FOREIGN KEY (to_account_id) REFERENCES accounts(id) ON DELETE SET NULL,
    FOREIGN KEY (category_id) REFERENCES categories(id) ON DELETE RESTRICT
);

CREATE TABLE IF NOT EXISTS budgets (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    amount REAL NOT NULL,
    period TEXT NOT NULL DEFAULT 'monthly',
    category_id TEXT,
    category_ids TEXT,
    start_date TEXT NOT NULL,
    end_date TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY (category_id) REFERENCES categories(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS bills_subscriptions (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    amount REAL NOT NULL,
    type TEXT NOT NULL DEFAULT 'expense',
    account_id TEXT NOT NULL,
    category_id TEXT,
    payee TEXT,
    frequency TEXT NOT NULL DEFAULT 'monthly',
    next_date TEXT NOT NULL,
    auto_record INTEGER NOT NULL DEFAULT 0,
    is_active INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE,
    FOREIGN KEY (category_id) REFERENCES categories(id) ON DELETE SET NULL
);

CREATE TABLE IF NOT EXISTS debts (
    id TEXT PRIMARY KEY,
    type TEXT NOT NULL,
    person_name TEXT NOT NULL,
    amount REAL NOT NULL,
    settled_amount REAL NOT NULL DEFAULT 0.0,
    due_date TEXT,
    notes TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_transactions_date ON transactions(date DESC, time DESC, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_transactions_account ON transactions(account_id);
CREATE INDEX IF NOT EXISTS idx_transactions_to_account ON transactions(to_account_id);
CREATE INDEX IF NOT EXISTS idx_transactions_category ON transactions(category_id);
CREATE INDEX IF NOT EXISTS idx_budgets_category ON budgets(category_id);
CREATE INDEX IF NOT EXISTS idx_bills_next_date ON bills_subscriptions(next_date);
CREATE INDEX IF NOT EXISTS idx_debts_status ON debts(status);
"#;

pub fn run_migrations(conn: &mut Connection) -> DbResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            name TEXT NOT NULL,
            applied_at TEXT NOT NULL
        );"
    )?;

    let applied: Vec<i32> = {
        let mut stmt = conn.prepare("SELECT version FROM schema_migrations ORDER BY version ASC")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let mut versions = Vec::new();
        for r in rows {
            versions.push(r?);
        }
        versions
    };

    if !applied.contains(&1) {
        let tx = conn.transaction()?;
        tx.execute_batch(MIGRATION_1)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
            params![1, "initial_single_user_schema", Utc::now().to_rfc3339()],
        )?;
        tx.commit()?;
    }

    // Migration 2: Drop multi-user tables (sessions, users) & migrate legacy planned_payments if existing
    if !applied.contains(&2) {
        let tx = conn.transaction()?;
        
        // Remove legacy multi-user RBAC tables
        tx.execute_batch("DROP TABLE IF EXISTS sessions; DROP TABLE IF EXISTS users;")?;

        // Check if legacy planned_payments exists and migrate to bills_subscriptions
        let has_legacy_planned: bool = tx
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='planned_payments'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .map(|cnt| cnt > 0)
            .unwrap_or(false);

        if has_legacy_planned {
            tx.execute_batch(
                "INSERT OR IGNORE INTO bills_subscriptions (
                    id, name, amount, type, account_id, category_id, payee, frequency, next_date, auto_record, is_active, created_at
                )
                SELECT id, name, amount, type, account_id, category_id, payee, frequency, next_date, auto_record, is_active, created_at
                FROM planned_payments;
                DROP TABLE planned_payments;"
            )?;
        }

        tx.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
            params![2, "drop_legacy_multiuser_and_migrate_bills", Utc::now().to_rfc3339()],
        )?;
        tx.commit()?;
    }

    // Migration 3: Add category_ids column to budgets table for multiple category tracking
    if !applied.contains(&3) {
        let tx = conn.transaction()?;
        let has_category_ids: bool = tx
            .prepare("PRAGMA table_info(budgets)")?
            .query_map([], |row| row.get::<_, String>(1))?
            .filter_map(|r| r.ok())
            .any(|col| col == "category_ids");

        if !has_category_ids {
            tx.execute_batch("ALTER TABLE budgets ADD COLUMN category_ids TEXT;")?;
        }

        tx.execute(
            "INSERT INTO schema_migrations (version, name, applied_at) VALUES (?1, ?2, ?3)",
            params![3, "add_budget_category_ids", Utc::now().to_rfc3339()],
        )?;
        tx.commit()?;
    }

    Ok(())
}

pub fn seed_initial_data(conn: &Connection) -> DbResult<()> {
    let now = Utc::now().to_rfc3339();

    let tx = conn.unchecked_transaction()?;

    // 1. Default Settings
    let default_settings = [
        ("base_currency", "BDT"),
        ("theme", "dark"),
        ("privacy_mode", "false"),
        ("date_format", "YYYY-MM-DD"),
        ("pin_enabled", "false"),
    ];

    for (k, v) in default_settings {
        tx.execute(
            "INSERT OR IGNORE INTO settings (key, value) VALUES (?1, ?2)",
            params![k, v],
        )?;
    }

    // 2. Default Categories
    let categories = [
        ("cat_groceries", "Groceries", "expense", "ShoppingBag", "#10B981"),
        ("cat_food_drinks", "Food & Drinks", "expense", "Utensils", "#F59E0B"),
        ("cat_shopping", "Shopping", "expense", "Shirt", "#8B5CF6"),
        ("cat_housing", "Housing & Rent", "expense", "Home", "#3B82F6"),
        ("cat_transport", "Transportation", "expense", "Car", "#06B6D4"),
        ("cat_vehicle", "Vehicle & Fuel", "expense", "Fuel", "#EC4899"),
        ("cat_entertainment", "Entertainment", "expense", "Film", "#F43F5E"),
        ("cat_communication", "Internet & Phone", "expense", "Wifi", "#6366F1"),
        ("cat_fitness", "Health & Fitness", "expense", "Activity", "#14B8A6"),
        ("cat_financial", "Financial Expenses", "expense", "Landmark", "#64748B"),
        ("cat_salary", "Salary", "income", "Briefcase", "#10B981"),
        ("cat_freelance", "Freelance & Side Gig", "income", "Laptop", "#3B82F6"),
        ("cat_investments", "Investments & Dividends", "income", "TrendingUp", "#8B5CF6"),
        ("cat_gifts", "Gifts & Bonuses", "income", "Gift", "#EC4899"),
    ];

    for (id, name, cat_type, icon, color) in categories {
        tx.execute(
            "INSERT OR IGNORE INTO categories (id, name, type, icon, color, parent_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)",
            params![id, name, cat_type, icon, color, now],
        )?;
    }

    tx.commit()?;
    Ok(())
}

pub fn seed_demo_data(conn: &Connection) -> DbResult<()> {
    let now = Utc::now().to_rfc3339();
    let year_month = Utc::now().format("%Y-%m").to_string();

    let base_curr: String = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'base_currency'",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "BDT".to_string());

    let tx = conn.unchecked_transaction()?;

    // 1. Default Accounts
    let accounts = [
        ("acc_checking", "Main Checking (Chase)", "bank", base_curr.as_str(), 5240.50, "#3B82F6", "CreditCard"),
        ("acc_cash", "Cash Wallet", "cash", base_curr.as_str(), 260.00, "#10B981", "Wallet"),
        ("acc_savings", "Emergency Fund", "savings", base_curr.as_str(), 14500.00, "#06B6D4", "PiggyBank"),
        ("acc_credit", "Amex Gold Card", "credit_card", base_curr.as_str(), -385.40, "#F59E0B", "CreditCard"),
    ];

    for (id, name, acc_type, currency, balance, color, icon) in accounts {
        tx.execute(
            "INSERT OR IGNORE INTO accounts (id, name, type, currency, balance, color, icon, is_archived, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?8)",
            params![id, name, acc_type, currency, balance, color, icon, now],
        )?;
    }

    // 2. Default Budgets
    let start_of_month = format!("{}-01", year_month);
    let end_of_month = format!("{}-31", year_month);

    let budgets = [
        ("b_food", "Food & Dining Out", 450.0, "monthly", "cat_food_drinks"),
        ("b_groceries", "Monthly Groceries", 500.0, "monthly", "cat_groceries"),
        ("b_shopping", "Discretionary Shopping", 250.0, "monthly", "cat_shopping"),
        ("b_transport", "Commute & Fuel", 180.0, "monthly", "cat_transport"),
    ];

    for (id, name, amount, period, category_id) in budgets {
        tx.execute(
            "INSERT OR IGNORE INTO budgets (id, name, amount, period, category_id, start_date, end_date, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![id, name, amount, period, category_id, start_of_month, end_of_month, now],
        )?;
    }

    // 3. Default Recurring Bills & Subscriptions
    let bills = [
        ("plan_rent", "Apartment Rent", 1250.0, "expense", "acc_checking", Some("cat_housing"), "Avalon Properties", "monthly", format!("{}-01", year_month)),
        ("plan_net_spo", "Streaming (Netflix & Spotify)", 27.98, "expense", "acc_credit", Some("cat_entertainment"), "Media Subscriptions", "monthly", format!("{}-15", year_month)),
        ("plan_wifi", "Fiber Gigabit Internet", 69.99, "expense", "acc_checking", Some("cat_communication"), "AT&T Fiber", "monthly", format!("{}-28", year_month)),
        ("plan_salary", "Monthly Base Salary", 4800.0, "income", "acc_checking", Some("cat_salary"), "Tech Corp Inc", "monthly", format!("{}-01", year_month)),
    ];

    for (id, name, amount, b_type, acc_id, cat_id, payee, freq, next_date) in bills {
        tx.execute(
            "INSERT OR IGNORE INTO bills_subscriptions (id, name, amount, type, account_id, category_id, payee, frequency, next_date, auto_record, is_active, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1, 1, ?10)",
            params![id, name, amount, b_type, acc_id, cat_id, payee, freq, next_date, now],
        )?;
    }

    // 4. Default Debts & Loans
    let debts = [
        ("d_1", "loan_owed_to_me", "Sayem Ahamed", 160.0, 60.0, Some(format!("{}-30", year_month)), "Concert tickets upfront cost", "active"),
        ("d_2", "debt_i_owe", "Sarah Miller", 45.0, 0.0, Some(format!("{}-25", year_month)), "Group dinner bill split", "active"),
    ];

    for (id, d_type, person, amount, settled, due, notes, status) in debts {
        tx.execute(
            "INSERT OR IGNORE INTO debts (id, type, person_name, amount, settled_amount, due_date, notes, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![id, d_type, person, amount, settled, due, notes, status, now],
        )?;
    }

    // 5. Seed Transactions
    let curr_ref = base_curr.as_str();
    let txs = [
        ("tx_inc_1", "acc_checking", None, Some("cat_salary"), "income", 4800.0, curr_ref, format!("{}-01", year_month), Some("09:00"), Some("Tech Corp Inc"), Some("Monthly Salary direct deposit"), "cleared", "transfer"),
        ("tx_inc_2", "acc_checking", None, Some("cat_freelance"), "income", 650.0, curr_ref, format!("{}-12", year_month), Some("14:20"), Some("Client: FinTech App"), Some("UI design consulting milestone 1"), "cleared", "transfer"),
        ("tx_inc_3", "acc_savings", None, Some("cat_investments"), "income", 48.75, curr_ref, format!("{}-15", year_month), Some("11:00"), Some("HYSA Yield"), Some("Interest payment 4.5% APY"), "cleared", "transfer"),
        ("tx_exp_1", "acc_checking", None, Some("cat_housing"), "expense", 1250.0, curr_ref, format!("{}-01", year_month), Some("10:15"), Some("Avalon Properties"), Some("Rent payment"), "cleared", "transfer"),
        ("tx_exp_2", "acc_checking", None, Some("cat_groceries"), "expense", 132.40, curr_ref, format!("{}-03", year_month), Some("17:30"), Some("Whole Foods Market"), Some("Weekly grocery run"), "cleared", "debit_card"),
        ("tx_exp_3", "acc_credit", None, Some("cat_groceries"), "expense", 94.60, curr_ref, format!("{}-10", year_month), Some("18:45"), Some("Trader Joe's"), Some("Produce and dairy"), "cleared", "credit_card"),
        ("tx_exp_4", "acc_checking", None, Some("cat_groceries"), "expense", 115.20, curr_ref, format!("{}-17", year_month), Some("16:10"), Some("Costco Wholesale"), Some("Household supplies"), "cleared", "debit_card"),
        ("tx_exp_5", "acc_cash", None, Some("cat_food_drinks"), "expense", 18.50, curr_ref, format!("{}-04", year_month), Some("12:45"), Some("Blue Bottle Coffee"), Some("Espresso & toast"), "cleared", "cash"),
        ("tx_exp_6", "acc_credit", None, Some("cat_food_drinks"), "expense", 68.30, curr_ref, format!("{}-08", year_month), Some("20:15"), Some("Bella Italia Trattoria"), Some("Dinner with colleagues"), "cleared", "credit_card"),
        ("tx_exp_7", "acc_credit", None, Some("cat_food_drinks"), "expense", 34.00, curr_ref, format!("{}-14", year_month), Some("19:00"), Some("Chipotle Mexican Grill"), Some("Takeout dinner"), "cleared", "credit_card"),
        ("tx_exp_8", "acc_cash", None, Some("cat_food_drinks"), "expense", 22.50, curr_ref, format!("{}-21", year_month), Some("13:10"), Some("Street Taco Stand"), Some("Lunch with friend"), "cleared", "cash"),
        ("tx_exp_9", "acc_credit", None, Some("cat_vehicle"), "expense", 52.00, curr_ref, format!("{}-05", year_month), Some("08:30"), Some("Shell Gas Station"), Some("Full tank fuel"), "cleared", "credit_card"),
        ("tx_exp_10", "acc_checking", None, Some("cat_transport"), "expense", 35.00, curr_ref, format!("{}-11", year_month), Some("09:20"), Some("Metro Transit Card"), Some("Monthly subway refill"), "cleared", "debit_card"),
        ("tx_exp_11", "acc_credit", None, Some("cat_transport"), "expense", 24.50, curr_ref, format!("{}-18", year_month), Some("23:15"), Some("Uber Technologies"), Some("Late night ride"), "cleared", "credit_card"),
        ("tx_exp_12", "acc_credit", None, Some("cat_shopping"), "expense", 79.99, curr_ref, format!("{}-09", year_month), Some("15:20"), Some("Uniqlo"), Some("Autumn jacket"), "cleared", "credit_card"),
        ("tx_exp_13", "acc_credit", None, Some("cat_shopping"), "expense", 45.00, curr_ref, format!("{}-16", year_month), Some("11:45"), Some("Amazon.com"), Some("Desk accessories"), "cleared", "credit_card"),
        ("tx_exp_14", "acc_credit", None, Some("cat_entertainment"), "expense", 27.98, curr_ref, format!("{}-15", year_month), Some("06:00"), Some("Netflix & Spotify"), Some("Monthly streaming"), "cleared", "credit_card"),
        ("tx_exp_15", "acc_checking", None, Some("cat_fitness"), "expense", 65.00, curr_ref, format!("{}-02", year_month), Some("07:00"), Some("Equinox Gym"), Some("Gym pass"), "cleared", "debit_card"),
        ("tx_trf_1", "acc_checking", Some("acc_savings"), None, "transfer", 500.00, curr_ref, format!("{}-02", year_month), Some("10:00"), None, Some("Savings auto-transfer"), "cleared", "transfer"),
        ("tx_trf_2", "acc_checking", Some("acc_cash"), None, "transfer", 150.00, curr_ref, format!("{}-07", year_month), Some("13:00"), None, Some("ATM Cash withdrawal"), "cleared", "cash"),
    ];

    for (id, acc_id, to_acc, cat_id, t_type, amount, curr, date, time, payee, note, status, pay_type) in txs {
        tx.execute(
            "INSERT OR IGNORE INTO transactions (
                id, account_id, to_account_id, category_id, type, amount, currency,
                date, time, payee, note, status, payment_type, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![id, acc_id, to_acc, cat_id, t_type, amount, curr, date, time, payee, note, status, pay_type, now],
        )?;
    }

    tx.commit()?;
    Ok(())
}

pub fn reset_and_seed_demo_data(conn: &Connection) -> DbResult<()> {
    conn.execute("PRAGMA foreign_keys = OFF", [])?;
    conn.execute("DELETE FROM transactions", [])?;
    conn.execute("DELETE FROM budgets", [])?;
    conn.execute("DELETE FROM bills_subscriptions", [])?;
    conn.execute("DELETE FROM debts", [])?;
    conn.execute("DELETE FROM accounts", [])?;
    conn.execute("DELETE FROM categories", [])?;
    conn.execute("PRAGMA foreign_keys = ON", [])?;
    seed_initial_data(conn)?;
    seed_demo_data(conn)?;
    Ok(())
}
