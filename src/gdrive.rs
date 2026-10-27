use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::db::models::FullBackupData;
use crate::db::Database;

pub const GDRIVE_FOLDER_NAME: &str = "urpwallet_backups";

#[derive(Debug, Clone, Default)]
pub struct GDriveSettings {
    pub client_id: String,
    pub client_secret: String,
    pub access_token: String,
    pub refresh_token: String,
    pub token_expires_at: i64,
    pub user_email: String,
    pub folder_id: String,
    pub scope: String,
    pub scope_type: String,
    pub auto_backup: bool,
    pub frequency: String,
    pub retention_count: i32,
    pub last_backup_time: String,
    pub last_backup_status: String,
    pub last_backup_error: String,
    pub last_backup_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupFileInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(rename = "createdTime", default)]
    pub created_time: Option<String>,
    #[serde(rename = "modifiedTime", default)]
    pub modified_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DriveFileList {
    #[serde(default)]
    files: Vec<BackupFileInfo>,
}

#[derive(Debug, Deserialize)]
struct DriveFileCreated {
    id: String,
    name: String,
    #[serde(default)]
    size: Option<String>,
    #[serde(rename = "createdTime", default)]
    created_time: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<i64>,
    #[serde(default)]
    scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UserInfoResponse {
    #[serde(default)]
    email: Option<String>,
}

/// Helper to percent-encode query parameters according to application/x-www-form-urlencoded
pub fn url_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &b in input.as_bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'*' => {
                out.push(b as char);
            }
            b' ' => out.push('+'),
            _ => {
                use std::fmt::Write;
                let _ = write!(out, "%{:02X}", b);
            }
        }
    }
    out
}

/// Load Google Drive settings from SQLite database
pub fn load_settings(db: &Database) -> GDriveSettings {
    let get = |k: &str| db.get_setting(k).ok().flatten().unwrap_or_default();

    let client_id = match get("gdrive_client_id") {
        s if !s.trim().is_empty() => s,
        _ => std::env::var("GOOGLE_CLIENT_ID").unwrap_or_default(),
    };
    let client_secret = match get("gdrive_client_secret") {
        s if !s.trim().is_empty() => s,
        _ => std::env::var("GOOGLE_CLIENT_SECRET").unwrap_or_default(),
    };

    let token_expires_at = get("gdrive_token_expires_at").parse().unwrap_or(0);
    let auto_backup = get("gdrive_auto_backup") == "true";
    let frequency = match get("gdrive_backup_frequency") {
        s if !s.trim().is_empty() => s,
        _ => "daily".to_string(),
    };
    let retention_count = get("gdrive_retention_count").parse().unwrap_or(10);

    GDriveSettings {
        client_id,
        client_secret,
        access_token: get("gdrive_access_token"),
        refresh_token: get("gdrive_refresh_token"),
        token_expires_at,
        user_email: get("gdrive_user_email"),
        folder_id: get("gdrive_folder_id"),
        scope: get("gdrive_scope"),
        scope_type: get("gdrive_scope_type"),
        auto_backup,
        frequency,
        retention_count,
        last_backup_time: get("gdrive_last_backup_time"),
        last_backup_status: get("gdrive_last_backup_status"),
        last_backup_error: get("gdrive_last_backup_error"),
        last_backup_hash: get("gdrive_last_backup_hash"),
    }
}

