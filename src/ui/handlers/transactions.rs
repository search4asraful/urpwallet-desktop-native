use chrono::Utc;
use slint::ComponentHandle;

use urpwallet::db::models::{
    CategoryType, CreateTransactionInput, PaymentType, TransactionType,
    UpdateTransactionInput,
};
use urpwallet::db::Database;
use urpwallet::logic::format_currency;

use crate::{MainWindow, TransactionItem};
use crate::ui::refresh::{refresh_categories, refresh_transactions, refresh_ui};

/// Registers all transaction-related event handlers on the main window.
pub fn register_transaction_handlers(ui: &MainWindow, db: &Database) {
    // F. View Transaction Details Modal
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_open_tx_details(move |tx_id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let privacy = ui.get_privacy_mode();

            if let Ok(item) = db_clone.get_transaction(tx_id.as_str()) {
                let tx = item.transaction;
                let is_income = tx.tx_type == TransactionType::Income;
                let is_transfer = tx.tx_type == TransactionType::Transfer;
                let prefix = if is_income { "+" } else if is_transfer { "⇄ " } else { "-" };
                let acc_name = item.account_name.unwrap_or_default();
                let to_acc_name = item.to_account_name.unwrap_or_default();
                let cat_name = item.category_name.unwrap_or_else(|| "Expense".into());

                let title = tx.payee.clone().unwrap_or_else(|| {
                    if is_transfer {
                        format!("{acc_name} → {to_acc_name}")
                    } else {
                        cat_name.clone()
                    }
                });

                ui.set_detail_tx(TransactionItem {
                    id: tx.id.clone().into(),
                    date: tx.date.clone().into(),
                    time: tx.time.clone().unwrap_or_default().into(),
                    title: title.into(),
                    subtitle: acc_name.clone().into(),
                    amount: format!("{}{}", prefix, format_currency(tx.amount, &tx.currency, privacy)).into(),
                    raw_amount: tx.amount.to_string().into(),
                    is_income,
                    is_transfer,
                    color: item.category_color.clone().unwrap_or_else(|| "#64748B".into()).into(),
                    account_id: tx.account_id.clone().into(),
                    account_name: acc_name.into(),
                    to_account_id: tx.to_account_id.unwrap_or_default().into(),
                    to_account_name: to_acc_name.into(),
                    category_id: tx.category_id.unwrap_or_default().into(),
                    category_name: cat_name.into(),
                    payee: tx.payee.unwrap_or_default().into(),
                    note: tx.note.unwrap_or_default().into(),
                    payment_type: tx.payment_type.as_str().into(),
                    status: tx.status.as_str().into(),
                });

                ui.set_show_tx_detail_modal(true);
            }
        });
    }

    // G. Open Add Transaction Modal
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_open_add_tx(move |tx_type| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let today = Utc::now().format("%Y-%m-%d").to_string();
            ui.set_modal_tx_is_editing(false);
            ui.set_modal_tx_id("".into());
            ui.set_modal_tx_type(tx_type);
            ui.set_modal_amount("".into());
            ui.set_modal_payee("".into());
            ui.set_modal_note("".into());
            ui.set_modal_date(today.into());
            ui.set_modal_error("".into());

            if let Ok(accs) = db_clone.list_accounts(false) {
                if let Some(first) = accs.first() {
                    ui.set_modal_account_id(first.id.clone().into());
                    ui.set_modal_account_name(first.name.clone().into());
                }
                if accs.len() > 1 {
                    ui.set_modal_to_account_id(accs[1].id.clone().into());
                    ui.set_modal_to_account_name(accs[1].name.clone().into());
                }
            }

            ui.set_modal_cat_show_all(false);
            let cat_filter = match tx_type {
                0 => Some(CategoryType::Expense),
                1 => Some(CategoryType::Income),
                _ => None,
            };
            if let Ok(cats) = db_clone.list_categories(cat_filter)
                && let Some(first_cat) = cats.first()
            {
                ui.set_modal_category_id(first_cat.id.clone().into());
                ui.set_modal_category_name(first_cat.name.clone().into());
            }

            refresh_categories(&ui, &db_clone);
            ui.set_show_add_tx_modal(true);
        });
    }

    // Modal Transaction Type Changed
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_modal_tx_type_changed(move |new_type| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            ui.set_modal_tx_type(new_type);
            let cat_type = match new_type {
                0 => Some(CategoryType::Expense),
                1 => Some(CategoryType::Income),
                _ => None,
            };
            if let Some(target_type) = cat_type
                && let Ok(cats) = db_clone.list_categories(Some(target_type))
                && let Some(first) = cats.first()
            {
                ui.set_modal_category_id(first.id.clone().into());
                ui.set_modal_category_name(first.name.clone().into());
            }
            refresh_categories(&ui, &db_clone);
        });
    }

    // Toggle Modal Category Filter
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_toggle_modal_cat_filter(move || {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let current = ui.get_modal_cat_show_all();
            ui.set_modal_cat_show_all(!current);
            refresh_categories(&ui, &db_clone);
        });
    }

    // H. Open Edit Transaction Modal
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_open_edit_tx(move |tx_id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            if let Ok(item) = db_clone.get_transaction(tx_id.as_str()) {
                let tx = item.transaction;
                ui.set_modal_tx_is_editing(true);
                ui.set_modal_tx_id(tx.id.clone().into());
                ui.set_modal_tx_type(match tx.tx_type {
                    TransactionType::Expense => 0,
                    TransactionType::Income => 1,
                    TransactionType::Transfer => 2,
                });
                ui.set_modal_amount(format!("{:.2}", tx.amount).into());
                ui.set_modal_account_id(tx.account_id.clone().into());
                ui.set_modal_account_name(item.account_name.unwrap_or_default().into());
                ui.set_modal_to_account_id(tx.to_account_id.clone().unwrap_or_default().into());
                ui.set_modal_to_account_name(item.to_account_name.unwrap_or_default().into());
                ui.set_modal_category_id(tx.category_id.clone().unwrap_or_default().into());
                ui.set_modal_category_name(item.category_name.unwrap_or_default().into());
                ui.set_modal_payee(tx.payee.unwrap_or_default().into());
                ui.set_modal_note(tx.note.unwrap_or_default().into());
                ui.set_modal_date(tx.date.into());
                ui.set_modal_error("".into());
                ui.set_modal_cat_show_all(false);

                refresh_categories(&ui, &db_clone);
                ui.set_show_tx_detail_modal(false);
                ui.set_show_add_tx_modal(true);
            }
        });
    }

    // I. Submit Save Transaction (Create or Edit)
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_submit_save_tx(
            move |is_editing, tx_id, tx_type_int, amount_str, account_id, to_account_id, category_id, payee, note, date| {
                let Some(ui) = ui_weak.upgrade() else { return; };

                let amount: f64 = match amount_str.trim().parse() {
                    Ok(val) if val > 0.0 => val,
                    _ => {
                        ui.set_modal_error("Please enter a valid amount greater than 0.00".into());
                        return;
                    }
                };

                let tx_type = match tx_type_int {
                    0 => TransactionType::Expense,
                    1 => TransactionType::Income,
                    2 => TransactionType::Transfer,
                    _ => TransactionType::Expense,
                };

                let acc_id = if account_id.is_empty() {
                    match db_clone.list_accounts(false) {
                        Ok(accs) if !accs.is_empty() => accs[0].id.clone(),
                        _ => {
                            ui.set_modal_error("No account found to assign transaction".into());
                            return;
                        }
                    }
                } else {
                    account_id.to_string()
                };

                let to_acc = if tx_type == TransactionType::Transfer {
                    if to_account_id.is_empty() {
                        match db_clone.list_accounts(false) {
                            Ok(accs) if accs.len() > 1 => Some(accs[1].id.clone()),
                            _ => None,
                        }
                    } else {
                        Some(to_account_id.to_string())
                    }
                } else {
                    None
                };

                let cat = if category_id.is_empty() { None } else { Some(category_id.to_string()) };
                let payee_opt = if payee.trim().is_empty() { None } else { Some(payee.trim().to_string()) };
                let note_opt = if note.trim().is_empty() { None } else { Some(note.trim().to_string()) };
                let date_str = if date.trim().is_empty() {
                    Utc::now().format("%Y-%m-%d").to_string()
                } else {
                    date.trim().to_string()
                };

                let result = if is_editing {
                    db_clone.update_transaction(
                        tx_id.as_str(),
                        UpdateTransactionInput {
                            account_id: Some(acc_id),
                            to_account_id: Some(to_acc),
                            category_id: Some(cat),
                            tx_type: Some(tx_type),
                            amount: Some(amount),
                            currency: None,
                            date: Some(date_str),
                            time: None,
                            payee: Some(payee_opt),
                            note: Some(note_opt),
                            status: None,
                            payment_type: None,
                        },
                    )
                } else {
                    db_clone.create_transaction(CreateTransactionInput {
                        id: None,
                        account_id: acc_id,
                        to_account_id: to_acc,
                        category_id: cat,
                        tx_type,
                        amount,
                        currency: None,
                        date: Some(date_str),
                        time: Some(Utc::now().format("%H:%M").to_string()),
                        payee: payee_opt,
                        note: note_opt,
                        status: None,
                        payment_type: Some(PaymentType::Cash),
                    })
                };

                match result {
                    Ok(_) => {
                        ui.set_show_add_tx_modal(false);
                        refresh_ui(&ui, &db_clone);
                    }
                    Err(e) => {
                        ui.set_modal_error(e.user_friendly_message().into());
                    }
                }
            },
        );
    }

    // Q. Search Transactions
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_search_transactions(move |query, filter_type| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            ui.set_tx_search_query(query.clone());
            ui.set_tx_filter_type(filter_type);
            refresh_transactions(&ui, &db_clone);
        });
    }

    // P. Delete Transaction
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_delete_transaction(move |id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            if db_clone.delete_transaction(id.as_str()).is_ok() {
                refresh_ui(&ui, &db_clone);
            }
        });
    }
}
