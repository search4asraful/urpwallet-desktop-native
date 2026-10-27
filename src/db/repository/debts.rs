use rusqlite::{params, Connection, OptionalExtension};
use chrono::Utc;
use uuid::Uuid;
use crate::db::error::{DbError, DbResult};
use crate::db::models::{
    CreateDebtInput, CreateTransactionInput, Debt, DebtStatus, DebtType, TransactionType,
    UpdateDebtInput,
};
use crate::db::repository::transactions::create_transaction;

pub fn map_debt_row(row: &rusqlite::Row) -> rusqlite::Result<Debt> {
    let debt_type_str: String = row.get("type")?;
    let debt_type = DebtType::from_str_val(&debt_type_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))))?;

    let status_str: String = row.get("status")?;
    let status = DebtStatus::from_str_val(&status_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))))?;

    Ok(Debt {
        id: row.get("id")?,
        debt_type,
        person_name: row.get("person_name")?,
        amount: row.get("amount")?,
        settled_amount: row.get("settled_amount")?,
        due_date: row.get("due_date")?,
        notes: row.get("notes")?,
        status,
        created_at: row.get("created_at")?,
    })
}

pub fn create_debt(conn: &Connection, input: CreateDebtInput) -> DbResult<Debt> {
    if input.amount <= 0.0 {
        return Err(DbError::Validation("Debt amount must be greater than zero".into()));
    }

    let id = input.id.unwrap_or_else(|| format!("d_{}", &Uuid::new_v4().to_string()[..8]));
    let now = Utc::now().to_rfc3339();
    let settled = input.settled_amount.unwrap_or(0.0);
    let status = if settled >= input.amount {
        DebtStatus::Settled
    } else {
        DebtStatus::Active
    };

    conn.execute(
        "INSERT INTO debts (id, type, person_name, amount, settled_amount, due_date, notes, status, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            input.debt_type.as_str(),
            input.person_name,
            input.amount,
            settled,
            input.due_date,
            input.notes,
            status.as_str(),
            now
        ],
    )?;

    get_debt(conn, &id)
}

pub fn get_debt(conn: &Connection, id: &str) -> DbResult<Debt> {
    let mut stmt = conn.prepare(
        "SELECT id, type, person_name, amount, settled_amount, due_date, notes, status, created_at
         FROM debts WHERE id = ?1",
    )?;

    let debt = stmt
        .query_row(params![id], map_debt_row)
        .optional()?
        .ok_or_else(|| DbError::NotFound(format!("Debt not found: {id}")))?;

    Ok(debt)
}

pub fn list_debts(conn: &Connection, active_only: bool) -> DbResult<Vec<Debt>> {
    let sql = if active_only {
        "SELECT id, type, person_name, amount, settled_amount, due_date, notes, status, created_at
         FROM debts WHERE status = 'active' ORDER BY created_at DESC"
    } else {
        "SELECT id, type, person_name, amount, settled_amount, due_date, notes, status, created_at
         FROM debts ORDER BY status ASC, created_at DESC"
    };

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], map_debt_row)?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn update_debt(conn: &Connection, id: &str, input: UpdateDebtInput) -> DbResult<Debt> {
    let current = get_debt(conn, id)?;

    let debt_type = input.debt_type.unwrap_or(current.debt_type);
    let person_name = input.person_name.unwrap_or(current.person_name);
    let amount = input.amount.unwrap_or(current.amount);
    let settled_amount = input.settled_amount.unwrap_or(current.settled_amount);
    let due_date = match input.due_date {
        Some(val) => val,
        None => current.due_date,
    };
    let notes = match input.notes {
        Some(val) => val,
        None => current.notes,
    };
    let status = match input.status {
        Some(s) => s,
        None => {
            if settled_amount >= amount {
                DebtStatus::Settled
            } else {
                DebtStatus::Active
            }
        }
    };

    conn.execute(
        "UPDATE debts SET
            type = ?1,
            person_name = ?2,
            amount = ?3,
            settled_amount = ?4,
            due_date = ?5,
            notes = ?6,
            status = ?7
         WHERE id = ?8",
        params![
            debt_type.as_str(),
            person_name,
            amount,
            settled_amount,
            due_date,
            notes,
            status.as_str(),
            id
        ],
    )?;

    get_debt(conn, id)
}

pub fn delete_debt(conn: &Connection, id: &str) -> DbResult<()> {
    let rows = conn.execute("DELETE FROM debts WHERE id = ?1", params![id])?;
    if rows == 0 {
        return Err(DbError::NotFound(format!("Debt not found: {id}")));
    }
    Ok(())
}

pub fn record_debt_payment(
    conn: &mut Connection,
    id: &str,
    pay_amount: f64,
    account_id: Option<&str>,
) -> DbResult<Debt> {
    if pay_amount <= 0.0 {
        return Err(DbError::Validation("Payment amount must be greater than zero".into()));
    }

    let debt = get_debt(conn, id)?;
    let new_settled = debt.settled_amount + pay_amount;
    let new_status = if new_settled >= debt.amount {
        DebtStatus::Settled
    } else {
        DebtStatus::Active
    };

    conn.execute(
        "UPDATE debts SET settled_amount = ?1, status = ?2 WHERE id = ?3",
        params![new_settled, new_status.as_str(), id],
    )?;

    // If an account is specified, record the financial transaction
    if let Some(acc_id) = account_id {
        let is_income = debt.debt_type == DebtType::LoanOwedToMe;
        let tx_type = if is_income {
            TransactionType::Income
        } else {
            TransactionType::Expense
        };

        let note = format!(
            "Debt repayment ({} {})",
            if is_income { "Collected from" } else { "Paid to" },
            debt.person_name
        );

        create_transaction(
            conn,
            CreateTransactionInput {
                id: None,
                account_id: acc_id.to_string(),
                to_account_id: None,
                category_id: None,
                tx_type,
                amount: pay_amount,
                currency: None,
                date: Some(Utc::now().format("%Y-%m-%d").to_string()),
                time: Some(Utc::now().format("%H:%M").to_string()),
                payee: Some(debt.person_name.clone()),
                note: Some(note),
                status: None,
                payment_type: None,
            },
        )?;
    }

    get_debt(conn, id)
}