/// Save Google Drive settings into SQLite database
pub fn save_config(
    db: &Database,
    client_id: &str,
    client_secret: &str,
    auto_backup: bool,
    frequency: &str,
    retention_count: i32,
) -> Result<(), String> {
    db.set_setting("gdrive_client_id", client_id.trim())
        .map_err(|e| e.to_string())?;
    db.set_setting("gdrive_client_secret", client_secret.trim())
        .map_err(|e| e.to_string())?;
    db.set_setting(
        "gdrive_auto_backup",
        if auto_backup { "true" } else { "false" },
    )
    .map_err(|e| e.to_string())?;
    db.set_setting("gdrive_backup_frequency", frequency.trim())
        .map_err(|e| e.to_string())?;
    db.set_setting(
        "gdrive_retention_count",
        &retention_count.max(1).to_string(),
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Generates Google OAuth 2.0 authorization URL
pub fn build_auth_url(client_id: &str, redirect_uri: &str) -> String {
    let scopes = "https://www.googleapis.com/auth/drive.file https://www.googleapis.com/auth/userinfo.email";
    format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent&include_granted_scopes=true",
        url_encode(client_id.trim()),
        url_encode(redirect_uri.trim()),
        url_encode(scopes),
    )
}

/// Starts a temporary loopback HTTP listener for OAuth redirect callback
pub fn start_oauth_listener(preferred_port: u16) -> Result<(u16, std::sync::mpsc::Receiver<Result<String, String>>), String> {
    // Try preferred port first (e.g. 9008 or 8085), fallback to random port
    let listener = TcpListener::bind(format!("127.0.0.1:{}", preferred_port))
        .or_else(|_| TcpListener::bind("127.0.0.1:0"))
        .map_err(|e| format!("Failed to bind local loopback port for OAuth: {e}"))?;

    let bound_port = listener
        .local_addr()
        .map_err(|e| e.to_string())?
        .port();

    let (tx, rx) = std::sync::mpsc::channel();

    // Listener thread accepts 1 HTTP request and extracts `code`
    std::thread::spawn(move || {
        let _ = listener.set_nonblocking(false);
        match listener.accept() {
            Ok((mut stream, _)) => {
                let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
                let mut buffer = [0u8; 4096];
                let bytes = stream.read(&mut buffer).unwrap_or(0);
                let req_text = String::from_utf8_lossy(&buffer[..bytes]);

                // Extract code query parameter from request line
                let mut code_found = None;
                let mut error_found = None;

                if let Some(first_line) = req_text.lines().next()
                    && let Some(query_start) = first_line.find('?')
                {
                    let query_str = &first_line[query_start + 1..];
                    let query_end = query_str.find(' ').unwrap_or(query_str.len());
                    let clean_query = &query_str[..query_end];

                    for pair in clean_query.split('&') {
                        let mut parts = pair.splitn(2, '=');
                        let key = parts.next().unwrap_or("");
                        let val = parts.next().unwrap_or("");
                        if key == "code" {
                            code_found = Some(val.to_string());
                        } else if key == "error" {
                            error_found = Some(val.to_string());
                        }
                    }
                }

                if let Some(code) = code_found {
                    let html = "<!DOCTYPE html><html><body style='font-family:system-ui,sans-serif;background:#090A0F;color:#FFFFFF;text-align:center;padding:50px;'><h2 style='color:#10B981;'>✓ Google Drive Connected!</h2><p>urpwallet has received your authorization. You can close this tab and return to the application.</p></body></html>";
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        html.len(),
                        html
                    );
                    let _ = stream.write_all(resp.as_bytes());
                    let _ = stream.flush();
                    let _ = tx.send(Ok(code));
                } else {
                    let err_msg = error_found.unwrap_or_else(|| "Authorization code was not found in callback".to_string());
                    let html = format!("<!DOCTYPE html><html><body style='font-family:system-ui,sans-serif;background:#090A0F;color:#FFFFFF;text-align:center;padding:50px;'><h2 style='color:#F43F5E;'>Connection Error</h2><p>{}</p></body></html>", err_msg);
                    let resp = format!(
                        "HTTP/1.1 400 Bad Request\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        html.len(),
                        html
                    );
                    let _ = stream.write_all(resp.as_bytes());
                    let _ = stream.flush();
                    let _ = tx.send(Err(err_msg));
                }
            }
            Err(e) => {
                let _ = tx.send(Err(format!("Local OAuth server error: {e}")));
            }
        }
    });

    Ok((bound_port, rx))
}

