use rusqlite::{params, Connection, OptionalExtension};
use chrono::Utc;
use uuid::Uuid;
use crate::db::error::{DbError, DbResult};
use crate::db::models::{
    PaymentType, Transaction, TransactionFilter, TransactionStatus, TransactionType,
    TransactionWithDetails, CreateTransactionInput, UpdateTransactionInput,
};

pub fn map_transaction_row(row: &rusqlite::Row) -> rusqlite::Result<Transaction> {
    let tx_type_str: String = row.get("type")?;
    let tx_type = TransactionType::from_str_val(&tx_type_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))))?;

    let status_str: String = row.get("status")?;
    let status = TransactionStatus::from_str_val(&status_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(11, rusqlite::types::Type::Text, Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))))?;

    let payment_type_str: String = row.get("payment_type")?;
    let payment_type = PaymentType::from_str_val(&payment_type_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(12, rusqlite::types::Type::Text, Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))))?;

    Ok(Transaction {
        id: row.get("id")?,
        account_id: row.get("account_id")?,
        to_account_id: row.get("to_account_id")?,
        category_id: row.get("category_id")?,
        tx_type,
        amount: row.get("amount")?,
        currency: row.get("currency")?,
        date: row.get("date")?,
        time: row.get("time")?,
        payee: row.get("payee")?,
        note: row.get("note")?,
        status,
        payment_type,
        created_at: row.get("created_at")?,
    })
}

pub fn map_transaction_with_details_row(row: &rusqlite::Row) -> rusqlite::Result<TransactionWithDetails> {
    let transaction = map_transaction_row(row)?;
    Ok(TransactionWithDetails {
        transaction,
        category_name: row.get("category_name")?,
        category_icon: row.get("category_icon")?,
        category_color: row.get("category_color")?,
        account_name: row.get("account_name")?,
        account_color: row.get("account_color")?,
        account_icon: row.get("account_icon")?,
        account_currency: row.get("account_currency")?,
        to_account_name: row.get("to_account_name")?,
        to_account_currency: row.get("to_account_currency")?,
    })
}

pub fn create_transaction(
    conn: &mut Connection,
    input: CreateTransactionInput,
) -> DbResult<TransactionWithDetails> {
    if input.amount <= 0.0 {
        return Err(DbError::Validation("Transaction amount must be greater than zero".into()));
    }

    if input.tx_type == TransactionType::Transfer {
        if input.to_account_id.is_none() {
            return Err(DbError::Validation("Destination account is required for transfers".into()));
        }
        if input.to_account_id.as_deref() == Some(&input.account_id) {
            return Err(DbError::Validation("Source and destination accounts cannot be identical".into()));
        }
    }

    let id = input.id.unwrap_or_else(|| format!("tx_{}", &Uuid::new_v4().to_string()[..8]));
    let now = Utc::now().to_rfc3339();
    let date = input.date.unwrap_or_else(|| Utc::now().format("%Y-%m-%d").to_string());
    let time = input.time.or_else(|| Some(Utc::now().format("%H:%M").to_string()));
    let status = input.status.unwrap_or(TransactionStatus::Cleared);
    let payment_type = input.payment_type.unwrap_or(PaymentType::Cash);

    // If currency not specified, fetch currency from the source account
    let currency = match input.currency {
        Some(c) => c,
        None => {
            conn.query_row(
                "SELECT currency FROM accounts WHERE id = ?1",
                params![input.account_id],
                |row| row.get(0),
            )
            .map_err(|_| DbError::NotFound(format!("Account not found: {}", input.account_id)))?
        }
    };

    let tx = conn.transaction()?;

    // 1. Insert transaction
    tx.execute(
        "INSERT INTO transactions (
            id, account_id, to_account_id, category_id, type, amount, currency,
            date, time, payee, note, status, payment_type, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
        params![
            id,
            input.account_id,
            input.to_account_id,
            input.category_id,
            input.tx_type.as_str(),
            input.amount,
            currency,
            date,
            time,
            input.payee,
            input.note,
            status.as_str(),
            payment_type.as_str(),
            now
        ],
    )?;

    // 2. Adjust account balances
    match input.tx_type {
        TransactionType::Expense => {
            let rows = tx.execute(
                "UPDATE accounts SET balance = balance - ?1, updated_at = ?2 WHERE id = ?3",
                params![input.amount, now, input.account_id],
            )?;
            if rows == 0 {
                return Err(DbError::NotFound(format!("Account not found: {}", input.account_id)));
            }
        }
        TransactionType::Income => {
            let rows = tx.execute(
                "UPDATE accounts SET balance = balance + ?1, updated_at = ?2 WHERE id = ?3",
                params![input.amount, now, input.account_id],
            )?;
            if rows == 0 {
                return Err(DbError::NotFound(format!("Account not found: {}", input.account_id)));
            }
        }
        TransactionType::Transfer => {
            let rows = tx.execute(
                "UPDATE accounts SET balance = balance - ?1, updated_at = ?2 WHERE id = ?3",
                params![input.amount, now, input.account_id],
            )?;
            if rows == 0 {
                return Err(DbError::NotFound(format!("Source account not found: {}", input.account_id)));
            }

            if let Some(ref to_id) = input.to_account_id {
                let rows_to = tx.execute(
                    "UPDATE accounts SET balance = balance + ?1, updated_at = ?2 WHERE id = ?3",
                    params![input.amount, now, to_id],
                )?;
                if rows_to == 0 {
                    return Err(DbError::NotFound(format!("Destination account not found: {to_id}")));
                }
            }
        }
    }

    tx.commit()?;

    get_transaction(conn, &id)
}

