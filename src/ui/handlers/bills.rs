use chrono::Utc;
use slint::ComponentHandle;
use urpwallet::db::models::{BillFrequency, CreateBillInput, TransactionType, UpdateBillInput};
use urpwallet::db::Database;

use crate::MainWindow;
use crate::ui::refresh::refresh_ui;

/// Registers all bill-related event handlers on the main window.
pub fn register_bill_handlers(ui: &MainWindow, db: &Database) {
    // L. Open Edit Bill
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_open_edit_bill(move |bill_id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            if let Ok(b) = db_clone.get_bill(bill_id.as_str()) {
                ui.set_modal_bill_is_editing(true);
                ui.set_modal_bill_id(b.id.into());
                ui.set_modal_bill_name(b.name.into());
                ui.set_modal_bill_amount(format!("{:.2}", b.amount).into());
                ui.set_modal_bill_payee(b.payee.unwrap_or_default().into());
                ui.set_modal_bill_next_date(b.next_date.into());
                ui.set_show_bill_detail_modal(false);
                ui.set_show_add_bill_modal(true);
            }
        });
    }

    // Submit Save Bill
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_submit_save_bill(
            move |is_editing, bill_id, name, amount_str, account_id, category_id, payee, freq_int, next_date| {
                let Some(ui) = ui_weak.upgrade() else { return; };
                let amount: f64 = match amount_str.trim().parse() {
                    Ok(v) if v > 0.0 => v,
                    _ => return,
                };

                let acc_id = if account_id.is_empty() {
                    db_clone
                        .list_accounts(false)
                        .ok()
                        .and_then(|a| a.first().map(|acc| acc.id.clone()))
                        .unwrap_or_default()
                } else {
                    account_id.to_string()
                };

                let frequency = match freq_int {
                    0 => BillFrequency::Daily,
                    1 => BillFrequency::Weekly,
                    2 => BillFrequency::Monthly,
                    3 => BillFrequency::Yearly,
                    _ => BillFrequency::Monthly,
                };

                let date_str = if next_date.trim().is_empty() {
                    Utc::now().format("%Y-%m-%d").to_string()
                } else {
                    next_date.trim().to_string()
                };

                if is_editing {
                    let _ = db_clone.update_bill(
                        bill_id.as_str(),
                        UpdateBillInput {
                            name: Some(name.trim().to_string()),
                            amount: Some(amount),
                            bill_type: None,
                            account_id: Some(acc_id),
                            category_id: Some(if category_id.is_empty() { None } else { Some(category_id.to_string()) }),
                            payee: Some(if payee.trim().is_empty() { None } else { Some(payee.trim().to_string()) }),
                            frequency: Some(frequency),
                            next_date: Some(date_str),
                            auto_record: None,
                            is_active: None,
                        },
                    );
                } else {
                    let _ = db_clone.create_bill(CreateBillInput {
                        id: None,
                        name: name.trim().to_string(),
                        amount,
                        bill_type: Some(TransactionType::Expense),
                        account_id: acc_id,
                        category_id: if category_id.is_empty() { None } else { Some(category_id.to_string()) },
                        payee: if payee.trim().is_empty() { None } else { Some(payee.trim().to_string()) },
                        frequency: Some(frequency),
                        next_date: date_str,
                        auto_record: Some(false),
                        is_active: Some(true),
                    });
                }

                ui.set_show_add_bill_modal(false);
                refresh_ui(&ui, &db_clone);
            },
        );
    }

    // O. 1-Click Recurring Bill Payment Execution
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_execute_bill_payment(move |bill_id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            if db_clone.execute_bill_payment(bill_id.as_str()).is_ok() {
                refresh_ui(&ui, &db_clone);
            }
        });
    }

    // P. Delete Bill
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_delete_bill(move |id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            if db_clone.delete_bill(id.as_str()).is_ok() {
                refresh_ui(&ui, &db_clone);
            }
        });
    }
}
