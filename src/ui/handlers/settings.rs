use slint::ComponentHandle;
use urpwallet::db::models::{CategoryType, CreateCategoryInput, UpdateCategoryInput};
use urpwallet::db::Database;

use crate::MainWindow;
use crate::ui::refresh::{filter_categories_in_memory, refresh_ui};
use crate::ui::toast::{show_error, show_success};

/// Registers all settings-related event handlers on the main window.
pub fn register_settings_handlers(ui: &MainWindow, db: &Database) {
    // Toggle Sidebar Collapsed
    {
        let db_clone = db.clone();
        ui.on_toggle_sidebar_collapsed(move |collapsed| {
            let _ = db_clone.set_setting("sidebar_collapsed", if collapsed { "true" } else { "false" });
        });
    }

    // A. Toggle Privacy Mode
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_toggle_privacy(move || {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let new_privacy = !ui.get_privacy_mode();
            ui.set_privacy_mode(new_privacy);
            let _ = db_clone.set_setting("privacy_mode", if new_privacy { "true" } else { "false" });
            refresh_ui(&ui, &db_clone);
        });
    }

    // B. Switch Theme
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_set_theme(move |mode| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            ui.global::<crate::Theme>().set_mode(mode);
            let theme_str = match mode {
                0 => "light",
                1 => "dark",
                2 => "oled",
                3 => "emerald",
                _ => "dark",
            };
            let _ = db_clone.set_setting("theme", theme_str);
        });
    }

    // C. Switch Navigation Tab
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_switch_tab(move |tab_index| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            ui.set_active_tab(tab_index);
            refresh_ui(&ui, &db_clone);
        });
    }

    // D. Unlock Master PIN
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_unlock_pin(move |pin| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            match db_clone.verify_master_pin(pin.as_str()) {
                Ok(true) => {
                    ui.set_is_locked(false);
                    ui.set_pin_error("".into());
                    refresh_ui(&ui, &db_clone);
                }
                _ => {
                    ui.set_pin_error("Invalid master PIN / passphrase. Please try again.".into());
                }
            }
        });
    }

    // E. Close All Modals
    {
        let ui_weak = ui.as_weak();
        ui.on_close_all_modals(move || {
            let Some(ui) = ui_weak.upgrade() else { return; };
            ui.set_show_add_tx_modal(false);
            ui.set_show_tx_detail_modal(false);
            ui.set_show_acc_detail_modal(false);
            ui.set_show_add_account_modal(false);
            ui.set_show_budget_detail_modal(false);
            ui.set_show_add_budget_modal(false);
            ui.set_show_bill_detail_modal(false);
            ui.set_show_add_bill_modal(false);
            ui.set_show_debt_detail_modal(false);
            ui.set_show_add_debt_modal(false);
            ui.set_show_repay_debt_modal(false);
            ui.set_show_gdrive_modal(false);
            ui.set_modal_cat_show_all(false);
        });
    }

    // T. Master PIN Management
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_set_master_pin(move |new_pin| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            match db_clone.set_master_pin(new_pin.as_str()) {
                Ok(_) => {
                    ui.set_pin_protected(true);
                    show_success(&ui, "Master PIN enabled/updated successfully");
                }
                Err(e) => {
                    show_error(&ui, e.user_friendly_message());
                }
            }
        });
    }

    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_disable_master_pin(move |current_pin| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            match db_clone.disable_master_pin(current_pin.as_str()) {
                Ok(_) => {
                    ui.set_pin_protected(false);
                    show_success(&ui, "Master PIN disabled");
                }
                Err(e) => {
                    show_error(&ui, e.user_friendly_message());
                }
            }
        });
    }

    // U. Base Currency & Regional Preferences
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_update_base_currency(move |currency| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let curr_str = currency.as_str();
            let _ = db_clone.set_setting("base_currency", curr_str);
            ui.set_base_currency(currency.clone());
            show_success(&ui, format!("Base currency updated to {curr_str}"));
            refresh_ui(&ui, &db_clone);
        });
    }

    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_update_date_format(move |fmt| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let fmt_str = fmt.as_str();
            let _ = db_clone.set_setting("date_format", fmt_str);
            ui.set_date_format(fmt.clone());
            show_success(&ui, format!("Date presentation format set to {fmt_str}"));
            refresh_ui(&ui, &db_clone);
        });
    }

    // V. Category Catalog Management
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_add_category(move |name, cat_type_idx, color| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let name_str = name.trim().to_string();
            if name_str.is_empty() {
                show_error(&ui, "Category name cannot be empty");
                return;
            }
            let cat_type = if cat_type_idx == 1 {
                CategoryType::Income
            } else {
                CategoryType::Expense
            };
            let input = CreateCategoryInput {
                id: None,
                name: name_str.clone(),
                category_type: cat_type,
                icon: Some("Tag".into()),
                color: Some(color.to_string()),
                parent_id: None,
            };

            match db_clone.create_category(input) {
                Ok(_) => {
                    show_success(&ui, format!("Category '{name_str}' created successfully"));
                    refresh_ui(&ui, &db_clone);
                }
                Err(e) => {
                    show_error(&ui, e.user_friendly_message());
                }
            }
        });
    }

    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_edit_category(move |id, name, cat_type_idx, color| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let name_str = name.trim().to_string();
            if name_str.is_empty() {
                show_error(&ui, "Category name cannot be empty");
                return;
            }
            let cat_type = if cat_type_idx == 1 {
                CategoryType::Income
            } else {
                CategoryType::Expense
            };
            let input = UpdateCategoryInput {
                name: Some(name_str.clone()),
                category_type: Some(cat_type),
                icon: None,
                color: Some(color.to_string()),
                parent_id: None,
            };

            match db_clone.update_category(id.as_str(), input) {
                Ok(_) => {
                    show_success(&ui, format!("Category '{name_str}' updated successfully"));
                    refresh_ui(&ui, &db_clone);
                }
                Err(e) => {
                    show_error(&ui, e.user_friendly_message());
                }
            }
        });
    }

    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_delete_category(move |id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            match db_clone.delete_category(id.as_str()) {
                Ok(_) => {
                    show_success(&ui, "Category deleted successfully");
                    refresh_ui(&ui, &db_clone);
                }
                Err(e) => {
                    show_error(&ui, e.user_friendly_message());
                }
            }
        });
    }

    // W. Reset & Seed Demo Data
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_reset_and_seed_demo(move || {
            let Some(ui) = ui_weak.upgrade() else { return; };
            match db_clone.reset_and_seed_demo() {
                Ok(_) => {
                    show_success(&ui, "Demo data successfully reset and seeded!");
                    refresh_ui(&ui, &db_clone);
                }
                Err(_) => {
                    show_error(&ui, "Unable to reset demo data. Please try again.");
                }
            }
        });
    }

    // Filter Categories Tab
    {
        let ui_weak = ui.as_weak();
        ui.on_filter_categories(move |tab| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            ui.set_cat_catalog_filter(tab);
            filter_categories_in_memory(&ui);
        });
    }

    // Search Categories in real-time
    {
        let ui_weak = ui.as_weak();
        ui.on_search_categories(move |query| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            ui.set_cat_search_query(query);
            filter_categories_in_memory(&ui);
        });
    }
}
