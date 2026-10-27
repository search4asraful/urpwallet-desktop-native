use slint::ComponentHandle;
use urpwallet::db::models::{CreateDebtInput, DebtType, UpdateDebtInput};
use urpwallet::db::Database;

use crate::MainWindow;
use crate::ui::refresh::{refresh_debts, refresh_ui};

/// Registers all debt-related event handlers on the main window.
pub fn register_debt_handlers(ui: &MainWindow, db: &Database) {
    // M. Open Edit Debt
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_open_edit_debt(move |debt_id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            if let Ok(d) = db_clone.get_debt(debt_id.as_str()) {
                ui.set_modal_debt_is_editing(true);
                ui.set_modal_debt_id(d.id.into());
                ui.set_modal_debt_person(d.person_name.into());
                ui.set_modal_debt_type(if d.debt_type == DebtType::LoanOwedToMe { 0 } else { 1 });
                ui.set_modal_debt_amount(format!("{:.2}", d.amount).into());
                ui.set_modal_debt_notes(d.notes.unwrap_or_default().into());
                ui.set_modal_debt_due_date(d.due_date.unwrap_or_default().into());
                ui.set_show_debt_detail_modal(false);
                ui.set_show_add_debt_modal(true);
            }
        });
    }

    // Submit Save Debt
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_submit_save_debt(move |is_editing, debt_id, person, type_int, amount_str, due_date, notes| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let amount: f64 = match amount_str.trim().parse() {
                Ok(v) if v > 0.0 => v,
                _ => return,
            };

            let debt_type = if type_int == 0 { DebtType::LoanOwedToMe } else { DebtType::DebtIOwe };

            if is_editing {
                let _ = db_clone.update_debt(
                    debt_id.as_str(),
                    UpdateDebtInput {
                        debt_type: Some(debt_type),
                        person_name: Some(person.trim().to_string()),
                        amount: Some(amount),
                        settled_amount: None,
                        due_date: Some(if due_date.trim().is_empty() { None } else { Some(due_date.trim().to_string()) }),
                        notes: Some(if notes.trim().is_empty() { None } else { Some(notes.trim().to_string()) }),
                        status: None,
                    },
                );
            } else {
                let _ = db_clone.create_debt(CreateDebtInput {
                    id: None,
                    debt_type,
                    person_name: person.trim().to_string(),
                    amount,
                    settled_amount: Some(0.0),
                    due_date: if due_date.trim().is_empty() { None } else { Some(due_date.trim().to_string()) },
                    notes: if notes.trim().is_empty() { None } else { Some(notes.trim().to_string()) },
                });
            }

            ui.set_show_add_debt_modal(false);
            refresh_ui(&ui, &db_clone);
        });
    }

    // N. Submit Repay Debt
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_submit_repay_debt(move |debt_id, amount_str| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let amount: f64 = match amount_str.trim().parse() {
                Ok(v) if v > 0.0 => v,
                _ => return,
            };

            let acc_id = db_clone
                .list_accounts(false)
                .ok()
                .and_then(|a| a.first().map(|acc| acc.id.clone()));
            let _ = db_clone.record_debt_payment(debt_id.as_str(), amount, acc_id.as_deref());

            ui.set_show_repay_debt_modal(false);
            refresh_ui(&ui, &db_clone);
        });
    }

    // P. Delete Debt
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_delete_debt(move |id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            if db_clone.delete_debt(id.as_str()).is_ok() {
                refresh_ui(&ui, &db_clone);
            }
        });
    }

    // Filter Debts Tab
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_filter_debts(move |tab| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            ui.set_debts_filter_tab(tab);
            refresh_debts(&ui, &db_clone);
        });
    }
}
