use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    Cash,
    Bank,
    CreditCard,
    Savings,
    Investment,
    Loan,
}

impl AccountType {
    pub fn as_str(&self) -> &'static str {
        match self {
            AccountType::Cash => "cash",
            AccountType::Bank => "bank",
            AccountType::CreditCard => "credit_card",
            AccountType::Savings => "savings",
            AccountType::Investment => "investment",
            AccountType::Loan => "loan",
        }
    }

    pub fn from_str_val(s: &str) -> Result<Self, String> {
        match s {
            "cash" => Ok(AccountType::Cash),
            "bank" => Ok(AccountType::Bank),
            "credit_card" => Ok(AccountType::CreditCard),
            "savings" => Ok(AccountType::Savings),
            "investment" => Ok(AccountType::Investment),
            "loan" => Ok(AccountType::Loan),
            other => Err(format!("Unknown account type: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub account_type: AccountType,
    pub currency: String,
    pub balance: f64,
    pub color: String,
    pub icon: String,
    pub is_archived: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateAccountInput {
    pub id: Option<String>,
    pub name: String,
    pub account_type: AccountType,
    pub currency: Option<String>,
    pub balance: Option<f64>,
    pub color: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateAccountInput {
    pub name: Option<String>,
    pub account_type: Option<AccountType>,
    pub currency: Option<String>,
    pub balance: Option<f64>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub is_archived: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CategoryType {
    Expense,
    Income,
}

impl CategoryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CategoryType::Expense => "expense",
            CategoryType::Income => "income",
        }
    }

    pub fn from_str_val(s: &str) -> Result<Self, String> {
        match s {
            "expense" => Ok(CategoryType::Expense),
            "income" => Ok(CategoryType::Income),
            other => Err(format!("Unknown category type: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Category {
    pub id: String,
    pub name: String,
    pub category_type: CategoryType,
    pub icon: String,
    pub color: String,
    pub parent_id: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCategoryInput {
    pub id: Option<String>,
    pub name: String,
    pub category_type: CategoryType,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub parent_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateCategoryInput {
    pub name: Option<String>,
    pub category_type: Option<CategoryType>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub parent_id: Option<Option<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionType {
    Expense,
    Income,
    Transfer,
}

impl TransactionType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransactionType::Expense => "expense",
            TransactionType::Income => "income",
            TransactionType::Transfer => "transfer",
        }
    }

    pub fn from_str_val(s: &str) -> Result<Self, String> {
        match s {
            "expense" => Ok(TransactionType::Expense),
            "income" => Ok(TransactionType::Income),
            "transfer" => Ok(TransactionType::Transfer),
            other => Err(format!("Unknown transaction type: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    Cleared,
    Pending,
}

impl TransactionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TransactionStatus::Cleared => "cleared",
            TransactionStatus::Pending => "pending",
        }
    }

    pub fn from_str_val(s: &str) -> Result<Self, String> {
        match s {
            "cleared" => Ok(TransactionStatus::Cleared),
            "pending" => Ok(TransactionStatus::Pending),
            other => Err(format!("Unknown transaction status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentType {
    Cash,
    DebitCard,
    CreditCard,
    Transfer,
    Voucher,
    MobilePayment,
}

impl PaymentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            PaymentType::Cash => "cash",
            PaymentType::DebitCard => "debit_card",
            PaymentType::CreditCard => "credit_card",
            PaymentType::Transfer => "transfer",
            PaymentType::Voucher => "voucher",
            PaymentType::MobilePayment => "mobile_payment",
        }
    }

    pub fn from_str_val(s: &str) -> Result<Self, String> {
        match s {
            "cash" => Ok(PaymentType::Cash),
            "debit_card" => Ok(PaymentType::DebitCard),
            "credit_card" => Ok(PaymentType::CreditCard),
            "transfer" => Ok(PaymentType::Transfer),
            "voucher" => Ok(PaymentType::Voucher),
            "mobile_payment" => Ok(PaymentType::MobilePayment),
            other => Err(format!("Unknown payment type: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Transaction {
    pub id: String,
    pub account_id: String,
    pub to_account_id: Option<String>,
    pub category_id: Option<String>,
    pub tx_type: TransactionType,
    pub amount: f64,
    pub currency: String,
    pub date: String, // YYYY-MM-DD
    pub time: Option<String>, // HH:mm
    pub payee: Option<String>,
    pub note: Option<String>,
    pub status: TransactionStatus,
    pub payment_type: PaymentType,
    pub created_at: String,
}

impl Transaction {
    /// Returns the parsed NaiveDateTime if date and time are valid.
    pub fn naive_date_time(&self) -> Option<chrono::NaiveDateTime> {
        let date = chrono::NaiveDate::parse_from_str(&self.date, "%Y-%m-%d").ok()?;
        let time_str = self.time.as_deref().unwrap_or("00:00");
        let time = chrono::NaiveTime::parse_from_str(time_str, "%H:%M")
            .or_else(|_| chrono::NaiveTime::parse_from_str(time_str, "%H:%M:%S"))
            .unwrap_or_else(|_| chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        Some(chrono::NaiveDateTime::new(date, time))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransactionWithDetails {
    pub transaction: Transaction,
    pub category_name: Option<String>,
    pub category_icon: Option<String>,
    pub category_color: Option<String>,
    pub account_name: Option<String>,
    pub account_color: Option<String>,
    pub account_icon: Option<String>,
    pub account_currency: Option<String>,
    pub to_account_name: Option<String>,
    pub to_account_currency: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTransactionInput {
    pub id: Option<String>,
    pub account_id: String,
    pub to_account_id: Option<String>,
    pub category_id: Option<String>,
    pub tx_type: TransactionType,
    pub amount: f64,
    pub currency: Option<String>,
    pub date: Option<String>, // Defaults to today
    pub time: Option<String>, // Defaults to current HH:mm
    pub payee: Option<String>,
    pub note: Option<String>,
    pub status: Option<TransactionStatus>,
    pub payment_type: Option<PaymentType>,
}

impl CreateTransactionInput {
    /// Parses the date and time fields into a unified NaiveDateTime.
    pub fn naive_date_time(&self) -> Option<chrono::NaiveDateTime> {
        let date_str = self.date.as_deref()?;
        let date = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").ok()?;
        let time_str = self.time.as_deref().unwrap_or("00:00");
        let time = chrono::NaiveTime::parse_from_str(time_str, "%H:%M")
            .or_else(|_| chrono::NaiveTime::parse_from_str(time_str, "%H:%M:%S"))
            .unwrap_or_else(|_| chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        Some(chrono::NaiveDateTime::new(date, time))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateTransactionInput {
    pub account_id: Option<String>,
    pub to_account_id: Option<Option<String>>,
    pub category_id: Option<Option<String>>,
    pub tx_type: Option<TransactionType>,
    pub amount: Option<f64>,
    pub currency: Option<String>,
    pub date: Option<String>,
    pub time: Option<Option<String>>,
    pub payee: Option<Option<String>>,
    pub note: Option<Option<String>>,
    pub status: Option<TransactionStatus>,
    pub payment_type: Option<PaymentType>,
}

#[derive(Debug, Clone, Default)]
pub struct TransactionFilter {
    pub tx_type: Option<TransactionType>,
    pub account_id: Option<String>,
    pub category_id: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub search: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetPeriod {
    Monthly,
    Weekly,
    OneTime,
}

impl BudgetPeriod {
    pub fn as_str(&self) -> &'static str {
        match self {
            BudgetPeriod::Monthly => "monthly",
            BudgetPeriod::Weekly => "weekly",
            BudgetPeriod::OneTime => "one_time",
        }
    }

    pub fn from_str_val(s: &str) -> Result<Self, String> {
        match s {
            "monthly" => Ok(BudgetPeriod::Monthly),
            "weekly" => Ok(BudgetPeriod::Weekly),
            "one_time" => Ok(BudgetPeriod::OneTime),
            other => Err(format!("Unknown budget period: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Budget {
    pub id: String,
    pub name: String,
    pub amount: f64,
    pub period: BudgetPeriod,
    pub category_id: Option<String>,
    #[serde(default)]
    pub category_ids: Vec<String>,
    pub start_date: String,
    pub end_date: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BudgetWithProgress {
    pub budget: Budget,
    pub spent: f64,
    pub remaining: f64,
    pub spent_percentage: f64,
    pub category_name: Option<String>,
    pub category_color: Option<String>,
    pub category_icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBudgetInput {
    pub id: Option<String>,
    pub name: String,
    pub amount: f64,
    pub period: BudgetPeriod,
    pub category_id: Option<String>,
    #[serde(default)]
    pub category_ids: Vec<String>,
    pub start_date: String,
    pub end_date: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateBudgetInput {
    pub name: Option<String>,
    pub amount: Option<f64>,
    pub period: Option<BudgetPeriod>,
    pub category_id: Option<Option<String>>,
    pub category_ids: Option<Vec<String>>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillFrequency {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

impl BillFrequency {
    pub fn as_str(&self) -> &'static str {
        match self {
            BillFrequency::Daily => "daily",
            BillFrequency::Weekly => "weekly",
            BillFrequency::Monthly => "monthly",
            BillFrequency::Yearly => "yearly",
        }
    }

    pub fn from_str_val(s: &str) -> Result<Self, String> {
        match s {
            "daily" => Ok(BillFrequency::Daily),
            "weekly" => Ok(BillFrequency::Weekly),
            "monthly" => Ok(BillFrequency::Monthly),
            "yearly" => Ok(BillFrequency::Yearly),
            other => Err(format!("Unknown bill frequency: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BillSubscription {
    pub id: String,
    pub name: String,
    pub amount: f64,
    pub bill_type: TransactionType,
    pub account_id: String,
    pub category_id: Option<String>,
    pub payee: Option<String>,
    pub frequency: BillFrequency,
    pub next_date: String, // YYYY-MM-DD
    pub auto_record: bool,
    pub is_active: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BillWithDetails {
    pub bill: BillSubscription,
    pub account_name: Option<String>,
    pub category_name: Option<String>,
    pub category_color: Option<String>,
    pub category_icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBillInput {
    pub id: Option<String>,
    pub name: String,
    pub amount: f64,
    pub bill_type: Option<TransactionType>,
    pub account_id: String,
    pub category_id: Option<String>,
    pub payee: Option<String>,
    pub frequency: Option<BillFrequency>,
    pub next_date: String,
    pub auto_record: Option<bool>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateBillInput {
    pub name: Option<String>,
    pub amount: Option<f64>,
    pub bill_type: Option<TransactionType>,
    pub account_id: Option<String>,
    pub category_id: Option<Option<String>>,
    pub payee: Option<Option<String>>,
    pub frequency: Option<BillFrequency>,
    pub next_date: Option<String>,
    pub auto_record: Option<bool>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebtType {
    DebtIOwe,
    LoanOwedToMe,
}

impl DebtType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DebtType::DebtIOwe => "debt_i_owe",
            DebtType::LoanOwedToMe => "loan_owed_to_me",
        }
    }

    pub fn from_str_val(s: &str) -> Result<Self, String> {
        match s {
            "debt_i_owe" => Ok(DebtType::DebtIOwe),
            "loan_owed_to_me" => Ok(DebtType::LoanOwedToMe),
            other => Err(format!("Unknown debt type: {other}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebtStatus {
    Active,
    Settled,
}

impl DebtStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DebtStatus::Active => "active",
            DebtStatus::Settled => "settled",
        }
    }

    pub fn from_str_val(s: &str) -> Result<Self, String> {
        match s {
            "active" => Ok(DebtStatus::Active),
            "settled" => Ok(DebtStatus::Settled),
            other => Err(format!("Unknown debt status: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Debt {
    pub id: String,
    pub debt_type: DebtType,
    pub person_name: String,
    pub amount: f64,
    pub settled_amount: f64,
    pub due_date: Option<String>,
    pub notes: Option<String>,
    pub status: DebtStatus,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateDebtInput {
    pub id: Option<String>,
    pub debt_type: DebtType,
    pub person_name: String,
    pub amount: f64,
    pub settled_amount: Option<f64>,
    pub due_date: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateDebtInput {
    pub debt_type: Option<DebtType>,
    pub person_name: Option<String>,
    pub amount: Option<f64>,
    pub settled_amount: Option<f64>,
    pub due_date: Option<Option<String>>,
    pub notes: Option<Option<String>>,
    pub status: Option<DebtStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullBackupData {
    pub exported_at: String,
    pub accounts: Vec<Account>,
    pub categories: Vec<Category>,
    pub transactions: Vec<Transaction>,
    pub budgets: Vec<Budget>,
    pub bills_subscriptions: Vec<BillSubscription>,
    pub debts: Vec<Debt>,
    pub settings: Vec<(String, String)>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike};

    #[test]
    fn test_transaction_naive_date_time() {
        let tx = Transaction {
            id: "tx-1".into(),
            account_id: "acc-1".into(),
            to_account_id: None,
            category_id: None,
            tx_type: TransactionType::Expense,
            amount: 50.0,
            currency: "USD".into(),
            date: "2026-10-29".into(),
            time: Some("14:30".into()),
            payee: Some("Store".into()),
            note: Some("Coffee".into()),
            status: TransactionStatus::Cleared,
            payment_type: PaymentType::Cash,
            created_at: "2026-10-29T14:30:00Z".into(),
        };

        let dt = tx.naive_date_time().expect("valid datetime");
        assert_eq!(dt.year(), 2026);
        assert_eq!(dt.month(), 10);
        assert_eq!(dt.day(), 29);
        assert_eq!(dt.hour(), 14);
        assert_eq!(dt.minute(), 30);

        // Test with missing time (defaults to 00:00)
        let mut tx_no_time = tx.clone();
        tx_no_time.time = None;
        let dt_no_time = tx_no_time.naive_date_time().expect("valid date with default time");
        assert_eq!(dt_no_time.hour(), 0);
        assert_eq!(dt_no_time.minute(), 0);

        // Test with invalid date
        let mut tx_invalid_date = tx.clone();
        tx_invalid_date.date = "invalid-date".into();
        assert!(tx_invalid_date.naive_date_time().is_none());
    }

    #[test]
    fn test_create_transaction_input_naive_date_time() {
        let input = CreateTransactionInput {
            id: None,
            account_id: "acc-1".into(),
            to_account_id: None,
            category_id: None,
            tx_type: TransactionType::Income,
            amount: 1000.0,
            currency: Some("USD".into()),
            date: Some("2026-12-25".into()),
            time: Some("09:15".into()),
            payee: None,
            note: None,
            status: None,
            payment_type: None,
        };

        let dt = input.naive_date_time().expect("valid datetime");
        assert_eq!(dt.year(), 2026);
        assert_eq!(dt.month(), 12);
        assert_eq!(dt.day(), 25);
        assert_eq!(dt.hour(), 9);
        assert_eq!(dt.minute(), 15);
    }
}
