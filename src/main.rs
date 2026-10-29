#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use urpwallet::db::Database;

use crate::ui::handlers::{
    accounts::register_account_handlers,
    bills::register_bill_handlers,
    budgets::register_budget_handlers,
    debts::register_debt_handlers,
    gdrive::{refresh_gdrive_state, register_gdrive_handlers},
    io::register_io_handlers,
    settings::register_settings_handlers,
    transactions::register_transaction_handlers,
};
use crate::ui::refresh::refresh_ui;

mod ui;

slint::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Starting urpwallet Native Desktop Application...");

    // 1. Initialize SQLite Database (WAL mode, Foreign Keys)
    let db = Database::open_default()?;
    println!("Database initialized at standard platform user directory.");

    // 2. Instantiate Slint MainWindow
    let main_window = MainWindow::new()?;

    // 3. Check Master PIN protection
    let is_pin_enabled = db.is_pin_enabled().unwrap_or(false);
    main_window.set_pin_protected(is_pin_enabled);
    main_window.set_is_locked(is_pin_enabled);

    // 4. Initial Theme & Privacy Setup
    let initial_privacy = db
        .get_setting("privacy_mode")?
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);
    main_window.set_privacy_mode(initial_privacy);

    let initial_theme = db
        .get_setting("theme")?
        .unwrap_or_else(|| "dark".to_string());
    let theme_mode = match initial_theme.as_str() {
        "light" => 0,
        "dark" => 1,
        "oled" | "midnight" => 2,
        "emerald" => 3,
        _ => 1,
    };
    main_window.global::<Theme>().set_mode(theme_mode);

    // 5. Wire all event handlers & callbacks
    register_settings_handlers(&main_window, &db);
    register_transaction_handlers(&main_window, &db);
    register_account_handlers(&main_window, &db);
    register_budget_handlers(&main_window, &db);
    register_bill_handlers(&main_window, &db);
    register_debt_handlers(&main_window, &db);
    register_io_handlers(&main_window, &db);
    register_gdrive_handlers(&main_window, &db);

    // 6. Initial data population
    refresh_ui(&main_window, &db);
    refresh_gdrive_state(&main_window, &db);

    // 7. In-App Background Scheduler (Continuous Sync Daemon, No Cron Job Required)
    let _scheduler_handle = {
        let ui_weak = main_window.as_weak();
        let db_clone = db.clone();
        urpwallet::gdrive::start_background_scheduler(
            db.clone(),
            std::time::Duration::from_secs(60),
            move |sync_res| {
                let ui_weak = ui_weak.clone();
                let db_clone = db_clone.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = ui_weak.upgrade() else { return; };
                    match sync_res {
                        urpwallet::gdrive::SyncResult::Uploaded(file) => {
                            println!("Scheduled backup uploaded to Google Drive: {}", file.name);
                            refresh_gdrive_state(&ui, &db_clone);
                            ui.set_gdrive_status_msg(format!("✓ Scheduled backup synced: {}", file.name).into());
                        }
                        urpwallet::gdrive::SyncResult::AlreadyUpToDate { last_backup_time, .. } => {
                            ui.set_gdrive_last_backup(last_backup_time.into());
                            ui.set_gdrive_status_msg("✓ Cloud backup in sync (no changes detected)".into());
                        }
                        urpwallet::gdrive::SyncResult::NotDue { .. }
                        | urpwallet::gdrive::SyncResult::SkippedNotConfigured => {}
                    }
                });
            },
        )
    };

    // 8. Run GUI event loop
    main_window.run()?;
    Ok(())
}
