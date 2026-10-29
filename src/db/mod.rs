pub mod config;
pub mod error;
pub mod models;
pub mod repository;
pub mod schema;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use rusqlite::Connection;
use directories::{ProjectDirs, UserDirs};

use config::AppConfig;
pub use error::{DbError, DbResult};
pub use models::*;

struct DbInner {
    conn: Connection,
    path: PathBuf,
}

#[derive(Clone)]
pub struct Database {
    inner: Arc<Mutex<DbInner>>,
}

impl Database {
    /// Opens or creates a SQLite database at the specified path with WAL mode and foreign keys enabled.
    pub fn open(path: impl AsRef<Path>) -> DbResult<Self> {
        let path_ref = path.as_ref();
        if let Some(parent) = path_ref.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }

        let mut conn = Connection::open(path_ref)?;
        Self::configure_connection(&mut conn)?;
        schema::run_migrations(&mut conn)?;

        // Auto-seed if brand new database
        let is_empty: bool = conn
            .query_row("SELECT COUNT(*) FROM categories", [], |r| r.get::<_, i64>(0))
            .map(|cnt| cnt == 0)
            .unwrap_or(true);

        if is_empty {
            schema::seed_initial_data(&conn)?;
        }

        let inner = DbInner {
            conn,
            path: path_ref.to_path_buf(),
        };

        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    /// Opens an in-memory SQLite database, primarily for testing.
    pub fn open_in_memory() -> DbResult<Self> {
        let mut conn = Connection::open_in_memory()?;
        Self::configure_connection(&mut conn)?;
        schema::run_migrations(&mut conn)?;

        let inner = DbInner {
            conn,
            path: PathBuf::from(":memory:"),
        };

        Ok(Self {
            inner: Arc::new(Mutex::new(inner)),
        })
    }

    /// Opens the database in the configured path or standard OS local user data directory (or fallback `./data/wallet.db`).
    pub fn open_default() -> DbResult<Self> {
        let cfg = AppConfig::load();
        if let Some(custom) = cfg.database_path
            && !custom.trim().is_empty()
        {
            let custom_path = PathBuf::from(custom);
            if let Ok(db) = Self::open(&custom_path) {
                return Ok(db);
            }
        }
        let db_path = Self::get_default_db_path();
        Self::open(db_path)
    }

    pub fn path(&self) -> PathBuf {
        self.inner
            .lock()
            .map(|l| l.path.clone())
            .unwrap_or_else(|_| PathBuf::from("unknown"))
    }

    /// Resolves the default database path using standard OS application directory:
    /// e.g. `~/.local/share/urpwallet/wallet.db` on Linux,
    /// `C:\Users\<User>\AppData\Local\urpwallet\wallet.db` on Windows.
    pub fn get_default_db_path() -> PathBuf {
        if let Some(proj_dirs) = ProjectDirs::from("com", "urpwallet", "urpwallet") {
            let data_dir = proj_dirs.data_local_dir();
            data_dir.join("wallet.db")
        } else {
            PathBuf::from("data/wallet.db")
        }
    }

    /// Default export directory path: User's Downloads directory (or data/exports fallback)
    pub fn get_default_export_path() -> PathBuf {
        if let Some(user_dirs) = UserDirs::new() {
            if let Some(download_dir) = user_dirs.download_dir() {
                return download_dir.to_path_buf();
            }
        }
        if let Some(proj_dirs) = ProjectDirs::from("com", "urpwallet", "urpwallet") {
            proj_dirs.data_local_dir().join("exports")
        } else {
            PathBuf::from("data/exports")
        }
    }

    /// Configured export directory path
    pub fn get_configured_export_path() -> PathBuf {
        let cfg = AppConfig::load();
        if let Some(p) = cfg.export_path
            && !p.trim().is_empty()
        {
            let path = PathBuf::from(&p);
            // Ignore non-existent temporary directory paths (e.g. from prior test contamination)
            if path.exists() || (!p.contains("/tmp/") && !p.contains("\\Temp\\")) {
                return path;
            }
        }
        Self::get_default_export_path()
    }

    /// Set and persist configured export directory path
    pub fn set_configured_export_path(new_path: impl AsRef<Path>) -> Result<(), std::io::Error> {
        let p = new_path.as_ref();
        std::fs::create_dir_all(p)?;
        let mut cfg = AppConfig::load();
        cfg.export_path = Some(p.to_string_lossy().to_string());
        cfg.save()
    }

    /// Reset export directory path back to the default (User's Downloads directory)
    pub fn reset_default_export_path() -> Result<(), std::io::Error> {
        let mut cfg = AppConfig::load();
        cfg.export_path = None;
        cfg.save()
    }

