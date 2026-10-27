use rusqlite::{params, Connection, OptionalExtension};
use argon2::{
    password_hash::{phc::PasswordHash, PasswordHasher, PasswordVerifier},
    Argon2,
};
use crate::db::error::{DbError, DbResult};

pub fn get_setting(conn: &Connection, key: &str) -> DbResult<Option<String>> {
    let mut stmt = conn.prepare("SELECT value FROM settings WHERE key = ?1")?;
    let val = stmt
        .query_row(params![key], |row| row.get(0))
        .optional()?;
    Ok(val)
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> DbResult<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub fn get_all_settings(conn: &Connection) -> DbResult<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings ORDER BY key ASC")?;
    let rows = stmt.query_map([], |row| {
        let k: String = row.get(0)?;
        let v: String = row.get(1)?;
        Ok((k, v))
    })?;

    let mut list = Vec::new();
    for r in rows {
        list.push(r?);
    }
    Ok(list)
}

// -------------------------------------------------------------
// Single-User Master PIN / Passphrase Security via Argon2
// -------------------------------------------------------------

pub fn is_pin_enabled(conn: &Connection) -> DbResult<bool> {
    let enabled_str = get_setting(conn, "pin_enabled")?.unwrap_or_else(|| "false".to_string());
    Ok(enabled_str.eq_ignore_ascii_case("true") || enabled_str == "1")
}

pub fn set_master_pin(conn: &Connection, pin: &str) -> DbResult<()> {
    if pin.trim().len() < 4 {
        return Err(DbError::Validation("PIN / Passphrase must be at least 4 characters long".into()));
    }

    let argon2 = Argon2::default();
    let hash: PasswordHash = argon2
        .hash_password(pin.as_bytes())
        .map_err(|e| DbError::PasswordHash(e.to_string()))?;

    set_setting(conn, "pin_hash", &hash.to_string())?;
    set_setting(conn, "pin_enabled", "true")?;

    Ok(())
}

pub fn verify_master_pin(conn: &Connection, pin: &str) -> DbResult<bool> {
    if !is_pin_enabled(conn)? {
        // If security is disabled, always allow access
        return Ok(true);
    }

    let stored_hash = get_setting(conn, "pin_hash")?
        .ok_or_else(|| DbError::Security("PIN protection is enabled but no PIN hash is found".into()))?;

    let parsed_hash = PasswordHash::new(&stored_hash)
        .map_err(|e| DbError::PasswordHash(e.to_string()))?;

    Ok(Argon2::default().verify_password(pin.as_bytes(), &parsed_hash).is_ok())
}

pub fn disable_master_pin(conn: &Connection, current_pin: &str) -> DbResult<()> {
    if is_pin_enabled(conn)? {
        let is_valid = verify_master_pin(conn, current_pin)?;
        if !is_valid {
            return Err(DbError::Security("Invalid master PIN provided".into()));
        }
    }

    set_setting(conn, "pin_enabled", "false")?;
    set_setting(conn, "pin_hash", "")?;

    Ok(())
}
