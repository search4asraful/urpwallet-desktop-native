use chrono::Utc;
use slint::{ComponentHandle, Model, ModelRc, VecModel};
use urpwallet::db::models::{CategoryType, CreateBudgetInput, UpdateBudgetInput};
use urpwallet::db::Database;

use crate::{MainWindow, BudgetCategoryOption};
use crate::ui::refresh::refresh_ui;
use crate::ui::toast::{show_error, show_success};

/// Registers all budget-related event handlers on the main window.
pub fn register_budget_handlers(ui: &MainWindow, db: &Database) {
    // 1. Open New Budget
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_open_new_budget(move || {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let categories = db_clone.list_categories(Some(CategoryType::Expense)).unwrap_or_default();
            let expense_cats: Vec<BudgetCategoryOption> = categories
                .into_iter()
                .map(|c| BudgetCategoryOption {
                    id: c.id.into(),
                    name: c.name.into(),
                    color: c.color.into(),
                    selected: false,
                })
                .collect();

            ui.set_modal_budget_categories(ModelRc::new(VecModel::from(expense_cats)));
            ui.set_modal_budget_selected_count(0);
            ui.set_modal_budget_is_editing(false);
            ui.set_modal_budget_id("".into());
            ui.set_modal_budget_name("".into());
            ui.set_modal_budget_amount("".into());
            ui.set_show_add_budget_modal(true);
        });
    }

    // 2. Open Edit Budget
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_open_edit_budget(move |b_id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            if let Ok(b) = db_clone.get_budget(b_id.as_str()) {
                let categories = db_clone.list_categories(Some(CategoryType::Expense)).unwrap_or_default();
                let mut selected_count = 0;
                let expense_cats: Vec<BudgetCategoryOption> = categories
                    .into_iter()
                    .map(|c| {
                        let is_selected = b.category_ids.contains(&c.id)
                            || (b.category_ids.is_empty() && b.category_id.as_deref() == Some(&c.id));
                        if is_selected {
                            selected_count += 1;
                        }
                        BudgetCategoryOption {
                            id: c.id.into(),
                            name: c.name.into(),
                            color: c.color.into(),
                            selected: is_selected,
                        }
                    })
                    .collect();

                ui.set_modal_budget_categories(ModelRc::new(VecModel::from(expense_cats)));
                ui.set_modal_budget_selected_count(selected_count);
                ui.set_modal_budget_is_editing(true);
                ui.set_modal_budget_id(b.id.into());
                ui.set_modal_budget_name(b.name.into());
                ui.set_modal_budget_amount(format!("{:.2}", b.amount).into());
                ui.set_show_budget_detail_modal(false);
                ui.set_show_add_budget_modal(true);
            }
        });
    }

    // 3. Toggle Category Selection in Budget Modal
    {
        let ui_weak = ui.as_weak();
        ui.on_toggle_modal_budget_category(move |cat_id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let model = ui.get_modal_budget_categories();
            let mut updated = Vec::new();
            let mut selected_count = 0;
            for i in 0..model.row_count() {
                if let Some(mut opt) = model.row_data(i) {
                    if opt.id == cat_id {
                        opt.selected = !opt.selected;
                    }
                    if opt.selected {
                        selected_count += 1;
                    }
                    updated.push(opt);
                }
            }
            ui.set_modal_budget_categories(ModelRc::new(VecModel::from(updated)));
            ui.set_modal_budget_selected_count(selected_count);
        });
    }

    // 4. Select All / Clear Categories in Budget Modal
    {
        let ui_weak = ui.as_weak();
        ui.on_select_all_budget_categories(move |select_all| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let model = ui.get_modal_budget_categories();
            let mut updated = Vec::new();
            let mut selected_count = 0;
            for i in 0..model.row_count() {
                if let Some(mut opt) = model.row_data(i) {
                    opt.selected = select_all;
                    if select_all {
                        selected_count += 1;
                    }
                    updated.push(opt);
                }
            }
            ui.set_modal_budget_categories(ModelRc::new(VecModel::from(updated)));
            ui.set_modal_budget_selected_count(selected_count);
        });
    }

    // 5. Submit Save Budget (Create or Edit with Chosen Categories)
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_submit_save_budget(move |is_editing, b_id, name, amount_str| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let amount: f64 = match amount_str.trim().parse() {
                Ok(v) if v > 0.0 => v,
                _ => return,
            };

            let model = ui.get_modal_budget_categories();
            let mut selected_ids = Vec::new();
            for i in 0..model.row_count() {
                if let Some(opt) = model.row_data(i) {
                    if opt.selected {
                        selected_ids.push(opt.id.to_string());
                    }
                }
            }

            let year_month = Utc::now().format("%Y-%m").to_string();
            let start_date = format!("{}-01", year_month);
            let end_date = format!("{}-31", year_month);

            let res = if is_editing {
                db_clone.update_budget(
                    b_id.as_str(),
                    UpdateBudgetInput {
                        name: Some(name.trim().to_string()),
                        amount: Some(amount),
                        period: None,
                        category_id: None,
                        category_ids: Some(selected_ids),
                        start_date: None,
                        end_date: None,
                    },
                )
            } else {
                db_clone.create_budget(CreateBudgetInput {
                    id: None,
                    name: name.trim().to_string(),
                    amount,
                    period: urpwallet::db::models::BudgetPeriod::Monthly,
                    category_id: None,
                    category_ids: selected_ids,
                    start_date,
                    end_date,
                })
            };

            ui.set_show_add_budget_modal(false);
            match res {
                Ok(_) => {
                    refresh_ui(&ui, &db_clone);
                    show_success(&ui, if is_editing { "Monthly budget updated successfully" } else { "Monthly budget created successfully" });
                }
                Err(e) => {
                    show_error(&ui, format!("Failed to save budget: {}", e.user_friendly_message()));
                }
            }
        });
    }

    // 6. Delete Budget
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_delete_budget(move |id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            match db_clone.delete_budget(id.as_str()) {
                Ok(_) => {
                    refresh_ui(&ui, &db_clone);
                    show_success(&ui, "Monthly budget deleted successfully");
                }
                Err(e) => {
                    show_error(&ui, format!("Failed to delete budget: {}", e.user_friendly_message()));
                }
            }
        });
    }
}
