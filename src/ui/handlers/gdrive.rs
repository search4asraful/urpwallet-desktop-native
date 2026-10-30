use std::rc::Rc;
use std::time::Duration;
use slint::{ComponentHandle, ModelRc, VecModel};
use urpwallet::db::Database;
use urpwallet::gdrive;

use crate::{GDriveBackupItem, MainWindow};
use crate::ui::refresh::refresh_ui;
use crate::ui::toast::{show_error, show_success};

/// Converts technical Google Drive errors into clean, human-readable explanations.
fn humanize_gdrive_error(context: &str, raw_err: &str) -> String {
    let err_lower = raw_err.to_lowercase();
    if err_lower.contains("invalid_grant") || err_lower.contains("expired") || err_lower.contains("code has already been used") {
        "Google authorization code expired or has already been used. Please click Connect to sign in again.".to_string()
    } else if err_lower.contains("invalid_client") || err_lower.contains("unauthorized_client") {
        "Google credentials rejected. Please verify your Client ID and Client Secret in Settings.".to_string()
    } else if err_lower.contains("quota") || err_lower.contains("storage") {
        "Google Drive storage quota exceeded. Please free up space in your Google Drive and try again.".to_string()
    } else if err_lower.contains("addrinuse") || err_lower.contains("address already in use") || err_lower.contains("port") {
        "Local authorization port 8080 is in use by another app. Please paste the authorization code manually below.".to_string()
    } else if err_lower.contains("connection refused") || err_lower.contains("timeout") || err_lower.contains("timed out") || err_lower.contains("dns") || err_lower.contains("connect") {
        "Unable to reach Google servers. Please check your internet connection and try again.".to_string()
    } else if err_lower.contains("401") || err_lower.contains("token") {
        "Google Drive session expired or access was revoked. Please disconnect and reconnect your account.".to_string()
    } else if err_lower.contains("404") || err_lower.contains("not found") {
        "The requested file was not found on Google Drive.".to_string()
    } else {
        match context {
            "auth" => "Google authorization could not be completed. Please try signing in again.".to_string(),
            "upload" => "Could not upload backup to Google Drive. Please check your internet connection and try again.".to_string(),
            "list" => "Could not load backups from Google Drive. Please check your internet connection.".to_string(),
            "restore" => "Could not restore backup from Google Drive. The file may be damaged or unreachable.".to_string(),
            "delete" => "Could not delete backup from Google Drive. Please try again.".to_string(),
            "config" => "Could not save Google Drive configuration. Please check your inputs.".to_string(),
            _ => "An error occurred while communicating with Google Drive. Please try again.".to_string(),
        }
    }
}

/// Refreshes the Google Drive connection state and backups list on the UI
pub fn refresh_gdrive_state(ui: &MainWindow, db: &Database) {
    let settings = gdrive::load_settings(db);
    let is_connected = !settings.refresh_token.trim().is_empty();

    ui.set_gdrive_connected(is_connected);
    ui.set_gdrive_email(settings.user_email.into());
    ui.set_gdrive_client_id(settings.client_id.into());
    ui.set_gdrive_client_secret(settings.client_secret.into());
    ui.set_gdrive_auto_backup(settings.auto_backup);
    ui.set_gdrive_frequency(settings.frequency.into());
    ui.set_gdrive_retention_count(settings.retention_count);
    ui.set_gdrive_last_backup(settings.last_backup_time.into());

    if is_connected {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        std::thread::spawn(move || {
            let res = gdrive::list_backups(&db_clone);
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = ui_weak.upgrade() else { return; };
                if let Ok(files) = res {
                    let items: Vec<GDriveBackupItem> = files
                        .into_iter()
                        .map(|f| GDriveBackupItem {
                            id: f.id.into(),
                            name: f.name.into(),
                            size: f.size.unwrap_or_else(|| "--".into()).into(),
                            created_time: f.created_time.unwrap_or_else(|| "--".into()).into(),
                        })
                        .collect();
                    ui.set_gdrive_backups(ModelRc::from(Rc::new(VecModel::from(items))));
                }
            });
        });
    } else {
        ui.set_gdrive_backups(ModelRc::from(Rc::new(VecModel::default())));
    }
}

