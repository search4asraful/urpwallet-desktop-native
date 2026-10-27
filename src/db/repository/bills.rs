use rusqlite::{params, Connection, OptionalExtension};
use chrono::{Datelike, Days, Months, NaiveDate, Utc};
use uuid::Uuid;
use crate::db::error::{DbError, DbResult};
use crate::db::models::{
    BillFrequency, BillSubscription, BillWithDetails, CreateBillInput, CreateTransactionInput,
    TransactionType, TransactionWithDetails, UpdateBillInput,
};
use crate::db::repository::transactions::create_transaction;

pub fn map_bill_row(row: &rusqlite::Row) -> rusqlite::Result<BillSubscription> {
    let bill_type_str: String = row.get("type")?;
    let bill_type = TransactionType::from_str_val(&bill_type_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))))?;

    let freq_str: String = row.get("frequency")?;
    let frequency = BillFrequency::from_str_val(&freq_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))))?;

    let auto_record_int: i32 = row.get("auto_record")?;
    let is_active_int: i32 = row.get("is_active")?;

    Ok(BillSubscription {
        id: row.get("id")?,
        name: row.get("name")?,
        amount: row.get("amount")?,
        bill_type,
        account_id: row.get("account_id")?,
        category_id: row.get("category_id")?,
        payee: row.get("payee")?,
        frequency,
        next_date: row.get("next_date")?,
        auto_record: auto_record_int != 0,
        is_active: is_active_int != 0,
        created_at: row.get("created_at")?,
    })
}

pub fn create_bill(conn: &Connection, input: CreateBillInput) -> DbResult<BillSubscription> {
    if input.amount <= 0.0 {
        return Err(DbError::Validation("Bill amount must be greater than zero".into()));
    }

    let id = input.id.unwrap_or_else(|| format!("plan_{}", &Uuid::new_v4().to_string()[..8]));
    let now = Utc::now().to_rfc3339();
    let bill_type = input.bill_type.unwrap_or(TransactionType::Expense);
    let frequency = input.frequency.unwrap_or(BillFrequency::Monthly);
    let auto_record = input.auto_record.unwrap_or(false);
    let is_active = input.is_active.unwrap_or(true);

    conn.execute(
        "INSERT INTO bills_subscriptions (
            id, name, amount, type, account_id, category_id, payee, frequency, next_date, auto_record, is_active, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            id,
            input.name,
            input.amount,
            bill_type.as_str(),
            input.account_id,
            input.category_id,
            input.payee,
            frequency.as_str(),
            input.next_date,
            if auto_record { 1 } else { 0 },
            if is_active { 1 } else { 0 },
            now
        ],
    )?;

    get_bill(conn, &id)
}

pub fn get_bill(conn: &Connection, id: &str) -> DbResult<BillSubscription> {
    let mut stmt = conn.prepare(
        "SELECT id, name, amount, type, account_id, category_id, payee, frequency, next_date, auto_record, is_active, created_at
         FROM bills_subscriptions WHERE id = ?1",
    )?;

    let bill = stmt
        .query_row(params![id], map_bill_row)
        .optional()?
        .ok_or_else(|| DbError::NotFound(format!("Bill/Subscription not found: {id}")))?;

    Ok(bill)
}

