use rusqlite::{params, Connection, OptionalExtension};
use chrono::Utc;
use uuid::Uuid;
use std::collections::HashMap;
use crate::db::error::{DbError, DbResult};
use crate::db::models::{
    Budget, BudgetPeriod, BudgetWithProgress, CreateBudgetInput, UpdateBudgetInput,
};

pub fn map_budget_row(row: &rusqlite::Row) -> rusqlite::Result<Budget> {
    let period_str: String = row.get("period")?;
    let period = BudgetPeriod::from_str_val(&period_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))))?;

    let category_id: Option<String> = row.get("category_id")?;
    let category_ids_raw: Option<String> = row.get("category_ids").ok();
    let category_ids: Vec<String> = if let Some(raw) = category_ids_raw {
        if let Ok(vec) = serde_json::from_str::<Vec<String>>(&raw) {
            vec
        } else if !raw.trim().is_empty() {
            raw.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
        } else if let Some(ref cid) = category_id {
            vec![cid.clone()]
        } else {
            Vec::new()
        }
    } else if let Some(ref cid) = category_id {
        vec![cid.clone()]
    } else {
        Vec::new()
    };

    Ok(Budget {
        id: row.get("id")?,
        name: row.get("name")?,
        amount: row.get("amount")?,
        period,
        category_id,
        category_ids,
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
    let category_ids_json = serde_json::to_string(&input.category_ids).unwrap_or_else(|_| "[]".to_string());
    let category_id = input.category_id.or_else(|| input.category_ids.first().cloned());

    conn.execute(
        "INSERT INTO budgets (id, name, amount, period, category_id, category_ids, start_date, end_date, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            input.name,
            input.amount,
            input.period.as_str(),
            category_id,
            category_ids_json,
            input.start_date,
            input.end_date,
            now
        ],
    )?;

    get_budget(conn, &id)
}

pub fn get_budget(conn: &Connection, id: &str) -> DbResult<Budget> {
    let mut stmt = conn.prepare(
        "SELECT id, name, amount, period, category_id, category_ids, start_date, end_date, created_at
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
        "SELECT id, name, amount, period, category_id, category_ids, start_date, end_date, created_at
         FROM budgets ORDER BY amount DESC, created_at ASC",
    )?;

    let rows = stmt.query_map([], map_budget_row)?;
    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn list_budgets_with_progress(
    conn: &Connection,
    base_currency: Option<&str>,
) -> DbResult<Vec<BudgetWithProgress>> {
    let budgets = list_budgets(conn)?;

    let target_curr = if let Some(bc) = base_currency {
        bc.to_string()
    } else {
        conn.query_row(
            "SELECT value FROM settings WHERE key = 'base_currency'",
            [],
            |r| r.get::<_, String>(0),
        )
        .unwrap_or_else(|_| "BDT".to_string())
    };

    // Load category mapping (id -> (name, color, icon))
    let mut cat_stmt = conn.prepare("SELECT id, name, color, icon FROM categories")?;
    let cat_rows = cat_stmt.query_map([], |row| {
        let id: String = row.get(0)?;
        let name: String = row.get(1)?;
        let color: String = row.get(2)?;
        let icon: String = row.get(3)?;
        Ok((id, (name, color, icon)))
    })?;
    let mut cat_map = HashMap::new();
    for r in cat_rows {
        let (id, val) = r?;
        cat_map.insert(id, val);
    }

    let mut stmt_all = conn.prepare_cached(
        "SELECT t.amount, t.currency, a.currency as account_currency, t.category_id
         FROM transactions t
         LEFT JOIN accounts a ON t.account_id = a.id
         WHERE t.type = 'expense' AND t.date >= ?1 AND t.date <= ?2",
    )?;

    let mut result = Vec::new();

    for budget in budgets {
        // Resolve category names, color, and icon
        let (category_name, category_color, category_icon) = if !budget.category_ids.is_empty() {
            let names: Vec<String> = budget.category_ids.iter()
                .filter_map(|cid| cat_map.get(cid).map(|(n, _, _)| n.clone()))
                .collect();
            let first_info = budget.category_ids.first().and_then(|cid| cat_map.get(cid));
            let name_str = if names.is_empty() {
                "Selected Categories".to_string()
            } else {
                names.join(", ")
            };
            let color = first_info.map(|(_, c, _)| c.clone()).unwrap_or_else(|| "#10B981".to_string());
            let icon = first_info.map(|(_, _, i)| i.clone()).unwrap_or_else(|| "Target".to_string());
            (Some(name_str), Some(color), Some(icon))
        } else if let Some(ref cat_id) = budget.category_id {
            if let Some((n, c, i)) = cat_map.get(cat_id) {
                (Some(n.clone()), Some(c.clone()), Some(i.clone()))
            } else {
                (Some("General".to_string()), Some("#10B981".to_string()), Some("Target".to_string()))
            }
        } else {
            (Some("All Categories".to_string()), Some("#10B981".to_string()), Some("Target".to_string()))
        };

        // Calculate spent amount in date range converted to target_curr
        let mut sum = 0.0;
        let mut q = stmt_all.query(params![budget.start_date, budget.end_date])?;
        while let Some(r) = q.next()? {
            let amt: f64 = r.get(0)?;
            let tx_c: String = r.get(1)?;
            let acc_c: Option<String> = r.get(2)?;
            let tx_cat: Option<String> = r.get(3)?;

            let matches = if !budget.category_ids.is_empty() {
                tx_cat.as_ref().map_or(false, |c| budget.category_ids.contains(c))
            } else if let Some(ref cid) = budget.category_id {
                tx_cat.as_ref().map_or(false, |c| c == cid)
            } else {
                // If no categories were chosen, tracks all expenses
                true
            };

            if matches {
                let curr = acc_c.as_deref().unwrap_or(&tx_c);
                sum += crate::logic::currency::convert_currency(amt, curr, &target_curr);
            }
        }

        let spent = (sum * 100.0).round() / 100.0;
        let remaining = ((budget.amount - spent) * 100.0).round() / 100.0;
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
    let category_ids = match input.category_ids {
        Some(val) => val,
        None => current.category_ids,
    };
    let category_ids_json = serde_json::to_string(&category_ids).unwrap_or_else(|_| "[]".to_string());
    let category_id = match input.category_id {
        Some(val) => val,
        None => category_ids.first().cloned(),
    };
    let start_date = input.start_date.unwrap_or(current.start_date);
    let end_date = input.end_date.unwrap_or(current.end_date);

    conn.execute(
        "UPDATE budgets SET
            name = ?1,
            amount = ?2,
            period = ?3,
            category_id = ?4,
            category_ids = ?5,
            start_date = ?6,
            end_date = ?7
         WHERE id = ?8",
        params![name, amount, period.as_str(), category_id, category_ids_json, start_date, end_date, id],
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