/// Exchanges authorization code for access and refresh tokens, and fetches user email
pub fn exchange_code_for_tokens(
    db: &Database,
    code: &str,
    client_id: &str,
    client_secret: &str,
    redirect_uri: &str,
) -> Result<String, String> {
    let body = format!(
        "code={}&client_id={}&client_secret={}&redirect_uri={}&grant_type=authorization_code",
        url_encode(code.trim()),
        url_encode(client_id.trim()),
        url_encode(client_secret.trim()),
        url_encode(redirect_uri.trim()),
    );

    let res = ureq::post("https://oauth2.googleapis.com/token")
        .set("Content-Type", "application/x-www-form-urlencoded")
        .timeout(Duration::from_secs(15))
        .send_string(&body)
        .map_err(|e| format!("OAuth token request error: {e}"))?;

    let token_resp: TokenResponse = res
        .into_json()
        .map_err(|e| format!("Failed to parse token response JSON: {e}"))?;

    let expires_in = token_resp.expires_in.unwrap_or(3600);
    let expires_at = Utc::now().timestamp_millis() + (expires_in - 120) * 1000;

    let _ = db.set_setting("gdrive_access_token", &token_resp.access_token);
    let _ = db.set_setting("gdrive_token_expires_at", &expires_at.to_string());
    if let Some(rt) = token_resp.refresh_token {
        let _ = db.set_setting("gdrive_refresh_token", &rt);
    }
    if let Some(scope) = token_resp.scope {
        let _ = db.set_setting("gdrive_scope", &scope);
    }

    // Fetch user email
    let email = match ureq::get("https://www.googleapis.com/oauth2/v2/userinfo")
        .set("Authorization", &format!("Bearer {}", token_resp.access_token))
        .timeout(Duration::from_secs(10))
        .call()
    {
        Ok(user_res) => {
            let user_info: UserInfoResponse = user_res.into_json().unwrap_or(UserInfoResponse { email: None });
            user_info.email.unwrap_or_default()
        }
        Err(_) => String::new(),
    };

    if !email.is_empty() {
        let _ = db.set_setting("gdrive_user_email", &email);
    }

    let _ = db.set_setting("gdrive_last_backup_error", "");

    Ok(email)
}

/// Ensures a valid access token, refreshing with refresh token if needed
pub fn get_valid_access_token(db: &Database) -> Result<String, String> {
    let settings = load_settings(db);

    if settings.refresh_token.trim().is_empty() {
        return Err("Google Drive is not connected (no refresh token found). Please connect Google Drive first.".to_string());
    }

    let now_ms = Utc::now().timestamp_millis();
    if !settings.access_token.trim().is_empty() && settings.token_expires_at > now_ms + 60_000 {
        return Ok(settings.access_token);
    }

    if settings.client_id.trim().is_empty() || settings.client_secret.trim().is_empty() {
        return Err("Google Client ID or Client Secret is missing in settings.".to_string());
    }

    let body = format!(
        "client_id={}&client_secret={}&refresh_token={}&grant_type=refresh_token",
        url_encode(&settings.client_id),
        url_encode(&settings.client_secret),
        url_encode(&settings.refresh_token),
    );

    let res = ureq::post("https://oauth2.googleapis.com/token")
        .set("Content-Type", "application/x-www-form-urlencoded")
        .timeout(Duration::from_secs(15))
        .send_string(&body)
        .map_err(|e| format!("Failed to refresh Google access token: {e}"))?;

    let token_resp: TokenResponse = res
        .into_json()
        .map_err(|e| format!("Failed to parse refresh token response: {e}"))?;

    let expires_in = token_resp.expires_in.unwrap_or(3600);
    let expires_at = Utc::now().timestamp_millis() + (expires_in - 120) * 1000;

    let _ = db.set_setting("gdrive_access_token", &token_resp.access_token);
    let _ = db.set_setting("gdrive_token_expires_at", &expires_at.to_string());

    Ok(token_resp.access_token)
}

