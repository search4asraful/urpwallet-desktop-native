use rusqlite::{params, Connection, OptionalExtension};
use chrono::Utc;
use uuid::Uuid;
use crate::db::error::{DbError, DbResult};
use crate::db::models::{
    Budget, BudgetPeriod, BudgetWithProgress, CreateBudgetInput, UpdateBudgetInput,
};

pub fn map_budget_row(row: &rusqlite::Row) -> rusqlite::Result<Budget> {
    let period_str: String = row.get("period")?;
    let period = BudgetPeriod::from_str_val(&period_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))))?;

    Ok(Budget {
        id: row.get("id")?,
        name: row.get("name")?,
        amount: row.get("amount")?,
        period,
        category_id: row.get("category_id")?,
        start_date: row.get("start_date")?,
        end_date: row.get("end_date")?,
        created_at: row.get("created_at")?,
    })
}

pub fn create_budget(conn: &Connection, input: CreateBudgetInput) -> DbResult<Budget> {
    if input.amount <= 0.0 {
        return Err(DbError::Validation("Budget amount must be greater than zero".into()));
    }

    let id = input.id.unwrap_or_else(|| format!("b_{}", &Uuid::new_v4().to_string()[..8]));
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO budgets (id, name, amount, period, category_id, start_date, end_date, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            id,
            input.name,
            input.amount,
            input.period.as_str(),
            input.category_id,
            input.start_date,
            input.end_date,
            now
        ],
    )?;

    get_budget(conn, &id)
}

pub fn get_budget(conn: &Connection, id: &str) -> DbResult<Budget> {
    let mut stmt = conn.prepare(
        "SELECT id, name, amount, period, category_id, start_date, end_date, created_at
         FROM budgets WHERE id = ?1",
    )?;

    let budget = stmt
        .query_row(params![id], map_budget_row)
        .optional()?
        .ok_or_else(|| DbError::NotFound(format!("Budget not found: {id}")))?;

    Ok(budget)
}

pub fn list_budgets(conn: &Connection) -> DbResult<Vec<Budget>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, amount, period, category_id, start_date, end_date, created_at
         FROM budgets ORDER BY amount DESC, created_at ASC",
    )?;

    let rows = stmt.query_map([], map_budget_row)?;
    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn list_budgets_with_progress(conn: &Connection) -> DbResult<Vec<BudgetWithProgress>> {
    let sql = "
        SELECT 
            b.id, b.name, b.amount, b.period, b.category_id, b.start_date, b.end_date, b.created_at,
            c.name as category_name, c.color as category_color, c.icon as category_icon
        FROM budgets b
        LEFT JOIN categories c ON b.category_id = c.id
        ORDER BY b.amount DESC, b.created_at ASC
    ";

    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |row| {
        let budget = map_budget_row(row)?;
        let category_name: Option<String> = row.get("category_name")?;
        let category_color: Option<String> = row.get("category_color")?;
        let category_icon: Option<String> = row.get("category_icon")?;
        Ok((budget, category_name, category_color, category_icon))
    })?;

    let mut result = Vec::new();
    let mut stmt_cat = conn.prepare_cached(
        "SELECT COALESCE(SUM(amount), 0.0) FROM transactions
         WHERE type = 'expense' AND category_id = ?1 AND date >= ?2 AND date <= ?3",
    )?;
    let mut stmt_all = conn.prepare_cached(
        "SELECT COALESCE(SUM(amount), 0.0) FROM transactions
         WHERE type = 'expense' AND date >= ?1 AND date <= ?2",
    )?;

    for item in rows {
        let (budget, category_name, category_color, category_icon) = item?;

        // Calculate spent amount in date range
        let spent: f64 = if let Some(ref cat_id) = budget.category_id {
            stmt_cat.query_row(
                params![cat_id, budget.start_date, budget.end_date],
                |r| r.get(0),
            )?
        } else {
            stmt_all.query_row(
                params![budget.start_date, budget.end_date],
                |r| r.get(0),
            )?
        };

        let remaining = budget.amount - spent;
        let spent_percentage = if budget.amount > 0.0 {
            (spent / budget.amount) * 100.0
        } else {
            0.0
        };

        result.push(BudgetWithProgress {
            budget,
            spent,
            remaining,
            spent_percentage,
            category_name,
            category_color,
            category_icon,
        });
    }

    Ok(result)
}

pub fn update_budget(conn: &Connection, id: &str, input: UpdateBudgetInput) -> DbResult<Budget> {
    let current = get_budget(conn, id)?;

    let name = input.name.unwrap_or(current.name);
    let amount = input.amount.unwrap_or(current.amount);
    let period = input.period.unwrap_or(current.period);
    let category_id = match input.category_id {
        Some(val) => val,
        None => current.category_id,
    };
    let start_date = input.start_date.unwrap_or(current.start_date);
    let end_date = input.end_date.unwrap_or(current.end_date);

    conn.execute(
        "UPDATE budgets SET
            name = ?1,
            amount = ?2,
            period = ?3,
            category_id = ?4,
            start_date = ?5,
            end_date = ?6
         WHERE id = ?7",
        params![name, amount, period.as_str(), category_id, start_date, end_date, id],
    )?;

    get_budget(conn, id)
}

pub fn delete_budget(conn: &Connection, id: &str) -> DbResult<()> {
    let rows = conn.execute("DELETE FROM budgets WHERE id = ?1", params![id])?;
    if rows == 0 {
        return Err(DbError::NotFound(format!("Budget not found: {id}")));
    }
    Ok(())
}