    /// Switches the active database connection to a new path.
    /// If copy_existing is true and new_path does not exist, copies current database data to new_path.
    pub fn switch_database(&self, new_path: PathBuf, copy_existing: bool) -> DbResult<()> {
        let mut lock = self
            .inner
            .lock()
            .map_err(|e| DbError::Validation(format!("Database mutex poisoned: {e}")))?;

        if lock.path == new_path {
            return Ok(());
        }

        if let Some(parent) = new_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }

        if copy_existing && !new_path.exists() && lock.path.exists() && lock.path.to_string_lossy() != ":memory:" {
            let _ = lock.conn.pragma_update(None, "wal_checkpoint", "TRUNCATE");
            let _ = std::fs::copy(&lock.path, &new_path);
        }

        let mut new_conn = Connection::open(&new_path)?;
        Self::configure_connection(&mut new_conn)?;
        schema::run_migrations(&mut new_conn)?;

        let is_empty: bool = new_conn
            .query_row("SELECT COUNT(*) FROM categories", [], |r| r.get::<_, i64>(0))
            .map(|cnt| cnt == 0)
            .unwrap_or(true);

        if is_empty {
            schema::seed_initial_data(&new_conn)?;
        }

        lock.conn = new_conn;
        lock.path = new_path.clone();

        let mut cfg = AppConfig::load();
        cfg.database_path = Some(new_path.to_string_lossy().to_string());
        let _ = cfg.save();