/// Locates or creates the `urpwallet_backups` folder on Google Drive
pub fn get_or_create_folder(access_token: &str, db: &Database) -> Result<String, String> {
    let settings = load_settings(db);

    if !settings.folder_id.trim().is_empty() {
        // Verify folder still exists and is not trashed
        let check_url = format!(
            "https://www.googleapis.com/drive/v3/files/{}?fields=id,trashed",
            settings.folder_id.trim()
        );
        if let Ok(res) = ureq::get(&check_url)
            .set("Authorization", &format!("Bearer {}", access_token))
            .timeout(Duration::from_secs(10))
            .call()
            && let Ok(v) = res.into_json::<serde_json::Value>()
            && v.get("trashed").and_then(|t| t.as_bool()) != Some(true)
        {
            return Ok(settings.folder_id);
        }
    }

    // Search for existing folder by name
    let query = format!("name = '{}' and mimeType = 'application/vnd.google-apps.folder' and trashed = false", GDRIVE_FOLDER_NAME);
    let search_url = format!(
        "https://www.googleapis.com/drive/v3/files?q={}&fields=files(id,name)",
        url_encode(&query)
    );

    if let Ok(res) = ureq::get(&search_url)
        .set("Authorization", &format!("Bearer {}", access_token))
        .timeout(Duration::from_secs(10))
        .call()
        && let Ok(list) = res.into_json::<DriveFileList>()
        && let Some(first) = list.files.first()
    {
        let _ = db.set_setting("gdrive_folder_id", &first.id);
        return Ok(first.id.clone());
    }

    // Create new folder
    let create_body = serde_json::json!({
        "name": GDRIVE_FOLDER_NAME,
        "mimeType": "application/vnd.google-apps.folder",
        "description": "Automated backups for urpwallet"
    });

    let res = ureq::post("https://www.googleapis.com/drive/v3/files")
        .set("Authorization", &format!("Bearer {}", access_token))
        .set("Content-Type", "application/json")
        .timeout(Duration::from_secs(15))
        .send_json(create_body)
        .map_err(|e| format!("Failed to create backup folder on Google Drive: {e}"))?;

    let folder_resp: serde_json::Value = res
        .into_json()
        .map_err(|e| format!("Failed to parse folder creation response: {e}"))?;

    let folder_id = folder_resp
        .get("id")
        .and_then(|id| id.as_str())
        .ok_or_else(|| "Google Drive did not return a folder ID".to_string())?
        .to_string();

    let _ = db.set_setting("gdrive_folder_id", &folder_id);
    Ok(folder_id)
}

/// Uploads full JSON snapshot of database to Google Drive
pub fn upload_backup(db: &Database) -> Result<BackupFileInfo, String> {
    let access_token = get_valid_access_token(db)?;
    let folder_id = get_or_create_folder(&access_token, db)?;

    let full_backup: FullBackupData = db
        .export_full_backup()
        .map_err(|e| format!("Failed to generate ledger backup: {e}"))?;

    let content = serde_json::to_string_pretty(&full_backup)
        .map_err(|e| format!("Failed to serialize backup data: {e}"))?;

    let date_stamp = Utc::now().format("%Y-%m-%dT%H-%M-%S").to_string();
    let filename = format!("urpwallet_backup_{}.json", date_stamp);

    let boundary = "-------314159265358979323846";
    let metadata = serde_json::json!({
        "name": filename,
        "parents": [folder_id],
        "mimeType": "application/json",
        "description": format!("urpwallet backup created at {}", Utc::now().to_rfc3339())
    });

    let multipart_body = format!(
        "\r\n--{boundary}\r\nContent-Type: application/json; charset=UTF-8\r\n\r\n{}\r\n--{boundary}\r\nContent-Type: application/json\r\n\r\n{}\r\n--{boundary}--\r\n",
        metadata,
        content
    );

    let upload_url = "https://www.googleapis.com/upload/drive/v3/files?uploadType=multipart&fields=id,name,size,createdTime";
    let res = ureq::post(upload_url)
        .set("Authorization", &format!("Bearer {}", access_token))
        .set("Content-Type", &format!("multipart/related; boundary={boundary}"))
        .timeout(Duration::from_secs(30))
        .send_bytes(multipart_body.as_bytes())
        .map_err(|e| format!("Failed to upload backup to Google Drive: {e}"))?;

    let uploaded: DriveFileCreated = res
        .into_json()
        .map_err(|e| format!("Failed to parse upload response: {e}"))?;

    let now_iso = Utc::now().to_rfc3339();
    let _ = db.set_setting("gdrive_last_backup_time", &now_iso);
    let _ = db.set_setting("gdrive_last_backup_status", "success");
    let _ = db.set_setting("gdrive_last_backup_error", "");

    if let Ok(hash) = compute_backup_hash(db) {
        let _ = db.set_setting("gdrive_last_backup_hash", &hash);
    }

    // Prune old backups in background
    let settings = load_settings(db);
    let retention = settings.retention_count.max(1);
    let token_clone = access_token.clone();
    let folder_clone = folder_id.clone();
    std::thread::spawn(move || {
        let _ = prune_backups(&token_clone, &folder_clone, retention);
    });

    Ok(BackupFileInfo {
        id: uploaded.id,
        name: uploaded.name,
        size: uploaded.size.or_else(|| Some(format!("{} KB", content.len() / 1024))),
        created_time: uploaded.created_time.or(Some(now_iso)),
        modified_time: None,
    })
}