pub fn list_bills(conn: &Connection, active_only: bool) -> DbResult<Vec<BillWithDetails>> {
    let sql = if active_only {
        "SELECT 
            b.id, b.name, b.amount, b.type, b.account_id, b.category_id, b.payee, b.frequency, b.next_date, b.auto_record, b.is_active, b.created_at,
            a.name as account_name,
            c.name as category_name, c.color as category_color, c.icon as category_icon
         FROM bills_subscriptions b
         LEFT JOIN accounts a ON b.account_id = a.id
         LEFT JOIN categories c ON b.category_id = c.id
         WHERE b.is_active = 1
         ORDER BY b.next_date ASC"
    } else {
        "SELECT 
            b.id, b.name, b.amount, b.type, b.account_id, b.category_id, b.payee, b.frequency, b.next_date, b.auto_record, b.is_active, b.created_at,
            a.name as account_name,
            c.name as category_name, c.color as category_color, c.icon as category_icon
         FROM bills_subscriptions b
         LEFT JOIN accounts a ON b.account_id = a.id
         LEFT JOIN categories c ON b.category_id = c.id
         ORDER BY b.is_active DESC, b.next_date ASC"
    };

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| {
        let bill = map_bill_row(row)?;
        let account_name: Option<String> = row.get("account_name")?;
        let category_name: Option<String> = row.get("category_name")?;
        let category_color: Option<String> = row.get("category_color")?;
        let category_icon: Option<String> = row.get("category_icon")?;

        Ok(BillWithDetails {
            bill,
            account_name,
            category_name,
            category_color,
            category_icon,
        })
    })?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn update_bill(conn: &Connection, id: &str, input: UpdateBillInput) -> DbResult<BillSubscription> {
    let current = get_bill(conn, id)?;

    let name = input.name.unwrap_or(current.name);
    let amount = input.amount.unwrap_or(current.amount);
    let bill_type = input.bill_type.unwrap_or(current.bill_type);
    let account_id = input.account_id.unwrap_or(current.account_id);
    let category_id = match input.category_id {
        Some(val) => val,
        None => current.category_id,
    };
    let payee = match input.payee {
        Some(val) => val,
        None => current.payee,
    };
    let frequency = input.frequency.unwrap_or(current.frequency);
    let next_date = input.next_date.unwrap_or(current.next_date);
    let auto_record = input.auto_record.unwrap_or(current.auto_record);
    let is_active = input.is_active.unwrap_or(current.is_active);

    conn.execute(
        "UPDATE bills_subscriptions SET
            name = ?1,
            amount = ?2,
            type = ?3,
            account_id = ?4,
            category_id = ?5,
            payee = ?6,
            frequency = ?7,
            next_date = ?8,
            auto_record = ?9,
            is_active = ?10
         WHERE id = ?11",
        params![
            name,
            amount,
            bill_type.as_str(),
            account_id,
            category_id,
            payee,
            frequency.as_str(),
            next_date,
            if auto_record { 1 } else { 0 },
            if is_active { 1 } else { 0 },
            id
        ],
    )?;

    get_bill(conn, id)
}

pub fn delete_bill(conn: &Connection, id: &str) -> DbResult<()> {
    let rows = conn.execute("DELETE FROM bills_subscriptions WHERE id = ?1", params![id])?;
    if rows == 0 {
        return Err(DbError::NotFound(format!("Bill not found: {id}")));
    }
    Ok(())
}

pub fn calculate_next_date(current_date_str: &str, freq: BillFrequency) -> String {
    let parsed = NaiveDate::parse_from_str(current_date_str, "%Y-%m-%d")
        .unwrap_or_else(|_| Utc::now().date_naive());

    let next = match freq {
        BillFrequency::Daily => parsed.checked_add_days(Days::new(1)).unwrap_or(parsed),
        BillFrequency::Weekly => parsed.checked_add_days(Days::new(7)).unwrap_or(parsed),
        BillFrequency::Monthly => parsed.checked_add_months(Months::new(1)).unwrap_or_else(|| {
            // Fallback for end-of-month rollover (e.g. Jan 31 -> Feb 28)
            let (year, month) = if parsed.month() == 12 {
                (parsed.year() + 1, 1)
            } else {
                (parsed.year(), parsed.month() + 1)
            };
            NaiveDate::from_ymd_opt(year, month, 28).unwrap_or(parsed)
        }),
        BillFrequency::Yearly => {
            NaiveDate::from_ymd_opt(parsed.year() + 1, parsed.month(), parsed.day())
                .unwrap_or_else(|| NaiveDate::from_ymd_opt(parsed.year() + 1, parsed.month(), 28).unwrap_or(parsed))
        }
    };

    next.format("%Y-%m-%d").to_string()
}

pub fn execute_bill_payment(
    conn: &mut Connection,
    id: &str,
) -> DbResult<TransactionWithDetails> {
    let bill = get_bill(conn, id)?;
    let today = Utc::now().format("%Y-%m-%d").to_string();
    let current_time = Utc::now().format("%H:%M").to_string();

    let payee_name = bill.payee.clone().unwrap_or_else(|| bill.name.clone());
    let note = format!("Recurring: {}", bill.name);

    // 1. Log transaction and adjust balance
    let tx = create_transaction(
        conn,
        CreateTransactionInput {
            id: None,
            account_id: bill.account_id.clone(),
            to_account_id: None,
            category_id: bill.category_id.clone(),
            tx_type: bill.bill_type,
            amount: bill.amount,
            currency: None,
            date: Some(today),
            time: Some(current_time),
            payee: Some(payee_name),
            note: Some(note),
            status: None,
            payment_type: None,
        },
    )?;

    // 2. Advance next_date in bills_subscriptions
    let next_date_str = calculate_next_date(&bill.next_date, bill.frequency);
    conn.execute(
        "UPDATE bills_subscriptions SET next_date = ?1 WHERE id = ?2",
        params![next_date_str, id],
    )?;

    Ok(tx)
}
