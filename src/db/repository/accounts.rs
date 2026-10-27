use rusqlite::{params, Connection, OptionalExtension};
use chrono::Utc;
use uuid::Uuid;
use crate::db::error::{DbError, DbResult};
use crate::db::models::{Account, AccountType, CreateAccountInput, UpdateAccountInput};

pub fn map_account_row(row: &rusqlite::Row) -> rusqlite::Result<Account> {
    let type_str: String = row.get("type")?;
    let account_type = AccountType::from_str_val(&type_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))))?;

    let is_archived_int: i32 = row.get("is_archived")?;

    Ok(Account {
        id: row.get("id")?,
        name: row.get("name")?,
        account_type,
        currency: row.get("currency")?,
        balance: row.get("balance")?,
        color: row.get("color")?,
        icon: row.get("icon")?,
        is_archived: is_archived_int != 0,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

pub fn create_account(conn: &Connection, input: CreateAccountInput) -> DbResult<Account> {
    let id = input.id.unwrap_or_else(|| format!("acc_{}", &Uuid::new_v4().to_string()[..8]));
    let now = Utc::now().to_rfc3339();
    let currency = input.currency.unwrap_or_else(|| "BDT".to_string());
    let balance = input.balance.unwrap_or(0.0);
    let color = input.color.unwrap_or_else(|| "#10B981".to_string());
    let icon = input.icon.unwrap_or_else(|| "Wallet".to_string());

    conn.execute(
        "INSERT INTO accounts (id, name, type, currency, balance, color, icon, is_archived, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?8)",
        params![id, input.name, input.account_type.as_str(), currency, balance, color, icon, now],
    )?;

    get_account(conn, &id)
}

pub fn get_account(conn: &Connection, id: &str) -> DbResult<Account> {
    let mut stmt = conn.prepare(
        "SELECT id, name, type, currency, balance, color, icon, is_archived, created_at, updated_at
         FROM accounts WHERE id = ?1",
    )?;

    let account = stmt
        .query_row(params![id], map_account_row)
        .optional()?
        .ok_or_else(|| DbError::NotFound(format!("Account not found with id: {id}")))?;

    Ok(account)
}

pub fn list_accounts(conn: &Connection, include_archived: bool) -> DbResult<Vec<Account>> {
    let sql = if include_archived {
        "SELECT id, name, type, currency, balance, color, icon, is_archived, created_at, updated_at
         FROM accounts ORDER BY balance DESC, created_at ASC"
    } else {
        "SELECT id, name, type, currency, balance, color, icon, is_archived, created_at, updated_at
         FROM accounts WHERE is_archived = 0 ORDER BY balance DESC, created_at ASC"
    };

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], map_account_row)?;

    let mut accounts = Vec::new();
    for row in rows {
        accounts.push(row?);
    }
    Ok(accounts)
}

pub fn update_account(conn: &Connection, id: &str, input: UpdateAccountInput) -> DbResult<Account> {
    let current = get_account(conn, id)?;
    let now = Utc::now().to_rfc3339();

    let name = input.name.unwrap_or(current.name);
    let account_type = input.account_type.unwrap_or(current.account_type);
    let currency = input.currency.unwrap_or(current.currency);
    let balance = input.balance.unwrap_or(current.balance);
    let color = input.color.unwrap_or(current.color);
    let icon = input.icon.unwrap_or(current.icon);
    let is_archived = input.is_archived.unwrap_or(current.is_archived);

    conn.execute(
        "UPDATE accounts SET
            name = ?1,
            type = ?2,
            currency = ?3,
            balance = ?4,
            color = ?5,
            icon = ?6,
            is_archived = ?7,
            updated_at = ?8
         WHERE id = ?9",
        params![
            name,
            account_type.as_str(),
            currency,
            balance,
            color,
            icon,
            if is_archived { 1 } else { 0 },
            now,
            id
        ],
    )?;

    get_account(conn, id)
}

pub fn delete_account(conn: &Connection, id: &str) -> DbResult<()> {
    let rows = conn.execute("DELETE FROM accounts WHERE id = ?1", params![id])?;
    if rows == 0 {
        return Err(DbError::NotFound(format!("Account not found: {id}")));
    }
    Ok(())
}

pub fn adjust_account_balance(conn: &Connection, id: &str, delta: f64) -> DbResult<f64> {
    let now = Utc::now().to_rfc3339();
    let rows = conn.execute(
        "UPDATE accounts SET balance = balance + ?1, updated_at = ?2 WHERE id = ?3",
        params![delta, now, id],
    )?;

    if rows == 0 {
        return Err(DbError::NotFound(format!("Account not found: {id}")));
    }

    let acc = get_account(conn, id)?;
    Ok(acc.balance)
}