/// Lists all backup files in the Google Drive folder
pub fn list_backups(db: &Database) -> Result<Vec<BackupFileInfo>, String> {
    let access_token = get_valid_access_token(db)?;
    let folder_id = get_or_create_folder(&access_token, db)?;

    let query = format!("'{}' in parents and trashed = false", folder_id);
    let list_url = format!(
        "https://www.googleapis.com/drive/v3/files?q={}&orderBy=createdTime desc&fields=files(id,name,size,createdTime,modifiedTime)&pageSize=100",
        url_encode(&query)
    );

    let res = ureq::get(&list_url)
        .set("Authorization", &format!("Bearer {}", access_token))
        .timeout(Duration::from_secs(15))
        .call()
        .map_err(|e| format!("Failed to list Google Drive backups: {e}"))?;

    let list: DriveFileList = res
        .into_json()
        .map_err(|e| format!("Failed to parse Google Drive files JSON: {e}"))?;

    Ok(list.files)
}

/// Downloads and restores a backup from Google Drive into SQLite
pub fn restore_backup(file_id: &str, db: &Database) -> Result<(), String> {
    let access_token = get_valid_access_token(db)?;
    let download_url = format!("https://www.googleapis.com/drive/v3/files/{}?alt=media", file_id.trim());

    let res = ureq::get(&download_url)
        .set("Authorization", &format!("Bearer {}", access_token))
        .timeout(Duration::from_secs(30))
        .call()
        .map_err(|e| format!("Failed to download backup file from Google Drive: {e}"))?;

    let backup_data: FullBackupData = res
        .into_json()
        .map_err(|e| format!("Failed to parse backup content JSON: {e}"))?;

    db.import_full_backup(&backup_data)
        .map_err(|e| format!("Failed to restore ledger data: {e}"))?;

    Ok(())
}

/// Deletes a backup file from Google Drive
pub fn delete_backup(file_id: &str, db: &Database) -> Result<(), String> {
    let access_token = get_valid_access_token(db)?;
    let delete_url = format!("https://www.googleapis.com/drive/v3/files/{}", file_id.trim());

    let _ = ureq::delete(&delete_url)
        .set("Authorization", &format!("Bearer {}", access_token))
        .timeout(Duration::from_secs(15))
        .call()
        .map_err(|e| format!("Failed to delete backup from Google Drive: {e}"))?;

    Ok(())
}

/// Prunes older backups if count exceeds retention count
fn prune_backups(access_token: &str, folder_id: &str, retention: i32) -> Result<(), String> {
    if retention <= 0 {
        return Ok(());
    }

    let query = format!("'{}' in parents and trashed = false", folder_id);
    let list_url = format!(
        "https://www.googleapis.com/drive/v3/files?q={}&orderBy=createdTime desc&fields=files(id,name,createdTime)&pageSize=100",
        url_encode(&query)
    );

    let res = ureq::get(&list_url)
        .set("Authorization", &format!("Bearer {}", access_token))
        .timeout(Duration::from_secs(15))
        .call()
        .map_err(|e| e.to_string())?;

    let list: DriveFileList = res.into_json().map_err(|e| e.to_string())?;
    let files = list.files;

    if files.len() > retention as usize {
        let to_delete = &files[retention as usize..];
        for f in to_delete {
            let delete_url = format!("https://www.googleapis.com/drive/v3/files/{}", f.id);
            let _ = ureq::delete(&delete_url)
                .set("Authorization", &format!("Bearer {}", access_token))
                .timeout(Duration::from_secs(10))
                .call();
        }
    }

    Ok(())
}

/// Disconnect Google Drive credentials
pub fn disconnect(db: &Database) -> Result<(), String> {
    let _ = db.set_setting("gdrive_access_token", "");
    let _ = db.set_setting("gdrive_refresh_token", "");
    let _ = db.set_setting("gdrive_token_expires_at", "0");
    let _ = db.set_setting("gdrive_user_email", "");
    let _ = db.set_setting("gdrive_auto_backup", "false");
    let _ = db.set_setting("gdrive_last_backup_status", "");
    let _ = db.set_setting("gdrive_last_backup_error", "");
    Ok(())
}