pub fn get_transaction(conn: &Connection, id: &str) -> DbResult<TransactionWithDetails> {
    let sql = "
        SELECT 
            t.id, t.account_id, t.to_account_id, t.category_id, t.type, t.amount, t.currency,
            t.date, t.time, t.payee, t.note, t.status, t.payment_type, t.created_at,
            c.name as category_name, c.icon as category_icon, c.color as category_color,
            a.name as account_name, a.color as account_color, a.icon as account_icon, a.currency as account_currency,
            a2.name as to_account_name, a2.currency as to_account_currency
        FROM transactions t
        LEFT JOIN categories c ON t.category_id = c.id
        LEFT JOIN accounts a ON t.account_id = a.id
        LEFT JOIN accounts a2 ON t.to_account_id = a2.id
        WHERE t.id = ?1
    ";

    let mut stmt = conn.prepare(sql)?;
    let tx = stmt
        .query_row(params![id], map_transaction_with_details_row)
        .optional()?
        .ok_or_else(|| DbError::NotFound(format!("Transaction not found: {id}")))?;

    Ok(tx)
}

pub fn list_transactions(
    conn: &Connection,
    filter: &TransactionFilter,
) -> DbResult<Vec<TransactionWithDetails>> {
    let mut conditions = Vec::new();
    let mut params_vec: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(t) = filter.tx_type {
        conditions.push("t.type = ?");
        params_vec.push(Box::new(t.as_str().to_string()));
    }

    if let Some(ref acc_id) = filter.account_id {
        conditions.push("(t.account_id = ? OR t.to_account_id = ?)");
        params_vec.push(Box::new(acc_id.clone()));
        params_vec.push(Box::new(acc_id.clone()));
    }

    if let Some(ref cat_id) = filter.category_id {
        conditions.push("t.category_id = ?");
        params_vec.push(Box::new(cat_id.clone()));
    }

    if let Some(ref start) = filter.start_date {
        conditions.push("t.date >= ?");
        params_vec.push(Box::new(start.clone()));
    }

    if let Some(ref end) = filter.end_date {
        conditions.push("t.date <= ?");
        params_vec.push(Box::new(end.clone()));
    }

    if let Some(ref search) = filter.search
        && !search.trim().is_empty()
    {
        conditions.push("(t.payee LIKE ? OR t.note LIKE ? OR c.name LIKE ? OR a.name LIKE ? OR a2.name LIKE ?)");
        let pattern = format!("%{}%", search.trim());
        params_vec.push(Box::new(pattern.clone()));
        params_vec.push(Box::new(pattern.clone()));
        params_vec.push(Box::new(pattern.clone()));
        params_vec.push(Box::new(pattern.clone()));
        params_vec.push(Box::new(pattern));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let limit = filter.limit.unwrap_or(100);
    let offset = filter.offset.unwrap_or(0);

    let sql = format!(
        "SELECT 
            t.id, t.account_id, t.to_account_id, t.category_id, t.type, t.amount, t.currency,
            t.date, t.time, t.payee, t.note, t.status, t.payment_type, t.created_at,
            c.name as category_name, c.icon as category_icon, c.color as category_color,
            a.name as account_name, a.color as account_color, a.icon as account_icon, a.currency as account_currency,
            a2.name as to_account_name, a2.currency as to_account_currency
        FROM transactions t
        LEFT JOIN categories c ON t.category_id = c.id
        LEFT JOIN accounts a ON t.account_id = a.id
        LEFT JOIN accounts a2 ON t.to_account_id = a2.id
        {where_clause}
        ORDER BY t.date DESC, t.time DESC, t.created_at DESC
        LIMIT ? OFFSET ?"
    );

    params_vec.push(Box::new(limit));
    params_vec.push(Box::new(offset));

    let mut stmt = conn.prepare(&sql)?;
    let rusqlite_params: Vec<&dyn rusqlite::ToSql> = params_vec.iter().map(|p| p.as_ref()).collect();
    let rows = stmt.query_map(rusqlite_params.as_slice(), map_transaction_with_details_row)?;

    let mut list = Vec::new();
    for row in rows {
        list.push(row?);
    }
    Ok(list)
}

pub fn update_transaction(
    conn: &mut Connection,
    id: &str,
    input: UpdateTransactionInput,
) -> DbResult<TransactionWithDetails> {
    let old_tx = get_transaction(conn, id)?.transaction;
    let now = Utc::now().to_rfc3339();

    let new_type = input.tx_type.unwrap_or(old_tx.tx_type);
    let new_amount = input.amount.unwrap_or(old_tx.amount);
    let new_account_id = input.account_id.unwrap_or(old_tx.account_id.clone());
    let new_to_account_id = match input.to_account_id {
        Some(val) => val,
        None => old_tx.to_account_id.clone(),
    };
    let new_category_id = match input.category_id {
        Some(val) => val,
        None => old_tx.category_id.clone(),
    };
    let new_currency = input.currency.unwrap_or(old_tx.currency);
    let new_date = input.date.unwrap_or(old_tx.date);
    let new_time = match input.time {
        Some(val) => val,
        None => old_tx.time,
    };
    let new_payee = match input.payee {
        Some(val) => val,
        None => old_tx.payee,
    };
    let new_note = match input.note {
        Some(val) => val,
        None => old_tx.note,
    };
    let new_status = input.status.unwrap_or(old_tx.status);
    let new_payment_type = input.payment_type.unwrap_or(old_tx.payment_type);

    if new_amount <= 0.0 {
        return Err(DbError::Validation("Transaction amount must be greater than zero".into()));
    }

    if new_type == TransactionType::Transfer {
        if new_to_account_id.is_none() {
            return Err(DbError::Validation("Destination account is required for transfers".into()));
        }
        if new_to_account_id.as_deref() == Some(&new_account_id) {
            return Err(DbError::Validation("Source and destination accounts cannot be identical".into()));
        }
    }

    let tx = conn.transaction()?;

    // 1. Revert old transaction's effect on balances
    match old_tx.tx_type {
        TransactionType::Expense => {
            tx.execute(
                "UPDATE accounts SET balance = balance + ?1, updated_at = ?2 WHERE id = ?3",
                params![old_tx.amount, now, old_tx.account_id],
            )?;
        }
        TransactionType::Income => {
            tx.execute(
                "UPDATE accounts SET balance = balance - ?1, updated_at = ?2 WHERE id = ?3",
                params![old_tx.amount, now, old_tx.account_id],
            )?;
        }
        TransactionType::Transfer => {
            tx.execute(
                "UPDATE accounts SET balance = balance + ?1, updated_at = ?2 WHERE id = ?3",
                params![old_tx.amount, now, old_tx.account_id],
            )?;
            if let Some(ref to_id) = old_tx.to_account_id {
                tx.execute(
                    "UPDATE accounts SET balance = balance - ?1, updated_at = ?2 WHERE id = ?3",
                    params![old_tx.amount, now, to_id],
                )?;
            }
        }
    }

    // 2. Update transaction
    tx.execute(
        "UPDATE transactions SET
            account_id = ?1,
            to_account_id = ?2,
            category_id = ?3,
            type = ?4,
            amount = ?5,
            currency = ?6,
            date = ?7,
            time = ?8,
            payee = ?9,
            note = ?10,
            status = ?11,
            payment_type = ?12
         WHERE id = ?13",
        params![
            new_account_id,
            new_to_account_id,
            new_category_id,
            new_type.as_str(),
            new_amount,
            new_currency,
            new_date,
            new_time,
            new_payee,
            new_note,
            new_status.as_str(),
            new_payment_type.as_str(),
            id
        ],
    )?;

    // 3. Apply new transaction's effect on balances
    match new_type {
        TransactionType::Expense => {
            tx.execute(
                "UPDATE accounts SET balance = balance - ?1, updated_at = ?2 WHERE id = ?3",
                params![new_amount, now, new_account_id],
            )?;
        }
        TransactionType::Income => {
            tx.execute(
                "UPDATE accounts SET balance = balance + ?1, updated_at = ?2 WHERE id = ?3",
                params![new_amount, now, new_account_id],
            )?;
        }
        TransactionType::Transfer => {
            tx.execute(
                "UPDATE accounts SET balance = balance - ?1, updated_at = ?2 WHERE id = ?3",
                params![new_amount, now, new_account_id],
            )?;
            if let Some(ref to_id) = new_to_account_id {
                tx.execute(
                    "UPDATE accounts SET balance = balance + ?1, updated_at = ?2 WHERE id = ?3",
                    params![new_amount, now, to_id],
                )?;
            }
        }
    }

    tx.commit()?;

    get_transaction(conn, id)
}

pub fn delete_transaction(conn: &mut Connection, id: &str) -> DbResult<()> {
    let old_tx = get_transaction(conn, id)?.transaction;
    let now = Utc::now().to_rfc3339();

    let tx = conn.transaction()?;

    // Revert balances
    match old_tx.tx_type {
        TransactionType::Expense => {
            tx.execute(
                "UPDATE accounts SET balance = balance + ?1, updated_at = ?2 WHERE id = ?3",
                params![old_tx.amount, now, old_tx.account_id],
            )?;
        }
        TransactionType::Income => {
            tx.execute(
                "UPDATE accounts SET balance = balance - ?1, updated_at = ?2 WHERE id = ?3",
                params![old_tx.amount, now, old_tx.account_id],
            )?;
        }
        TransactionType::Transfer => {
            tx.execute(
                "UPDATE accounts SET balance = balance + ?1, updated_at = ?2 WHERE id = ?3",
                params![old_tx.amount, now, old_tx.account_id],
            )?;
            if let Some(ref to_id) = old_tx.to_account_id {
                tx.execute(
                    "UPDATE accounts SET balance = balance - ?1, updated_at = ?2 WHERE id = ?3",
                    params![old_tx.amount, now, to_id],
                )?;
            }
        }
    }

    tx.execute("DELETE FROM transactions WHERE id = ?1", params![id])?;

    tx.commit()?;
    Ok(())
}