        Ok(())
    }

    /// Resets database path back to system default.
    pub fn reset_default_database(&self) -> DbResult<()> {
        let default_path = Self::get_default_db_path();
        self.switch_database(default_path, false)?;

        let mut cfg = AppConfig::load();
        cfg.database_path = None;
        let _ = cfg.save();

        Ok(())
    }

    /// Configures connection pragmas (WAL mode, Foreign Keys, Normal synchronous, busy timeout).
    fn configure_connection(conn: &mut Connection) -> DbResult<()> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        Ok(())
    }

    /// Gives raw access to the underlying connection under a mutex lock.
    pub fn with_conn<F, R>(&self, f: F) -> DbResult<R>
    where
        F: FnOnce(&mut Connection) -> DbResult<R>,
    {
        let mut lock = self
            .inner
            .lock()
            .map_err(|e| DbError::Validation(format!("Database mutex poisoned: {e}")))?;
        f(&mut lock.conn)
    }

    // ------------------------------------------------------------------------
    // Accounts
    // ------------------------------------------------------------------------

    pub fn create_account(&self, input: CreateAccountInput) -> DbResult<Account> {
        self.with_conn(|conn| repository::accounts::create_account(conn, input))
    }

    pub fn get_account(&self, id: &str) -> DbResult<Account> {
        self.with_conn(|conn| repository::accounts::get_account(conn, id))
    }

    pub fn list_accounts(&self, include_archived: bool) -> DbResult<Vec<Account>> {
        self.with_conn(|conn| repository::accounts::list_accounts(conn, include_archived))
    }

    pub fn update_account(&self, id: &str, input: UpdateAccountInput) -> DbResult<Account> {
        self.with_conn(|conn| repository::accounts::update_account(conn, id, input))
    }

    pub fn delete_account(&self, id: &str) -> DbResult<()> {
        self.with_conn(|conn| repository::accounts::delete_account(conn, id))
    }

    // ------------------------------------------------------------------------
    // Categories
    // ------------------------------------------------------------------------

    pub fn create_category(&self, input: CreateCategoryInput) -> DbResult<Category> {
        self.with_conn(|conn| repository::categories::create_category(conn, input))
    }

    pub fn get_category(&self, id: &str) -> DbResult<Category> {
        self.with_conn(|conn| repository::categories::get_category(conn, id))
    }

    pub fn list_categories(&self, cat_type: Option<CategoryType>) -> DbResult<Vec<Category>> {
        self.with_conn(|conn| repository::categories::list_categories(conn, cat_type))
    }

    pub fn list_categories_with_counts(&self) -> DbResult<Vec<(Category, i64)>> {
        self.with_conn(|conn| repository::categories::list_categories_with_counts(conn))
    }

    pub fn update_category(&self, id: &str, input: UpdateCategoryInput) -> DbResult<Category> {
        self.with_conn(|conn| repository::categories::update_category(conn, id, input))
    }

    pub fn delete_category(&self, id: &str) -> DbResult<()> {
        self.with_conn(|conn| repository::categories::delete_category(conn, id))
    }

    // ------------------------------------------------------------------------
    // Transactions
    // ------------------------------------------------------------------------

    pub fn create_transaction(&self, input: CreateTransactionInput) -> DbResult<TransactionWithDetails> {
        self.with_conn(|conn| repository::transactions::create_transaction(conn, input))
    }

    pub fn get_transaction(&self, id: &str) -> DbResult<TransactionWithDetails> {
        self.with_conn(|conn| repository::transactions::get_transaction(conn, id))
    }

    pub fn list_transactions(&self, filter: &TransactionFilter) -> DbResult<Vec<TransactionWithDetails>> {
        self.with_conn(|conn| repository::transactions::list_transactions(conn, filter))
    }

    pub fn update_transaction(&self, id: &str, input: UpdateTransactionInput) -> DbResult<TransactionWithDetails> {
        self.with_conn(|conn| repository::transactions::update_transaction(conn, id, input))
    }

    pub fn delete_transaction(&self, id: &str) -> DbResult<()> {
        self.with_conn(|conn| repository::transactions::delete_transaction(conn, id))
    }

    pub fn delete_transactions_bulk(&self, ids: &[String]) -> DbResult<usize> {
        self.with_conn(|conn| repository::transactions::delete_transactions_bulk(conn, ids))
    }

    // ------------------------------------------------------------------------
    // Budgets
    // ------------------------------------------------------------------------

    pub fn create_budget(&self, input: CreateBudgetInput) -> DbResult<Budget> {
        self.with_conn(|conn| repository::budgets::create_budget(conn, input))
    }

    pub fn get_budget(&self, id: &str) -> DbResult<Budget> {
        self.with_conn(|conn| repository::budgets::get_budget(conn, id))
    }

    pub fn list_budgets(&self) -> DbResult<Vec<Budget>> {
        self.with_conn(|conn| repository::budgets::list_budgets(conn))
    }

    pub fn list_budgets_with_progress(&self) -> DbResult<Vec<BudgetWithProgress>> {
        self.with_conn(|conn| repository::budgets::list_budgets_with_progress(conn))
    }

    pub fn update_budget(&self, id: &str, input: UpdateBudgetInput) -> DbResult<Budget> {
        self.with_conn(|conn| repository::budgets::update_budget(conn, id, input))
    }

    pub fn delete_budget(&self, id: &str) -> DbResult<()> {
        self.with_conn(|conn| repository::budgets::delete_budget(conn, id))
    }

    // ------------------------------------------------------------------------
    // Recurring Bills & Subscriptions
    // ------------------------------------------------------------------------

    pub fn create_bill(&self, input: CreateBillInput) -> DbResult<BillSubscription> {
        self.with_conn(|conn| repository::bills::create_bill(conn, input))
    }

    pub fn get_bill(&self, id: &str) -> DbResult<BillSubscription> {
        self.with_conn(|conn| repository::bills::get_bill(conn, id))
    }

    pub fn list_bills(&self, active_only: bool) -> DbResult<Vec<BillWithDetails>> {
        self.with_conn(|conn| repository::bills::list_bills(conn, active_only))
    }

    pub fn update_bill(&self, id: &str, input: UpdateBillInput) -> DbResult<BillSubscription> {
        self.with_conn(|conn| repository::bills::update_bill(conn, id, input))
    }

    pub fn delete_bill(&self, id: &str) -> DbResult<()> {
        self.with_conn(|conn| repository::bills::delete_bill(conn, id))
    }

    pub fn execute_bill_payment(&self, id: &str) -> DbResult<TransactionWithDetails> {
        self.with_conn(|conn| repository::bills::execute_bill_payment(conn, id))
    }

    // ------------------------------------------------------------------------
    // Debts & Loans
    // ------------------------------------------------------------------------

    pub fn create_debt(&self, input: CreateDebtInput) -> DbResult<Debt> {
        self.with_conn(|conn| repository::debts::create_debt(conn, input))
    }

    pub fn get_debt(&self, id: &str) -> DbResult<Debt> {
        self.with_conn(|conn| repository::debts::get_debt(conn, id))
    }

    pub fn list_debts(&self, active_only: bool) -> DbResult<Vec<Debt>> {
        self.with_conn(|conn| repository::debts::list_debts(conn, active_only))
    }

    pub fn update_debt(&self, id: &str, input: UpdateDebtInput) -> DbResult<Debt> {
        self.with_conn(|conn| repository::debts::update_debt(conn, id, input))
    }

    pub fn delete_debt(&self, id: &str) -> DbResult<()> {
        self.with_conn(|conn| repository::debts::delete_debt(conn, id))
    }

    pub fn record_debt_payment(&self, id: &str, amount: f64, account_id: Option<&str>) -> DbResult<Debt> {
        self.with_conn(|conn| repository::debts::record_debt_payment(conn, id, amount, account_id))
    }

    // ------------------------------------------------------------------------
    // Settings & Master PIN Security
    // ------------------------------------------------------------------------

    pub fn get_setting(&self, key: &str) -> DbResult<Option<String>> {
        self.with_conn(|conn| repository::settings::get_setting(conn, key))
    }

    pub fn set_setting(&self, key: &str, value: &str) -> DbResult<()> {
        self.with_conn(|conn| repository::settings::set_setting(conn, key, value))
    }

    pub fn get_all_settings(&self) -> DbResult<Vec<(String, String)>> {
        self.with_conn(|conn| repository::settings::get_all_settings(conn))
    }

    pub fn is_pin_enabled(&self) -> DbResult<bool> {
        self.with_conn(|conn| repository::settings::is_pin_enabled(conn))
    }

    pub fn set_master_pin(&self, pin: &str) -> DbResult<()> {
        self.with_conn(|conn| repository::settings::set_master_pin(conn, pin))
    }

    pub fn verify_master_pin(&self, pin: &str) -> DbResult<bool> {
        self.with_conn(|conn| repository::settings::verify_master_pin(conn, pin))
    }

    pub fn disable_master_pin(&self, current_pin: &str) -> DbResult<()> {
        self.with_conn(|conn| repository::settings::disable_master_pin(conn, current_pin))
    }

    // ------------------------------------------------------------------------
    // Backup & Restore
    // ------------------------------------------------------------------------

    pub fn export_full_backup(&self) -> DbResult<FullBackupData> {
        self.with_conn(|conn| repository::export_import::export_full_backup(conn))
    }

    pub fn export_transactions_csv(&self) -> DbResult<String> {
        self.with_conn(|conn| repository::export_import::export_transactions_csv(conn))
    }

    pub fn import_full_backup(&self, backup: &FullBackupData) -> DbResult<()> {
        self.with_conn(|conn| repository::export_import::import_full_backup(conn, backup))
    }

    pub fn seed(&self) -> DbResult<()> {
        self.with_conn(|conn| schema::seed_initial_data(conn))
    }

    pub fn seed_demo(&self) -> DbResult<()> {
        self.with_conn(|conn| schema::seed_demo_data(conn))
    }

    pub fn reset_and_seed_demo(&self) -> DbResult<()> {
        self.with_conn(|conn| schema::reset_and_seed_demo_data(conn))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_db_migrations_and_foreign_keys() {
        let db = Database::open_in_memory().expect("Failed to open in-memory database");

        // Verify foreign key enforcement
        let res = db.create_transaction(CreateTransactionInput {
            id: None,
            account_id: "non_existent_account".into(),
            to_account_id: None,
            category_id: None,
            tx_type: TransactionType::Expense,
            amount: 50.0,
            currency: Some("USD".into()),
            date: None,
            time: None,
            payee: None,
            note: None,
            status: None,
            payment_type: None,
        });

        assert!(res.is_err(), "Expected foreign key violation for non-existent account");
    }

    #[test]
    fn test_account_crud() {
        let db = Database::open_in_memory().unwrap();

        let acc = db.create_account(CreateAccountInput {
            id: Some("acc_test".into()),
            name: "Savings Vault".into(),
            account_type: AccountType::Savings,
            currency: Some("EUR".into()),
            balance: Some(1000.0),
            color: Some("#00FF00".into()),
            icon: Some("PiggyBank".into()),
        }).unwrap();

        assert_eq!(acc.id, "acc_test");
        assert_eq!(acc.balance, 1000.0);
        assert_eq!(acc.currency, "EUR");

        // Verify default currency when None
        let default_curr_acc = db.create_account(CreateAccountInput {
            id: Some("acc_default_curr".into()),
            name: "Default Currency Acc".into(),
            account_type: AccountType::Cash,
            currency: None,
            balance: Some(500.0),
            color: None,
            icon: None,
        }).unwrap();
        assert_eq!(default_curr_acc.currency, "BDT");
        db.delete_account("acc_default_curr").unwrap();

        // Update
        let updated = db.update_account("acc_test", UpdateAccountInput {
            name: Some("High Yield Savings".into()),
            balance: Some(1500.0),
            ..Default::default()
        }).unwrap();

        assert_eq!(updated.name, "High Yield Savings");
        assert_eq!(updated.balance, 1500.0);

        // List
        let list = db.list_accounts(false).unwrap();
        assert_eq!(list.len(), 1);

        // Delete
        db.delete_account("acc_test").unwrap();
        assert!(db.get_account("acc_test").is_err());
    }

    #[test]
    fn test_category_crud_and_cascade_safety() {
        let db = Database::open_in_memory().unwrap();

        let cat = db.create_category(CreateCategoryInput {
            id: Some("cat_books".into()),
            name: "Books & Learning".into(),
            category_type: CategoryType::Expense,
            icon: Some("Book".into()),
            color: Some("#336699".into()),
            parent_id: None,
        }).unwrap();

        assert_eq!(cat.id, "cat_books");

        let acc = db.create_account(CreateAccountInput {
            id: Some("acc_card".into()),
            name: "Card".into(),
            account_type: AccountType::CreditCard,
            currency: None,
            balance: Some(500.0),
            color: None,
            icon: None,
        }).unwrap();

        // Create transaction using this category
        db.create_transaction(CreateTransactionInput {
            id: None,
            account_id: acc.id.clone(),
            to_account_id: None,
            category_id: Some(cat.id.clone()),
            tx_type: TransactionType::Expense,
            amount: 40.0,
            currency: None,
            date: None,
            time: None,
            payee: Some("Bookstore".into()),
            note: None,
            status: None,
            payment_type: None,
        }).unwrap();

        // Category deletion must fail because transaction references it
        let del_res = db.delete_category(&cat.id);
        assert!(del_res.is_err(), "Category deletion must fail when referenced by transactions");
    }

    #[test]
    fn test_transactions_atomic_balance_updates() {
        let db = Database::open_in_memory().unwrap();

        let acc1 = db.create_account(CreateAccountInput {
            id: Some("acc_1".into()),
            name: "Checking".into(),
            account_type: AccountType::Bank,
            currency: Some("USD".into()),
            balance: Some(1000.0),
            color: None,
            icon: None,
        }).unwrap();

        let acc2 = db.create_account(CreateAccountInput {
            id: Some("acc_2".into()),
            name: "Cash".into(),
            account_type: AccountType::Cash,
            currency: Some("USD".into()),
            balance: Some(100.0),
            color: None,
            icon: None,
        }).unwrap();

        // 1. Expense transaction
        let tx_exp = db.create_transaction(CreateTransactionInput {
            id: Some("tx_1".into()),
            account_id: acc1.id.clone(),
            to_account_id: None,
            category_id: None,
            tx_type: TransactionType::Expense,
            amount: 250.0,
            currency: None,
            date: Some("2026-10-01".into()),
            time: None,
            payee: Some("Electric Co".into()),
            note: None,
            status: None,
            payment_type: None,
        }).unwrap();

        assert_eq!(db.get_account(&acc1.id).unwrap().balance, 750.0);

        // 2. Transfer transaction
        let tx_trf = db.create_transaction(CreateTransactionInput {
            id: Some("tx_2".into()),
            account_id: acc1.id.clone(),
            to_account_id: Some(acc2.id.clone()),
            category_id: None,
            tx_type: TransactionType::Transfer,
            amount: 150.0,
            currency: None,
            date: Some("2026-10-02".into()),
            time: None,
            payee: None,
            note: None,
            status: None,
            payment_type: None,
        }).unwrap();

        assert_eq!(db.get_account(&acc1.id).unwrap().balance, 600.0);
        assert_eq!(db.get_account(&acc2.id).unwrap().balance, 250.0);

        // 3. Update transfer amount from 150 to 50
        db.update_transaction(&tx_trf.transaction.id, UpdateTransactionInput {
            amount: Some(50.0),
            ..Default::default()
        }).unwrap();

        assert_eq!(db.get_account(&acc1.id).unwrap().balance, 700.0);
        assert_eq!(db.get_account(&acc2.id).unwrap().balance, 150.0);

        // 4. Delete expense transaction (balance should restore)
        db.delete_transaction(&tx_exp.transaction.id).unwrap();
        assert_eq!(db.get_account(&acc1.id).unwrap().balance, 950.0);
    }

    #[test]
    fn test_delete_transactions_bulk() {
        let db = Database::open_in_memory().unwrap();

        let acc = db.create_account(CreateAccountInput {
            id: Some("acc_bulk_test".into()),
            name: "Bulk Test Account".into(),
            account_type: AccountType::Bank,
            currency: Some("USD".into()),
            balance: Some(1000.0),
            color: None,
            icon: None,
        }).unwrap();

        let tx1 = db.create_transaction(CreateTransactionInput {
            id: None,
            account_id: acc.id.clone(),
            to_account_id: None,
            category_id: None,
            tx_type: TransactionType::Expense,
            amount: 100.0,
            currency: None,
            date: Some("2026-10-20".into()),
            time: None,
            payee: None,
            note: None,
            status: None,
            payment_type: None,
        }).unwrap();

        let tx2 = db.create_transaction(CreateTransactionInput {
            id: None,
            account_id: acc.id.clone(),
            to_account_id: None,
            category_id: None,
            tx_type: TransactionType::Income,
            amount: 250.0,
            currency: None,
            date: Some("2026-10-21".into()),
            time: None,
            payee: None,
            note: None,
            status: None,
            payment_type: None,
        }).unwrap();

        // After tx1 (-100) and tx2 (+250), balance = 1150
        assert_eq!(db.get_account(&acc.id).unwrap().balance, 1150.0);

        // Bulk delete both transactions
        let ids = vec![tx1.transaction.id, tx2.transaction.id];
        let deleted = db.delete_transactions_bulk(&ids).unwrap();
        assert_eq!(deleted, 2);

        // Balance should be reverted back to initial 1000.0
        assert_eq!(db.get_account(&acc.id).unwrap().balance, 1000.0);
        assert!(db.get_transaction(&ids[0]).is_err());
        assert!(db.get_transaction(&ids[1]).is_err());
    }

    #[test]
    fn test_transaction_filtering_and_search() {
        let db = Database::open_in_memory().unwrap();

        let acc_bank = db.create_account(CreateAccountInput {
            id: Some("acc_bank".into()),
            name: "Silicon Valley Bank".into(),
            account_type: AccountType::Bank,
            currency: Some("USD".into()),
            balance: Some(1000.0),
            color: None,
            icon: None,
        }).unwrap();

        let acc_cash = db.create_account(CreateAccountInput {
            id: Some("acc_cash".into()),
            name: "Pocket Cash".into(),
            account_type: AccountType::Cash,
            currency: Some("USD".into()),
            balance: Some(200.0),
            color: None,
            icon: None,
        }).unwrap();

        let cat_groceries = db.create_category(CreateCategoryInput {
            id: Some("cat_groceries".into()),
            name: "Groceries & Supermarket".into(),
            category_type: CategoryType::Expense,
            icon: None,
            color: None,
            parent_id: None,
        }).unwrap();

        let cat_salary = db.create_category(CreateCategoryInput {
            id: Some("cat_salary".into()),
            name: "Job Salary".into(),
            category_type: CategoryType::Income,
            icon: None,
            color: None,
            parent_id: None,
        }).unwrap();

        // Transaction 1: Expense at Supermarket
        db.create_transaction(CreateTransactionInput {
            id: Some("tx_groceries".into()),
            account_id: acc_bank.id.clone(),
            to_account_id: None,
            category_id: Some(cat_groceries.id.clone()),
            tx_type: TransactionType::Expense,
            amount: 75.0,
            currency: None,
            date: Some("2026-10-05".into()),
            time: None,
            payee: Some("Whole Foods".into()),
            note: Some("Weekly organic supplies".into()),
            status: None,
            payment_type: None,
        }).unwrap();

        // Transaction 2: Income
        db.create_transaction(CreateTransactionInput {
            id: Some("tx_income".into()),
            account_id: acc_bank.id.clone(),
            to_account_id: None,
            category_id: Some(cat_salary.id.clone()),
            tx_type: TransactionType::Income,
            amount: 3000.0,
            currency: None,
            date: Some("2026-10-01".into()),
            time: None,
            payee: Some("Tech Corp Employer".into()),
            note: Some("Monthly direct deposit".into()),
            status: None,
            payment_type: None,
        }).unwrap();

        // Transaction 3: Transfer Bank -> Cash
        db.create_transaction(CreateTransactionInput {
            id: Some("tx_atm".into()),
            account_id: acc_bank.id.clone(),
            to_account_id: Some(acc_cash.id.clone()),
            category_id: None,
            tx_type: TransactionType::Transfer,
            amount: 100.0,
            currency: None,
            date: Some("2026-10-06".into()),
            time: None,
            payee: Some("ATM Withdrawal".into()),
            note: Some("Weekend pocket money".into()),
            status: None,
            payment_type: None,
        }).unwrap();

        // Filter: Expense only
        let exp_txs = db.list_transactions(&TransactionFilter {
            tx_type: Some(TransactionType::Expense),
            ..Default::default()
        }).unwrap();
        assert_eq!(exp_txs.len(), 1);
        assert_eq!(exp_txs[0].transaction.id, "tx_groceries");

        // Filter: Income only
        let inc_txs = db.list_transactions(&TransactionFilter {
            tx_type: Some(TransactionType::Income),
            ..Default::default()
        }).unwrap();
        assert_eq!(inc_txs.len(), 1);
        assert_eq!(inc_txs[0].transaction.id, "tx_income");

        // Filter: Transfer only
        let trf_txs = db.list_transactions(&TransactionFilter {
            tx_type: Some(TransactionType::Transfer),
            ..Default::default()
        }).unwrap();
        assert_eq!(trf_txs.len(), 1);
        assert_eq!(trf_txs[0].transaction.id, "tx_atm");

        // Search by payee: "Whole Foods"
        let search_payee = db.list_transactions(&TransactionFilter {
            search: Some("Whole Foods".into()),
            ..Default::default()
        }).unwrap();
        assert_eq!(search_payee.len(), 1);
        assert_eq!(search_payee[0].transaction.id, "tx_groceries");

        // Search by note: "direct deposit"
        let search_note = db.list_transactions(&TransactionFilter {
            search: Some("direct deposit".into()),
            ..Default::default()
        }).unwrap();
        assert_eq!(search_note.len(), 1);
        assert_eq!(search_note[0].transaction.id, "tx_income");

        // Search by category: "Supermarket"
        let search_cat = db.list_transactions(&TransactionFilter {
            search: Some("Supermarket".into()),
            ..Default::default()
        }).unwrap();
        assert_eq!(search_cat.len(), 1);
        assert_eq!(search_cat[0].transaction.id, "tx_groceries");

        // Search by account name: "Silicon Valley Bank"
        let search_acc = db.list_transactions(&TransactionFilter {
            search: Some("Silicon Valley".into()),
            ..Default::default()
        }).unwrap();
        assert_eq!(search_acc.len(), 3);

        // Combined Filter + Search
        let combined = db.list_transactions(&TransactionFilter {
            tx_type: Some(TransactionType::Expense),
            search: Some("Silicon Valley".into()),
            ..Default::default()
        }).unwrap();
        assert_eq!(combined.len(), 1);
        assert_eq!(combined[0].transaction.id, "tx_groceries");
    }

    #[test]
    fn test_master_pin_security() {
        let db = Database::open_in_memory().unwrap();

        // Initially disabled
        assert!(!db.is_pin_enabled().unwrap());
        assert!(db.verify_master_pin("1234").unwrap());

        // Set master PIN
        db.set_master_pin("secure99").unwrap();
        assert!(db.is_pin_enabled().unwrap());

        // Check verification
        assert!(db.verify_master_pin("secure99").unwrap());
        assert!(!db.verify_master_pin("wrong_pass").unwrap());

        // Disable PIN
        assert!(db.disable_master_pin("wrong").is_err());
        assert!(db.disable_master_pin("secure99").is_ok());
        assert!(!db.is_pin_enabled().unwrap());
    }

    #[test]
    fn test_bill_execution_and_debt_payments() {
        let db = Database::open_in_memory().unwrap();

        let acc = db.create_account(CreateAccountInput {
            id: Some("acc_bills".into()),
            name: "Primary Bank".into(),
            account_type: AccountType::Bank,
            currency: Some("USD".into()),
            balance: Some(2000.0),
            color: None,
            icon: None,
        }).unwrap();

        let bill = db.create_bill(CreateBillInput {
            id: Some("bill_vpn".into()),
            name: "VPN Service".into(),
            amount: 10.0,
            bill_type: Some(TransactionType::Expense),
            account_id: acc.id.clone(),
            category_id: None,
            payee: Some("Proton".into()),
            frequency: Some(BillFrequency::Monthly),
            next_date: "2026-10-15".into(),
            auto_record: Some(false),
            is_active: Some(true),
        }).unwrap();

        // One-click payment execution
        let logged_tx = db.execute_bill_payment(&bill.id).unwrap();
        assert_eq!(logged_tx.transaction.amount, 10.0);
        assert_eq!(db.get_account(&acc.id).unwrap().balance, 1990.0);

        let updated_bill = db.get_bill(&bill.id).unwrap();
        assert_eq!(updated_bill.next_date, "2026-11-15");

        // Debt partial and full payment
        let debt = db.create_debt(CreateDebtInput {
            id: Some("debt_alex".into()),
            debt_type: DebtType::LoanOwedToMe,
            person_name: "Alex".into(),
            amount: 100.0,
            settled_amount: None,
            due_date: None,
            notes: None,
        }).unwrap();

        // Repay 40 with cash inflow into account
        let part_paid = db.record_debt_payment(&debt.id, 40.0, Some(&acc.id)).unwrap();
        assert_eq!(part_paid.settled_amount, 40.0);
        assert_eq!(part_paid.status, DebtStatus::Active);
        assert_eq!(db.get_account(&acc.id).unwrap().balance, 2030.0); // 1990 + 40

        // Repay remaining 60
        let full_paid = db.record_debt_payment(&debt.id, 60.0, None).unwrap();
        assert_eq!(full_paid.settled_amount, 100.0);
        assert_eq!(full_paid.status, DebtStatus::Settled);
    }

    #[test]
    fn test_budget_progress_calculation() {
        let db = Database::open_in_memory().unwrap();

        let acc = db.create_account(CreateAccountInput {
            id: Some("acc_b".into()),
            name: "Main Bank".into(),
            account_type: AccountType::Bank,
            currency: Some("USD".into()),
            balance: Some(5000.0),
            color: None,
            icon: None,
        }).unwrap();

        let cat = db.create_category(CreateCategoryInput {
            id: Some("cat_dining".into()),
            name: "Dining".into(),
            category_type: CategoryType::Expense,
            icon: None,
            color: None,
            parent_id: None,
        }).unwrap();

        let budget = db.create_budget(CreateBudgetInput {
            id: Some("b_dining".into()),
            name: "Monthly Dining".into(),
            amount: 300.0,
            period: BudgetPeriod::Monthly,
            category_id: Some(cat.id.clone()),
            start_date: "2026-10-01".into(),
            end_date: "2026-10-31".into(),
        }).unwrap();

        // Add 2 expense transactions
        db.create_transaction(CreateTransactionInput {
            id: None,
            account_id: acc.id.clone(),
            to_account_id: None,
            category_id: Some(cat.id.clone()),
            tx_type: TransactionType::Expense,
            amount: 75.0,
            currency: None,
            date: Some("2026-10-05".into()),
            time: None,
            payee: None,
            note: None,
            status: None,
            payment_type: None,
        }).unwrap();

        db.create_transaction(CreateTransactionInput {
            id: None,
            account_id: acc.id.clone(),
            to_account_id: None,
            category_id: Some(cat.id.clone()),
            tx_type: TransactionType::Expense,
            amount: 45.0,
            currency: None,
            date: Some("2026-10-12".into()),
            time: None,
            payee: None,
            note: None,
            status: None,
            payment_type: None,
        }).unwrap();

        let progress = db.list_budgets_with_progress().unwrap();
        assert_eq!(progress.len(), 1);
        let b = &progress[0];
        assert_eq!(b.budget.id, budget.id);
        assert_eq!(b.spent, 120.0);
        assert_eq!(b.remaining, 180.0);
        assert_eq!(b.spent_percentage, 40.0);
    }

    #[test]
    fn test_export_and_import() {
        let db = Database::open_in_memory().unwrap();
        db.seed().unwrap();
        db.seed_demo().unwrap();

        // Test CSV export
        let csv = db.export_transactions_csv().unwrap();
        assert!(csv.contains("id,date,time,type,amount,currency"));
        assert!(csv.contains("Groceries"));

        // Test JSON full backup export
        let backup = db.export_full_backup().unwrap();
        assert!(!backup.accounts.is_empty());
        assert!(!backup.categories.is_empty());
        assert!(!backup.transactions.is_empty());

        // Import into new database
        let db2 = Database::open_in_memory().unwrap();
        db2.import_full_backup(&backup).unwrap();

        let accs = db2.list_accounts(true).unwrap();
        assert_eq!(accs.len(), backup.accounts.len());

        let txs = db2.list_transactions(&TransactionFilter::default()).unwrap();
        assert_eq!(txs.len(), backup.transactions.len());
    }

    #[test]
    fn test_reset_and_seed_demo() {
        let db = Database::open_in_memory().unwrap();
        db.seed().unwrap();
        let accs_before = db.list_accounts(false).unwrap();
        assert!(accs_before.is_empty());

        db.reset_and_seed_demo().unwrap();
        let accs_after = db.list_accounts(false).unwrap();
        assert_eq!(accs_after.len(), 4);
        assert_eq!(db.get_setting("base_currency").unwrap(), Some("BDT".to_string()));
    }

    #[test]
    fn test_switch_database_and_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let db1_path = temp_dir.path().join("wallet1.db");
        let db2_path = temp_dir.path().join("wallet2.db");

        let db = Database::open(&db1_path).unwrap();
        assert_eq!(db.path(), db1_path);

        let initial_count = db.list_accounts(false).unwrap().len();

        // Create an account in db1
        let _ = db.create_account(CreateAccountInput {
            id: Some("acc_db1".into()),
            name: "Primary Checking".into(),
            account_type: AccountType::Bank,
            currency: Some("USD".into()),
            balance: Some(2500.0),
            color: Some("#10B981".into()),
            icon: Some("wallet".into()),
        }).unwrap();

        assert_eq!(db.list_accounts(false).unwrap().len(), initial_count + 1);

        // Switch to db2 with copy_existing = true
        db.switch_database(db2_path.clone(), true).unwrap();
        assert_eq!(db.path(), db2_path);
        assert!(db2_path.exists());

        // Accounts should have been copied over
        let accs = db.list_accounts(false).unwrap();
        assert_eq!(accs.len(), initial_count + 1);
        assert!(accs.iter().any(|a| a.name == "Primary Checking"));

        // Now create another account in db2
        let _ = db.create_account(CreateAccountInput {
            id: Some("acc_db2".into()),
            name: "Secondary Vault".into(),
            account_type: AccountType::Savings,
            currency: Some("USD".into()),
            balance: Some(500.0),
            color: Some("#3B82F6".into()),
            icon: Some("vault".into()),
        }).unwrap();
        assert_eq!(db.list_accounts(false).unwrap().len(), initial_count + 2);

        // Switch back to db1 without copying
        db.switch_database(db1_path.clone(), false).unwrap();
        assert_eq!(db.path(), db1_path);
        // db1 should still have initial_count + 1 accounts
        assert_eq!(db.list_accounts(false).unwrap().len(), initial_count + 1);
    }

    #[test]
    fn test_export_path_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let custom_export = temp_dir.path().join("my_custom_exports");

        Database::set_configured_export_path(&custom_export).unwrap();
        let loaded = Database::get_configured_export_path();
        assert_eq!(loaded, custom_export);
        assert!(custom_export.exists());

        // Test resetting back to default Downloads directory
        Database::reset_default_export_path().unwrap();
        let reset_path = Database::get_configured_export_path();
        assert_eq!(reset_path, Database::get_default_export_path());
    }

    #[test]
    fn test_default_export_path_is_downloads() {
        let default_path = Database::get_default_export_path();
        if let Some(user_dirs) = directories::UserDirs::new() {
            if let Some(download_dir) = user_dirs.download_dir() {
                assert_eq!(default_path, download_dir);
            }
        }
    }
}