/// Interval in seconds for configured frequency
pub fn get_frequency_interval_seconds(frequency: &str) -> Option<i64> {
    match frequency.trim() {
        "hourly" => Some(3600),
        "every_6h" => Some(6 * 3600),
        "daily" => Some(24 * 3600),
        "weekly" => Some(7 * 24 * 3600),
        _ => None, // "manual" or unrecognized frequency
    }
}

/// Parses ISO 8601 / RFC 3339 timestamp strings safely
pub fn parse_timestamp(ts: &str) -> Option<chrono::DateTime<Utc>> {
    let s = ts.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(chrono::DateTime::from_naive_utc_and_offset(naive, Utc));
    }
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S") {
        return Some(chrono::DateTime::from_naive_utc_and_offset(naive, Utc));
    }
    None
}

/// Computes a deterministic SHA-256 hash of the current database ledger content.
/// Excludes transient metadata (like `exported_at` and `gdrive_*` settings) so that
/// unchanged transactions, accounts, budgets, and debts result in identical hashes.
pub fn compute_backup_hash(db: &Database) -> Result<String, String> {
    let mut backup = db
        .export_full_backup()
        .map_err(|e| format!("Failed to export ledger data: {e}"))?;

    backup.exported_at = String::new();
    backup.settings.retain(|(k, _)| !k.starts_with("gdrive_"));

    let json_bytes = serde_json::to_vec(&backup)
        .map_err(|e| format!("Failed to serialize backup: {e}"))?;

    let mut hasher = Sha256::new();
    hasher.update(&json_bytes);
    let result = hasher.finalize();
    let hex_str: String = result.iter().map(|b| format!("{:02x}", b)).collect();
    Ok(hex_str)
}

/// Checks if an automated backup is due according to frequency and last backup timestamp.
pub fn is_backup_due(db: &Database) -> bool {
    let (due, _) = is_backup_due_with_remaining(db);
    due
}

