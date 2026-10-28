use std::path::{Path, PathBuf};
use chrono::Utc;
use slint::ComponentHandle;
use urpwallet::db::Database;

use crate::MainWindow;
use crate::ui::handlers::gdrive::refresh_gdrive_state;
use crate::ui::refresh::refresh_ui;

/// Opens directory in native OS file manager
fn open_in_file_manager(path: &Path) {
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("explorer").arg(path).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(path).spawn();
    }
    #[cfg(target_os = "linux")]
    {
        let _ = std::process::Command::new("xdg-open").arg(path).spawn();
    }
}

/// Registers data import/export, database switching, and export path handlers.
pub fn register_io_handlers(ui: &MainWindow, db: &Database) {
    // 1. Data Export (JSON & CSV) to configured export path
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_export_data(move |fmt| {
            let timestamp = Utc::now().format("%Y%m%d_%H%M%S").to_string();
            let export_dir = Database::get_configured_export_path();
            let _ = std::fs::create_dir_all(&export_dir);

            if fmt == "json" {
                if let Ok(backup) = db_clone.export_full_backup() {
                    let path = export_dir.join(format!("urpwallet_backup_{}.json", timestamp));
                    if let Ok(json_str) = serde_json::to_string_pretty(&backup) {
                        if std::fs::write(&path, json_str).is_ok() {
                            println!("Full JSON backup successfully exported to: {:?}", path);
                            if let Some(ui) = ui_weak.upgrade() {
                                ui.set_settings_status_msg(format!("✓ Full JSON backup exported to: {}", path.display()).into());
                            }
                        } else if let Some(ui) = ui_weak.upgrade() {
                            ui.set_settings_status_msg("Failed to write export file. Please check folder permissions.".into());
                        }
                    }
                }
            } else if fmt == "csv" {
                if let Ok(csv_str) = db_clone.export_transactions_csv() {
                    let path = export_dir.join(format!("urpwallet_transactions_{}.csv", timestamp));
                    if std::fs::write(&path, csv_str).is_ok() {
                        println!("CSV transactions history exported to: {:?}", path);
                        if let Some(ui) = ui_weak.upgrade() {
                            ui.set_settings_status_msg(format!("✓ Transactions CSV exported to: {}", path.display()).into());
                        }
                    } else if let Some(ui) = ui_weak.upgrade() {
                        ui.set_settings_status_msg("Failed to write export file. Please check folder permissions.".into());
                    }
                }
            }
        });
    }

    // 2. Data Import (with native file picker or latest from export directory)
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_import_backup(move || {
            let ui_bg = ui_weak.clone();
            let db_bg = db_clone.clone();

            std::thread::spawn(move || {
                // Try file picker first
                let chosen_file = rfd::FileDialog::new()
                    .set_title("Select urpwallet JSON Backup File")
                    .add_filter("JSON Backup (*.json)", &["json"])
                    .pick_file();

                let file_to_import = match chosen_file {
                    Some(f) => Some(f),
                    None => {
                        // Fallback to latest in export dir or data/
                        let export_dir = Database::get_configured_export_path();
                        let mut backups: Vec<PathBuf> = Vec::new();
                        if let Ok(entries) = std::fs::read_dir(&export_dir) {
                            for e in entries.flatten() {
                                if e.path().extension().is_some_and(|ext| ext == "json") {
                                    backups.push(e.path());
                                }
                            }
                        }
                        if let Ok(entries) = std::fs::read_dir("data") {
                            for e in entries.flatten() {
                                if e.path().extension().is_some_and(|ext| ext == "json") {
                                    backups.push(e.path());
                                }
                            }
                        }
                        backups.sort();
                        backups.pop()
                    }
                };

                if let Some(file_path) = file_to_import {
                    match std::fs::read_to_string(&file_path) {
                        Ok(content) => match serde_json::from_str::<urpwallet::db::models::FullBackupData>(&content) {
                            Ok(data) => match db_bg.import_full_backup(&data) {
                                Ok(_) => {
                                    println!("Imported backup from: {:?}", file_path);
                                    let _ = slint::invoke_from_event_loop(move || {
                                        let Some(ui) = ui_bg.upgrade() else { return; };
                                        refresh_ui(&ui, &db_bg);
                                        ui.set_settings_status_msg(format!("✓ Ledger restored from: {}", file_path.display()).into());
                                    });
                                }
                                Err(e) => {
                                    let _ = slint::invoke_from_event_loop(move || {
                                        let Some(ui) = ui_bg.upgrade() else { return; };
                                        ui.set_settings_status_msg(format!("Could not restore ledger: {}", e.user_friendly_message()).into());
                                    });
                                }
                            },
                            Err(_) => {
                                let _ = slint::invoke_from_event_loop(move || {
                                    let Some(ui) = ui_bg.upgrade() else { return; };
                                    ui.set_settings_status_msg("Invalid backup file. The JSON format is corrupted or unrecognized.".into());
                                });
                            }
                        },
                        Err(_) => {
                            let _ = slint::invoke_from_event_loop(move || {
                                let Some(ui) = ui_bg.upgrade() else { return; };
                                ui.set_settings_status_msg("Unable to read the selected backup file. Please check file permissions.".into());
                            });
                        }
                    }
                }
            });
        });
    }

    // 3. Select Database File via Native File Dialog
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_select_db_file(move || {
            let ui_bg = ui_weak.clone();
            let db_bg = db_clone.clone();
            std::thread::spawn(move || {
                let file = rfd::FileDialog::new()
                    .set_title("Select SQLite Database File")
                    .add_filter("SQLite Database (*.db, *.sqlite)", &["db", "sqlite", "sqlite3"])
                    .pick_file();

                if let Some(path) = file {
                    let res = db_bg.switch_database(path.clone(), false);
                    let _ = slint::invoke_from_event_loop(move || {
                        let Some(ui) = ui_bg.upgrade() else { return; };
                        match res {
                            Ok(_) => {
                                refresh_ui(&ui, &db_bg);
                                refresh_gdrive_state(&ui, &db_bg);
                                ui.set_settings_status_msg(format!("✓ Switched database to: {}", path.display()).into());
                            }
                            Err(e) => {
                                ui.set_settings_status_msg(format!("Failed to switch database: {}", e.user_friendly_message()).into());
                            }
                        }
                    });
                }
            });
        });
    }

    // 4. Set Custom Database Path Manually
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_set_custom_db_path(move |path_str, copy_existing| {
            let p = path_str.trim().to_string();
            if p.is_empty() { return; }
            let path = PathBuf::from(&p);
            let ui_bg = ui_weak.clone();
            let db_bg = db_clone.clone();
            std::thread::spawn(move || {
                let res = db_bg.switch_database(path.clone(), copy_existing);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_bg.upgrade() else { return; };
                    match res {
                        Ok(_) => {
                            refresh_ui(&ui, &db_bg);
                            refresh_gdrive_state(&ui, &db_bg);
                            ui.set_settings_status_msg(format!("✓ Switched database to: {}", path.display()).into());
                        }
                        Err(e) => {
                            ui.set_settings_status_msg(format!("Failed to switch database: {}", e.user_friendly_message()).into());
                        }
                    }
                });
            });
        });
    }

    // 5. Reset to System Default Database Path
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_reset_default_db(move || {
            let Some(ui) = ui_weak.upgrade() else { return; };
            match db_clone.reset_default_database() {
                Ok(_) => {
                    refresh_ui(&ui, &db_clone);
                    refresh_gdrive_state(&ui, &db_clone);
                    ui.set_settings_status_msg("✓ Database reset to standard system location".into());
                }
                Err(e) => {
                    ui.set_settings_status_msg(format!("Failed to reset database: {}", e.user_friendly_message()).into());
                }
            }
        });
    }

    // 6. Select Export Folder via Native Folder Dialog
    {
        let ui_weak = ui.as_weak();
        ui.on_select_export_folder(move || {
            let ui_bg = ui_weak.clone();
            std::thread::spawn(move || {
                let folder = rfd::FileDialog::new()
                    .set_title("Select Export Directory")
                    .pick_folder();

                if let Some(path) = folder {
                    let res = Database::set_configured_export_path(&path);
                    let _ = slint::invoke_from_event_loop(move || {
                        let Some(ui) = ui_bg.upgrade() else { return; };
                        match res {
                            Ok(_) => {
                                ui.set_export_storage_path(path.to_string_lossy().to_string().into());
                                ui.set_settings_status_msg(format!("✓ Export destination set to: {}", path.display()).into());
                            }
                            Err(e) => {
                                ui.set_settings_status_msg(format!("Failed to set export path: {}", urpwallet::db::DbError::Io(e).user_friendly_message()).into());
                            }
                        }
                    });
                }
            });
        });
    }

    // 7. Set Custom Export Path Manually
    {
        let ui_weak = ui.as_weak();
        ui.on_set_custom_export_path(move |path_str| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let p = path_str.trim().to_string();
            if p.is_empty() { return; }
            let path = PathBuf::from(&p);
            match Database::set_configured_export_path(&path) {
                Ok(_) => {
                    ui.set_export_storage_path(path.to_string_lossy().to_string().into());
                    ui.set_settings_status_msg(format!("✓ Export destination set to: {}", path.display()).into());
                }
                Err(e) => {
                    ui.set_settings_status_msg(format!("Failed to set export path: {}", urpwallet::db::DbError::Io(e).user_friendly_message()).into());
                }
            }
        });
    }

    // 8. Open Export Folder in Native OS File Manager
    {
        let ui_weak = ui.as_weak();
        ui.on_open_export_folder(move || {
            let export_dir = Database::get_configured_export_path();
            let _ = std::fs::create_dir_all(&export_dir);
            open_in_file_manager(&export_dir);
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_settings_status_msg(format!("Opened folder: {}", export_dir.display()).into());
            }
        });
    }
}