/// Registers all Google Drive event handlers on the main window.
pub fn register_gdrive_handlers(ui: &MainWindow, db: &Database) {
    // 1. Open Google Drive Modal
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_open_gdrive_modal(move || {
            let Some(ui) = ui_weak.upgrade() else { return; };
            ui.set_gdrive_error_msg("".into());
            ui.set_gdrive_status_msg("".into());
            ui.set_show_gdrive_modal(true);
            refresh_gdrive_state(&ui, &db_clone);
        });
    }

    // 2. Connect via Browser OAuth
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_gdrive_connect(move || {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let settings = gdrive::load_settings(&db_clone);

            if settings.client_id.trim().is_empty() {
                ui.set_gdrive_error_msg("Please configure Google Client ID first in the Settings tab.".into());
                return;
            }

            ui.set_gdrive_status_msg("Opening browser for Google authorization...".into());
            ui.set_gdrive_error_msg("".into());
            ui.set_gdrive_loading(true);

            let ui_bg = ui.as_weak();
            let db_bg = db_clone.clone();

            std::thread::spawn(move || {
                match gdrive::start_oauth_listener(9008) {
                    Ok((port, rx)) => {
                        let redirect_uri = format!("http://127.0.0.1:{}/", port);
                        let auth_url = gdrive::build_auth_url(&settings.client_id, &redirect_uri);
                        let _ = webbrowser::open(&auth_url);

                        match rx.recv_timeout(Duration::from_secs(120)) {
                            Ok(Ok(code)) => {
                                match gdrive::exchange_code_for_tokens(
                                    &db_bg,
                                    &code,
                                    &settings.client_id,
                                    &settings.client_secret,
                                    &redirect_uri,
                                ) {
                                    Ok(email) => {
                                        let _ = slint::invoke_from_event_loop(move || {
                                            if let Some(ui) = ui_bg.upgrade() {
                                                ui.set_gdrive_connected(true);
                                                ui.set_gdrive_email(email.into());
                                                ui.set_gdrive_status_msg("✓ Connected to Google Drive successfully!".into());
                                                ui.set_gdrive_error_msg("".into());
                                                ui.set_gdrive_loading(false);
                                                refresh_gdrive_state(&ui, &db_bg);
                                            }
                                        });
                                    }
                                    Err(err) => {
                                        let _ = slint::invoke_from_event_loop(move || {
                                            if let Some(ui) = ui_bg.upgrade() {
                                                ui.set_gdrive_error_msg(humanize_gdrive_error("auth", &err.to_string()).into());
                                                ui.set_gdrive_loading(false);
                                            }
                                        });
                                    }
                                }
                            }
                            Ok(Err(err)) => {
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = ui_bg.upgrade() {
                                        ui.set_gdrive_error_msg(humanize_gdrive_error("auth", &err.to_string()).into());
                                        ui.set_gdrive_loading(false);
                                    }
                                });
                            }
                            Err(_) => {
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = ui_bg.upgrade() {
                                        ui.set_gdrive_error_msg("Google login timed out or browser was closed. You can also paste the authorization code manually.".into());
                                        ui.set_gdrive_loading(false);
                                    }
                                });
                            }
                        }
                    }
                    Err(e) => {
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_bg.upgrade() {
                                ui.set_gdrive_error_msg(humanize_gdrive_error("auth", &e.to_string()).into());
                                ui.set_gdrive_loading(false);
                            }
                        });
                    }
                }
            });
        });
    }

    // 3. Submit Code Manually
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_gdrive_submit_code(move |code| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let code_str = code.trim().to_string();
            if code_str.is_empty() {
                ui.set_gdrive_error_msg("Please enter an authorization code".into());
                return;
            }

            let settings = gdrive::load_settings(&db_clone);
            ui.set_gdrive_loading(true);
            ui.set_gdrive_status_msg("Exchanging authorization code...".into());

            let ui_bg = ui.as_weak();
            let db_bg = db_clone.clone();

            std::thread::spawn(move || {
                let redirect_uri = "http://127.0.0.1:9008/";
                let res = gdrive::exchange_code_for_tokens(
                    &db_bg,
                    &code_str,
                    &settings.client_id,
                    &settings.client_secret,
                    redirect_uri,
                ).or_else(|_| {
                    gdrive::exchange_code_for_tokens(
                        &db_bg,
                        &code_str,
                        &settings.client_id,
                        &settings.client_secret,
                        "urn:ietf:wg:oauth:2.0:oob",
                    )
                });

                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_bg.upgrade() else { return; };
                    match res {
                        Ok(email) => {
                            ui.set_gdrive_connected(true);
                            ui.set_gdrive_email(email.into());
                            ui.set_gdrive_status_msg("✓ Connected to Google Drive successfully!".into());
                            ui.set_gdrive_error_msg("".into());
                            refresh_gdrive_state(&ui, &db_bg);
                        }
                        Err(e) => {
                            ui.set_gdrive_error_msg(humanize_gdrive_error("auth", &e.to_string()).into());
                        }
                    }
                    ui.set_gdrive_loading(false);
                });
            });
        });
    }

    // 4. Disconnect Google Drive
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_gdrive_disconnect(move || {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let _ = gdrive::disconnect(&db_clone);
            ui.set_gdrive_connected(false);
            ui.set_gdrive_email("".into());
            ui.set_gdrive_status_msg("Google Drive disconnected. Local data unaffected.".into());
            ui.set_gdrive_error_msg("".into());
            ui.set_gdrive_backups(ModelRc::from(Rc::new(VecModel::default())));
        });
    }

    // 5. Backup Now
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_gdrive_backup_now(move || {
            let Some(ui) = ui_weak.upgrade() else { return; };
            ui.set_gdrive_loading(true);
            ui.set_gdrive_status_msg("Generating snapshot and uploading to Google Drive...".into());
            ui.set_gdrive_error_msg("".into());

            let ui_bg = ui.as_weak();
            let db_bg = db_clone.clone();

            std::thread::spawn(move || {
                let res = gdrive::upload_backup(&db_bg);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_bg.upgrade() else { return; };
                    match res {
                        Ok(file) => {
                            let msg = format!("Backup uploaded: {}", file.name);
                            ui.set_gdrive_status_msg(format!("✓ {msg}").into());
                            ui.set_gdrive_error_msg("".into());
                            refresh_gdrive_state(&ui, &db_bg);
                            show_success(&ui, msg);
                        }
                        Err(e) => {
                            let err_msg = humanize_gdrive_error("upload", &e.to_string());
                            ui.set_gdrive_error_msg(err_msg.clone().into());
                            ui.set_gdrive_status_msg("".into());
                            show_error(&ui, err_msg);
                        }
                    }
                    ui.set_gdrive_loading(false);
                });
            });
        });
    }

    // 6. Refresh Backups List
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_gdrive_refresh_backups(move || {
            let Some(ui) = ui_weak.upgrade() else { return; };
            ui.set_gdrive_loading(true);

            let ui_bg = ui.as_weak();
            let db_bg = db_clone.clone();

            std::thread::spawn(move || {
                let res = gdrive::list_backups(&db_bg);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_bg.upgrade() else { return; };
                    match res {
                        Ok(files) => {
                            let items: Vec<GDriveBackupItem> = files
                                .into_iter()
                                .map(|f| GDriveBackupItem {
                                    id: f.id.into(),
                                    name: f.name.into(),
                                    size: f.size.unwrap_or_else(|| "--".into()).into(),
                                    created_time: f.created_time.unwrap_or_else(|| "--".into()).into(),
                                })
                                .collect();
                            ui.set_gdrive_backups(ModelRc::from(Rc::new(VecModel::from(items))));
                            ui.set_gdrive_status_msg("Backups list refreshed".into());
                        }
                        Err(e) => {
                            ui.set_gdrive_error_msg(humanize_gdrive_error("list", &e.to_string()).into());
                        }
                    }
                    ui.set_gdrive_loading(false);
                });
            });
        });
    }

    // 7. Restore Backup
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_gdrive_restore_backup(move |file_id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            ui.set_gdrive_loading(true);
            ui.set_gdrive_status_msg("Downloading and restoring ledger snapshot...".into());

            let ui_bg = ui.as_weak();
            let db_bg = db_clone.clone();
            let fid = file_id.to_string();

            std::thread::spawn(move || {
                let res = gdrive::restore_backup(&fid, &db_bg);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_bg.upgrade() else { return; };
                    match res {
                        Ok(_) => {
                            ui.set_gdrive_status_msg("✓ Ledger restored successfully from Google Drive!".into());
                            ui.set_gdrive_error_msg("".into());
                            refresh_ui(&ui, &db_bg);
                            show_success(&ui, "Ledger restored successfully from Google Drive!");
                        }
                        Err(e) => {
                            let err_msg = humanize_gdrive_error("restore", &e.to_string());
                            ui.set_gdrive_error_msg(err_msg.clone().into());
                            show_error(&ui, err_msg);
                        }
                    }
                    ui.set_gdrive_loading(false);
                });
            });
        });
    }

    // 8. Delete Backup
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_gdrive_delete_backup(move |file_id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            ui.set_gdrive_loading(true);

            let ui_bg = ui.as_weak();
            let db_bg = db_clone.clone();
            let fid = file_id.to_string();

            std::thread::spawn(move || {
                let res = gdrive::delete_backup(&fid, &db_bg);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_bg.upgrade() else { return; };
                    match res {
                        Ok(_) => {
                            ui.set_gdrive_status_msg("Backup deleted from Google Drive".into());
                            refresh_gdrive_state(&ui, &db_bg);
                            show_success(&ui, "Backup deleted from Google Drive");
                        }
                        Err(e) => {
                            let err_msg = humanize_gdrive_error("delete", &e.to_string());
                            ui.set_gdrive_error_msg(err_msg.clone().into());
                            show_error(&ui, err_msg);
                        }
                    }
                    ui.set_gdrive_loading(false);
                });
            });
        });
    }

    // 9. Save Configuration
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_gdrive_save_config(move |client_id, client_secret, auto_backup, freq, ret_count| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            match gdrive::save_config(
                &db_clone,
                client_id.as_str(),
                client_secret.as_str(),
                auto_backup,
                freq.as_str(),
                ret_count,
            ) {
                Ok(_) => {
                    ui.set_gdrive_status_msg("✓ Google Drive configuration saved successfully".into());
                    ui.set_gdrive_error_msg("".into());
                    refresh_gdrive_state(&ui, &db_clone);
                    show_success(&ui, "Google Drive configuration saved successfully");
                }
                Err(e) => {
                    let err_msg = humanize_gdrive_error("config", &e.to_string());
                    ui.set_gdrive_error_msg(err_msg.clone().into());
                    show_error(&ui, err_msg);
                }
            }
        });
    }
}