/// Checks if an automated backup is due according to frequency and last backup timestamp.
/// Returns (is_due, seconds_remaining_or_overdue).
pub fn is_backup_due_with_remaining(db: &Database) -> (bool, Option<i64>) {
    let settings = load_settings(db);
    if !settings.auto_backup || settings.refresh_token.trim().is_empty() {
        return (false, None);
    }

    let Some(interval_secs) = get_frequency_interval_seconds(&settings.frequency) else {
        // "manual" or unrecognized frequency
        return (false, None);
    };

    if settings.last_backup_time.trim().is_empty() {
        // Never backed up before -> due immediately
        return (true, Some(0));
    }

    let Some(last_dt) = parse_timestamp(&settings.last_backup_time) else {
        // Timestamp is unparseable -> trigger backup to fix state
        return (true, Some(0));
    };

    let elapsed_secs = Utc::now().signed_duration_since(last_dt).num_seconds();
    if elapsed_secs >= interval_secs {
        (true, Some(0))
    } else {
        (false, Some(interval_secs - elapsed_secs))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncResult {
    /// Fresh backup snapshot uploaded to Google Drive
    Uploaded(BackupFileInfo),
    /// Schedule was reached, but database ledger content is unchanged.
    /// Timestamp updated without costly duplicate upload.
    AlreadyUpToDate {
        last_backup_time: String,
        hash: String,
    },
    /// Not due yet according to schedule and last backup timestamp
    NotDue {
        seconds_remaining: i64,
        frequency: String,
    },
    /// Auto-backup disabled, frequency is manual, or Google Drive not connected
    SkippedNotConfigured,
}

/// Performs scheduled sync if due, or forced sync if `force` is true.
/// Compares content hash to avoid costly redundant uploads when data is unchanged.
pub fn sync_if_due(db: &Database, force: bool) -> Result<SyncResult, String> {
    let settings = load_settings(db);

    if !force {
        if !settings.auto_backup || settings.refresh_token.trim().is_empty() {
            return Ok(SyncResult::SkippedNotConfigured);
        }

        let (due, remaining) = is_backup_due_with_remaining(db);
        if !due {
            return Ok(SyncResult::NotDue {
                seconds_remaining: remaining.unwrap_or(0),
                frequency: settings.frequency,
            });
        }
    } else if settings.refresh_token.trim().is_empty() {
        return Err("Google Drive is not connected (no refresh token found)".to_string());
    }

    // Check content hash to avoid duplicate uploads
    let current_hash = compute_backup_hash(db)?;
    let last_hash = db
        .get_setting("gdrive_last_backup_hash")
        .ok()
        .flatten()
        .unwrap_or_default();

    if !force && !last_hash.is_empty() && current_hash == last_hash {
        let now_iso = Utc::now().to_rfc3339();
        let _ = db.set_setting("gdrive_last_backup_time", &now_iso);
        let _ = db.set_setting("gdrive_last_backup_status", "success");
        let _ = db.set_setting("gdrive_last_backup_error", "");

        return Ok(SyncResult::AlreadyUpToDate {
            last_backup_time: now_iso,
            hash: current_hash,
        });
    }

    let uploaded = upload_backup(db)?;
    Ok(SyncResult::Uploaded(uploaded))
}

pub struct SchedulerHandle {
    running: Arc<AtomicBool>,
}

impl SchedulerHandle {
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Starts an in-app background scheduler daemon thread.
/// Runs inside the application lifecycle, periodically checking schedule and sync state.
/// This completely eliminates the need for external OS cron jobs.
pub fn start_background_scheduler<F>(
    db: Database,
    check_interval: Duration,
    on_event: F,
) -> SchedulerHandle
where
    F: Fn(SyncResult) + Send + 'static,
{
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    std::thread::spawn(move || {
        // Wait 3 seconds on startup before first check
        std::thread::sleep(Duration::from_secs(3));

        // Initial launch check: ONLY syncs if overdue according to last_backup_time!
        if running_clone.load(Ordering::SeqCst) {
            match sync_if_due(&db, false) {
                Ok(res) => on_event(res),
                Err(err) => eprintln!("Google Drive launch sync check warning: {err}"),
            }
        }

        // Periodic ticker while application is open
        while running_clone.load(Ordering::SeqCst) {
            let total_secs = check_interval.as_secs().max(10);
            for _ in 0..total_secs {
                if !running_clone.load(Ordering::SeqCst) {
                    return;
                }
                std::thread::sleep(Duration::from_secs(1));
            }

            if !running_clone.load(Ordering::SeqCst) {
                return;
            }

            match sync_if_due(&db, false) {
                Ok(SyncResult::Uploaded(file)) => {
                    println!("[In-App Scheduler] Auto-backup synced to Google Drive: {}", file.name);
                    on_event(SyncResult::Uploaded(file));
                }
                Ok(SyncResult::AlreadyUpToDate { last_backup_time, hash }) => {
                    println!("[In-App Scheduler] Schedule reached, ledger unchanged. Updated timestamp.");
                    on_event(SyncResult::AlreadyUpToDate { last_backup_time, hash });
                }
                Ok(SyncResult::NotDue { .. }) => {}
                Ok(SyncResult::SkippedNotConfigured) => {}
                Err(err) => {
                    eprintln!("[In-App Scheduler] Backup sync check error: {err}");
                }
            }
        }
    });

    SchedulerHandle { running }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;

    #[test]
    fn test_frequency_interval_seconds() {
        assert_eq!(get_frequency_interval_seconds("hourly"), Some(3600));
        assert_eq!(get_frequency_interval_seconds("every_6h"), Some(21600));
        assert_eq!(get_frequency_interval_seconds("daily"), Some(86400));
        assert_eq!(get_frequency_interval_seconds("weekly"), Some(604800));
        assert_eq!(get_frequency_interval_seconds("manual"), None);
        assert_eq!(get_frequency_interval_seconds("invalid"), None);
    }

    #[test]
    fn test_parse_timestamp() {
        let ts_rfc = "2026-10-26T16:00:00Z";
        assert!(parse_timestamp(ts_rfc).is_some());

        let ts_offset = "2026-10-26T16:00:00+00:00";
        assert!(parse_timestamp(ts_offset).is_some());

        let ts_naive = "2026-10-26T16:00:00";
        assert!(parse_timestamp(ts_naive).is_some());

        assert!(parse_timestamp("").is_none());
        assert!(parse_timestamp("not-a-date").is_none());
    }

    #[test]
    fn test_backup_due_calculations() {
        let db = Database::open_in_memory().unwrap();

        // 1. Not connected / no auto backup -> not due
        assert!(!is_backup_due(&db));

        // 2. Auto-backup enabled, connected, but no prior backup -> due immediately
        let _ = db.set_setting("gdrive_auto_backup", "true");
        let _ = db.set_setting("gdrive_refresh_token", "dummy_token");
        let _ = db.set_setting("gdrive_backup_frequency", "daily");
        assert!(is_backup_due(&db));

        // 3. Recent backup (10 minutes ago, daily schedule) -> NOT due (prevents costly backup on launch!)
        let recent_ts = (Utc::now() - ChronoDuration::minutes(10)).to_rfc3339();
        let _ = db.set_setting("gdrive_last_backup_time", &recent_ts);
        let (due, remaining) = is_backup_due_with_remaining(&db);
        assert!(!due);
        assert!(remaining.unwrap() > 80000);

        // 4. Old backup (25 hours ago, daily schedule) -> IS due
        let old_ts = (Utc::now() - ChronoDuration::hours(25)).to_rfc3339();
        let _ = db.set_setting("gdrive_last_backup_time", &old_ts);
        let (due, remaining) = is_backup_due_with_remaining(&db);
        assert!(due);
        assert_eq!(remaining.unwrap(), 0);

        // 5. Manual frequency -> NEVER due automatically
        let _ = db.set_setting("gdrive_backup_frequency", "manual");
        let (due, _) = is_backup_due_with_remaining(&db);
        assert!(!due);
    }

    #[test]
    fn test_compute_backup_hash_and_smart_sync() {
        let db = Database::open_in_memory().unwrap();

        let hash1 = compute_backup_hash(&db).unwrap();
        assert!(!hash1.is_empty());

        // Hashing again without data changes produces the exact same hash
        let hash2 = compute_backup_hash(&db).unwrap();
        assert_eq!(hash1, hash2);

        // Changing volatile gdrive settings does NOT change the data hash
        let _ = db.set_setting("gdrive_last_backup_time", &Utc::now().to_rfc3339());
        let _ = db.set_setting("gdrive_last_backup_status", "success");
        let hash_after_gdrive_setting = compute_backup_hash(&db).unwrap();
        assert_eq!(hash1, hash_after_gdrive_setting);

        // Adding actual ledger data (e.g. account) changes the hash
        let _ = db.create_account(crate::db::models::CreateAccountInput {
            id: None,
            name: "Checking".to_string(),
            account_type: crate::db::models::AccountType::Bank,
            currency: Some("USD".to_string()),
            balance: Some(1000.0),
            color: Some("#10B981".to_string()),
            icon: Some("wallet".to_string()),
        });
        let hash3 = compute_backup_hash(&db).unwrap();
        assert_ne!(hash1, hash3);

        // Test smart sync unchanged detection
        let _ = db.set_setting("gdrive_auto_backup", "true");
        let _ = db.set_setting("gdrive_refresh_token", "dummy_token");
        let _ = db.set_setting("gdrive_backup_frequency", "daily");
        // Due because 25 hours ago
        let old_ts = (Utc::now() - ChronoDuration::hours(25)).to_rfc3339();
        let _ = db.set_setting("gdrive_last_backup_time", &old_ts);
        // Pre-store current hash as last backup hash
        let _ = db.set_setting("gdrive_last_backup_hash", &hash3);

        // sync_if_due should detect data is unchanged and return AlreadyUpToDate without upload!
        let sync_res = sync_if_due(&db, false).unwrap();
        match sync_res {
            SyncResult::AlreadyUpToDate { hash, .. } => {
                assert_eq!(hash, hash3);
            }
            other => panic!("Expected AlreadyUpToDate, got {:?}", other),
        }

        // And verify that last_backup_time was updated to recent, so it won't be due anymore
        assert!(!is_backup_due(&db));
    }

    #[test]
    fn test_url_encode() {
        assert_eq!(url_encode("hello world"), "hello+world");
        assert_eq!(url_encode("name = 'test' & foo"), "name+%3D+%27test%27+%26+foo");
        assert_eq!(url_encode("abc-123_.*"), "abc-123_.*");
        assert_eq!(url_encode("user@example.com"), "user%40example.com");
    }
}
