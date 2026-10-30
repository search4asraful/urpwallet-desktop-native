use slint::ComponentHandle;
use urpwallet::db::models::{AccountType, CreateAccountInput, UpdateAccountInput};
use urpwallet::db::Database;

use crate::MainWindow;
use crate::ui::refresh::{refresh_accounts, refresh_ui};
use crate::ui::toast::{show_error, show_success};

/// Registers all account-related event handlers on the main window.
pub fn register_account_handlers(ui: &MainWindow, db: &Database) {
    // J. Open Edit Account
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_open_edit_account(move |acc_id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            if let Ok(acc) = db_clone.get_account(acc_id.as_str()) {
                ui.set_modal_acc_is_editing(true);
                ui.set_modal_acc_id(acc.id.into());
                ui.set_modal_acc_name(acc.name.into());
                ui.set_modal_acc_type(match acc.account_type {
                    AccountType::Bank => 0,
                    AccountType::Cash => 1,
                    AccountType::Savings => 2,
                    AccountType::CreditCard => 3,
                    AccountType::Investment => 4,
                    AccountType::Loan => 5,
                });
                ui.set_modal_acc_balance(format!("{:.2}", acc.balance).into());
                ui.set_modal_acc_currency(acc.currency.into());
                ui.set_show_acc_detail_modal(false);
                ui.set_show_add_account_modal(true);
            }
        });
    }

    // Submit Save Account
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_submit_save_account(move |is_editing, acc_id, name, type_int, balance_str, currency, color| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            if name.trim().is_empty() { return; }

            let acc_type = match type_int {
                0 => AccountType::Bank,
                1 => AccountType::Cash,
                2 => AccountType::Savings,
                3 => AccountType::CreditCard,
                4 => AccountType::Investment,
                5 => AccountType::Loan,
                _ => AccountType::Bank,
            };

            let balance: f64 = balance_str.trim().parse().unwrap_or(0.0);
            let curr = if currency.trim().is_empty() { "BDT".to_string() } else { currency.to_string() };
            let col = if color.trim().is_empty() { "#3B82F6".to_string() } else { color.to_string() };

            let res = if is_editing {
                db_clone.update_account(
                    acc_id.as_str(),
                    UpdateAccountInput {
                        name: Some(name.trim().to_string()),
                        account_type: Some(acc_type),
                        currency: Some(curr),
                        balance: Some(balance),
                        color: Some(col),
                        icon: None,
                        is_archived: None,
                    },
                )
            } else {
                db_clone.create_account(CreateAccountInput {
                    id: None,
                    name: name.trim().to_string(),
                    account_type: acc_type,
                    currency: Some(curr),
                    balance: Some(balance),
                    color: Some(col),
                    icon: Some("Wallet".into()),
                })
            };

            ui.set_show_add_account_modal(false);
            match res {
                Ok(_) => {
                    refresh_ui(&ui, &db_clone);
                    show_success(&ui, if is_editing { "Account updated successfully" } else { "Account created successfully" });
                }
                Err(e) => {
                    show_error(&ui, format!("Failed to save account: {}", e.user_friendly_message()));
                }
            }
        });
    }

    // P. Delete Account
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_delete_account(move |id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            match db_clone.delete_account(id.as_str()) {
                Ok(_) => {
                    refresh_ui(&ui, &db_clone);
                    show_success(&ui, "Account deleted successfully");
                }
                Err(e) => {
                    show_error(&ui, format!("Failed to delete account: {}", e.user_friendly_message()));
                }
            }
        });
    }

    // Filter Accounts Tab
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_filter_accounts(move |tab| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            ui.set_accounts_filter_tab(tab);
            refresh_accounts(&ui, &db_clone);
        });
    }
}
