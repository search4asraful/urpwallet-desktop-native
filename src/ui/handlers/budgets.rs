use chrono::Utc;
use slint::ComponentHandle;
use urpwallet::db::models::{CreateBudgetInput, UpdateBudgetInput};
use urpwallet::db::Database;

use crate::MainWindow;
use crate::ui::refresh::refresh_ui;

/// Registers all budget-related event handlers on the main window.
pub fn register_budget_handlers(ui: &MainWindow, db: &Database) {
    // K. Open Edit Budget
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_open_edit_budget(move |b_id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            if let Ok(b) = db_clone.get_budget(b_id.as_str()) {
                ui.set_modal_budget_is_editing(true);
                ui.set_modal_budget_id(b.id.into());
                ui.set_modal_budget_name(b.name.into());
                ui.set_modal_budget_amount(format!("{:.2}", b.amount).into());
                ui.set_modal_budget_category_id(b.category_id.unwrap_or_default().into());
                ui.set_show_budget_detail_modal(false);
                ui.set_show_add_budget_modal(true);
            }
        });
    }

    // Submit Save Budget
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_submit_save_budget(move |is_editing, b_id, name, amount_str, category_id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let amount: f64 = match amount_str.trim().parse() {
                Ok(v) if v > 0.0 => v,
                _ => return,
            };

            let year_month = Utc::now().format("%Y-%m").to_string();
            let start_date = format!("{}-01", year_month);
            let end_date = format!("{}-31", year_month);
            let cat = if category_id.is_empty() { None } else { Some(category_id.to_string()) };

            if is_editing {
                let _ = db_clone.update_budget(
                    b_id.as_str(),
                    UpdateBudgetInput {
                        name: Some(name.trim().to_string()),
                        amount: Some(amount),
                        period: None,
                        category_id: Some(cat),
                        start_date: None,
                        end_date: None,
                    },
                );
            } else {
                let _ = db_clone.create_budget(CreateBudgetInput {
                    id: None,
                    name: name.trim().to_string(),
                    amount,
                    period: urpwallet::db::models::BudgetPeriod::Monthly,
                    category_id: cat,
                    start_date,
                    end_date,
                });
            }

            ui.set_show_add_budget_modal(false);
            refresh_ui(&ui, &db_clone);
        });
    }

    // P. Delete Budget
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_delete_budget(move |id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            if db_clone.delete_budget(id.as_str()).is_ok() {
                refresh_ui(&ui, &db_clone);
            }
        });
    }
}
