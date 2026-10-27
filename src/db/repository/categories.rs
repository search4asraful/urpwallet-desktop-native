use rusqlite::{params, Connection, OptionalExtension};
use chrono::Utc;
use uuid::Uuid;
use crate::db::error::{DbError, DbResult};
use crate::db::models::{Category, CategoryType, CreateCategoryInput, UpdateCategoryInput};

pub fn map_category_row(row: &rusqlite::Row) -> rusqlite::Result<Category> {
    let type_str: String = row.get("type")?;
    let category_type = CategoryType::from_str_val(&type_str)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))))?;

    Ok(Category {
        id: row.get("id")?,
        name: row.get("name")?,
        category_type,
        icon: row.get("icon")?,
        color: row.get("color")?,
        parent_id: row.get("parent_id")?,
        created_at: row.get("created_at")?,
    })
}

pub fn create_category(conn: &Connection, input: CreateCategoryInput) -> DbResult<Category> {
    let id = input.id.unwrap_or_else(|| format!("cat_{}", &Uuid::new_v4().to_string()[..8]));
    let now = Utc::now().to_rfc3339();
    let icon = input.icon.unwrap_or_else(|| "CircleDollarSign".to_string());
    let color = input.color.unwrap_or_else(|| "#64748B".to_string());

    conn.execute(
        "INSERT INTO categories (id, name, type, icon, color, parent_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![id, input.name, input.category_type.as_str(), icon, color, input.parent_id, now],
    )?;

    get_category(conn, &id)
}

pub fn get_category(conn: &Connection, id: &str) -> DbResult<Category> {
    let mut stmt = conn.prepare(
        "SELECT id, name, type, icon, color, parent_id, created_at
         FROM categories WHERE id = ?1",
    )?;

    let category = stmt
        .query_row(params![id], map_category_row)
        .optional()?
        .ok_or_else(|| DbError::NotFound(format!("Category not found with id: {id}")))?;

    Ok(category)
}

pub fn list_categories(conn: &Connection, cat_type: Option<CategoryType>) -> DbResult<Vec<Category>> {
    let mut categories = Vec::new();

    if let Some(t) = cat_type {
        let mut stmt = conn.prepare(
            "SELECT id, name, type, icon, color, parent_id, created_at
             FROM categories WHERE type = ?1 ORDER BY name ASC",
        )?;
        let rows = stmt.query_map(params![t.as_str()], map_category_row)?;
        for r in rows {
            categories.push(r?);
        }
    } else {
        let mut stmt = conn.prepare(
            "SELECT id, name, type, icon, color, parent_id, created_at
             FROM categories ORDER BY type ASC, name ASC",
        )?;
        let rows = stmt.query_map([], map_category_row)?;
        for r in rows {
            categories.push(r?);
        }
    }

    Ok(categories)
}

pub fn list_categories_with_counts(conn: &Connection) -> DbResult<Vec<(Category, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT c.id, c.name, c.type, c.icon, c.color, c.parent_id, c.created_at,
                (SELECT COUNT(*) FROM transactions t WHERE t.category_id = c.id) as tx_count
         FROM categories c
         ORDER BY c.type ASC, c.name ASC",
    )?;

    let rows = stmt.query_map([], |row| {
        let cat = map_category_row(row)?;
        let count: i64 = row.get("tx_count")?;
        Ok((cat, count))
    })?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

pub fn update_category(conn: &Connection, id: &str, input: UpdateCategoryInput) -> DbResult<Category> {
    let current = get_category(conn, id)?;

    let name = input.name.unwrap_or(current.name);
    let category_type = input.category_type.unwrap_or(current.category_type);
    let icon = input.icon.unwrap_or(current.icon);
    let color = input.color.unwrap_or(current.color);
    let parent_id = match input.parent_id {
        Some(p) => p,
        None => current.parent_id,
    };

    conn.execute(
        "UPDATE categories SET
            name = ?1,
            type = ?2,
            icon = ?3,
            color = ?4,
            parent_id = ?5
         WHERE id = ?6",
        params![name, category_type.as_str(), icon, color, parent_id, id],
    )?;

    get_category(conn, id)
}

pub fn delete_category(conn: &Connection, id: &str) -> DbResult<()> {
    // Check if category is used in transactions
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM transactions WHERE category_id = ?1",
        params![id],
        |row| row.get(0),
    )?;

    if count > 0 {
        return Err(DbError::ForeignKeyViolation(format!(
            "Cannot delete category '{id}' because it is referenced by {count} transactions"
        )));
    }

    let rows = conn.execute("DELETE FROM categories WHERE id = ?1", params![id])?;
    if rows == 0 {
        return Err(DbError::NotFound(format!("Category not found: {id}")));
    }
    Ok(())
}
