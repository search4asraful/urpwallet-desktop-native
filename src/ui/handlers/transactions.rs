use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use chrono::{Datelike, Local, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use slint::{ComponentHandle, Model, ModelRc, VecModel};

use urpwallet::db::models::{
    CategoryType, CreateTransactionInput, PaymentType, TransactionType,
    UpdateTransactionInput,
};
use urpwallet::db::Database;
use urpwallet::logic::{format_date, parse_date_input};

use crate::MainWindow;
use crate::ui::mappers::map_tx_item;
use crate::ui::refresh::{refresh_categories, refresh_transactions, refresh_ui};
use crate::ui::toast::{show_error, show_success};

fn sync_tx_selection_ui(ui: &MainWindow, selected_set: &HashSet<String>) {
    let model = ui.get_all_transactions();
    let count = model.row_count();
    let mut updated = Vec::with_capacity(count);
    let mut selected_count = 0;
    for i in 0..count {
        if let Some(mut tx) = model.row_data(i) {
            let is_sel = selected_set.contains(tx.id.as_str());
            if is_sel {
                selected_count += 1;
            }
            tx.selected = is_sel;
            updated.push(tx);
        }
    }
    ui.set_all_transactions(ModelRc::from(Rc::new(VecModel::from(updated))));
    ui.set_selected_tx_count(selected_count);
}

fn parse_time_flexible(s: &str) -> Option<NaiveTime> {
    let trimmed = s.trim();
    if let Ok(t) = NaiveTime::parse_from_str(trimmed, "%H:%M") {
        return Some(t);
    }
    if let Ok(t) = NaiveTime::parse_from_str(trimmed, "%H:%M:%S") {
        return Some(t);
    }
    let upper = trimmed.to_uppercase();
    if let Ok(t) = NaiveTime::parse_from_str(&upper, "%I:%M %p") {
        return Some(t);
    }
    if let Ok(t) = NaiveTime::parse_from_str(&upper, "%I:%M%p") {
        return Some(t);
    }
    if let Ok(t) = NaiveTime::parse_from_str(&upper, "%l:%M %p") {
        return Some(t);
    }
    if let Ok(t) = NaiveTime::parse_from_str(&upper, "%l:%M%p") {
        return Some(t);
    }
    None
}

/// Registers all transaction-related event handlers on the main window.
pub fn register_transaction_handlers(ui: &MainWindow, db: &Database) {
    let selected_ids = Rc::new(RefCell::new(HashSet::<String>::new()));
    // F. View Transaction Details Modal
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        ui.on_open_tx_details(move |tx_id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let privacy = ui.get_privacy_mode();

            if let Ok(item) = db_clone.get_transaction(tx_id.as_str()) {
                let date_fmt = db_clone
                    .get_setting("date_format")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "YYYY-MM-DD".to_string());

                ui.set_detail_tx(map_tx_item(&item, privacy, &date_fmt));
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
            let date_fmt = db_clone
                .get_setting("date_format")
                .ok()
                .flatten()
                .unwrap_or_else(|| "YYYY-MM-DD".to_string());
            let now = Local::now();
            let today_iso = now.format("%Y-%m-%d").to_string();
            let formatted_date = format_date(&today_iso, &date_fmt);
            let current_time = now.format("%H:%M").to_string();
            ui.set_modal_tx_is_editing(false);
            ui.set_modal_tx_id("".into());
            ui.set_modal_tx_type(tx_type);
            ui.set_modal_amount("".into());
            ui.set_modal_payee("".into());
            ui.set_modal_note("".into());
            ui.set_modal_date(formatted_date.into());
            ui.set_modal_time(current_time.into());
            ui.set_modal_date_year(now.year());
            ui.set_modal_date_month(now.month() as i32);
            ui.set_modal_date_day(now.day() as i32);
            ui.set_modal_time_hour(now.hour() as i32);
            ui.set_modal_time_minute(now.minute() as i32);
            ui.set_today_year(now.year());
            ui.set_today_month(now.month() as i32);
            ui.set_today_day(now.day() as i32);
            ui.set_today_hour(now.hour() as i32);
            ui.set_today_minute(now.minute() as i32);
            ui.set_modal_error("".into());

            let base_currency = db_clone
                .get_setting("base_currency")
                .ok()
                .flatten()
                .unwrap_or_else(|| "BDT".to_string());
            let mut curr_sym = urpwallet::logic::get_currency_symbol(&base_currency);

            if let Ok(accs) = db_clone.list_accounts(false) {
                if let Some(first) = accs.first() {
                    ui.set_modal_account_id(first.id.clone().into());
                    ui.set_modal_account_name(first.name.clone().into());
                    curr_sym = urpwallet::logic::get_currency_symbol(&first.currency);
                }
                if accs.len() > 1 {
                    ui.set_modal_to_account_id(accs[1].id.clone().into());
                    ui.set_modal_to_account_name(accs[1].name.clone().into());
                }
            }
            ui.set_modal_currency_symbol(curr_sym.into());

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
                let now = Local::now();
                ui.set_today_year(now.year());
                ui.set_today_month(now.month() as i32);
                ui.set_today_day(now.day() as i32);
                ui.set_today_hour(now.hour() as i32);
                ui.set_today_minute(now.minute() as i32);

                let date_fmt = db_clone
                    .get_setting("date_format")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "YYYY-MM-DD".to_string());
                let (date_year, date_month, date_day) = match NaiveDate::parse_from_str(&tx.date, "%Y-%m-%d") {
                    Ok(d) => (d.year(), d.month() as i32, d.day() as i32),
                    Err(_) => (now.year(), now.month() as i32, now.day() as i32),
                };

                let time_str = tx.time.clone().unwrap_or_else(|| now.format("%H:%M").to_string());
                let (time_hour, time_minute) = match parse_time_flexible(&time_str) {
                    Some(t) => (t.hour() as i32, t.minute() as i32),
                    None => (now.hour() as i32, now.minute() as i32),
                };

                let formatted_date = format_date(&tx.date, &date_fmt);
                let curr_sym = urpwallet::logic::get_currency_symbol(&tx.currency);
                ui.set_modal_currency_symbol(curr_sym.into());
                ui.set_modal_payee(tx.payee.unwrap_or_default().into());
                ui.set_modal_note(tx.note.unwrap_or_default().into());
                ui.set_modal_date(formatted_date.into());
                ui.set_modal_time(time_str.into());
                ui.set_modal_date_year(date_year);
                ui.set_modal_date_month(date_month);
                ui.set_modal_date_day(date_day);
                ui.set_modal_time_hour(time_hour);
                ui.set_modal_time_minute(time_minute);
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
            move |is_editing, tx_id, tx_type_int, amount_str, account_id, to_account_id, category_id, payee, note, date, time| {
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

                let date_fmt = db_clone
                    .get_setting("date_format")
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "YYYY-MM-DD".to_string());

                let now = Local::now();
                let date_str = if date.trim().is_empty() {
                    now.format("%Y-%m-%d").to_string()
                } else {
                    date.trim().to_string()
                };

                let parsed_date = match parse_date_input(&date_str, &date_fmt) {
                    Ok(d) => d,
                    Err(_) => {
                        let example = format_date("2026-10-29", &date_fmt);
                        ui.set_modal_error(format!("Please enter a valid date in {date_fmt} format (e.g. {example})").into());
                        return;
                    }
                };

                let time_str = if time.trim().is_empty() {
                    now.format("%H:%M").to_string()
                } else {
                    time.trim().to_string()
                };

                let parsed_time = match parse_time_flexible(&time_str) {
                    Some(t) => t,
                    None => {
                        ui.set_modal_error("Please enter a valid time (e.g. 14:30 or 02:30 PM)".into());
                        return;
                    }
                };

                let _unified_dt: NaiveDateTime = NaiveDateTime::new(parsed_date, parsed_time);
                let formatted_date = parsed_date.format("%Y-%m-%d").to_string();
                let formatted_time = parsed_time.format("%H:%M").to_string();

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
                            date: Some(formatted_date),
                            time: Some(Some(formatted_time)),
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
                        date: Some(formatted_date),
                        time: Some(formatted_time),
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
                        show_success(&ui, if is_editing { "Transaction updated successfully" } else { "Transaction recorded successfully" });
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
        let selected_ids_clone = selected_ids.clone();
        ui.on_search_transactions(move |query, filter_type| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            ui.set_tx_search_query(query.clone());
            ui.set_tx_filter_type(filter_type);
            selected_ids_clone.borrow_mut().clear();
            ui.set_selected_tx_count(0);
            refresh_transactions(&ui, &db_clone);
        });
    }

    // P. Delete Single Transaction
    {
        let ui_weak = ui.as_weak();
        let db_clone = db.clone();
        let selected_ids_clone = selected_ids.clone();
        ui.on_delete_transaction(move |id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let id_str = id.to_string();
            match db_clone.delete_transaction(id_str.as_str()) {
                Ok(_) => {
                    selected_ids_clone.borrow_mut().remove(&id_str);
                    ui.set_selected_tx_count(selected_ids_clone.borrow().len() as i32);
                    refresh_ui(&ui, &db_clone);
                    show_success(&ui, "Transaction deleted successfully");
                }
                Err(e) => {
                    show_error(&ui, format!("Failed to delete transaction: {}", e.user_friendly_message()));
                }
            }
        });
    }

    // R. Toggle Transaction Selection
    {
        let ui_weak = ui.as_weak();
        let selected_ids_clone = selected_ids.clone();
        ui.on_toggle_tx_selection(move |id| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let id_str = id.to_string();
            {
                let mut set = selected_ids_clone.borrow_mut();
                if set.contains(&id_str) {
                    set.remove(&id_str);
                } else {
                    set.insert(id_str);
                }
            }
            sync_tx_selection_ui(&ui, &selected_ids_clone.borrow());
        });
    }

    // S. Select All Visible Transactions
    {
        let ui_weak = ui.as_weak();
        let selected_ids_clone = selected_ids.clone();
        ui.on_select_all_txs(move |select_all| {
            let Some(ui) = ui_weak.upgrade() else { return; };
            {
                let mut set = selected_ids_clone.borrow_mut();
                set.clear();
                if select_all {
                    let model = ui.get_all_transactions();
                    for i in 0..model.row_count() {
                        if let Some(tx) = model.row_data(i) {
                            set.insert(tx.id.to_string());
                        }
                    }
                }
            }
            sync_tx_selection_ui(&ui, &selected_ids_clone.borrow());
        });
    }

    // T. Clear Selection
    {
        let ui_weak = ui.as_weak();
        let selected_ids_clone = selected_ids.clone();
        ui.on_clear_tx_selection(move || {
            let Some(ui) = ui_weak.upgrade() else { return; };
            selected_ids_clone.borrow_mut().clear();
            sync_tx_selection_ui(&ui, &selected_ids_clone.borrow());
        });
    }

    // U. Bulk Delete Selected Transactions
    {
        let ui_weak = ui.as_weak();
        let selected_ids_clone = selected_ids.clone();
        let db_clone = db.clone();
        ui.on_bulk_delete_selected(move || {
            let Some(ui) = ui_weak.upgrade() else { return; };
            let ids: Vec<String> = selected_ids_clone.borrow().iter().cloned().collect();
            if ids.is_empty() { return; }

            if let Ok(count) = db_clone.delete_transactions_bulk(&ids) {
                selected_ids_clone.borrow_mut().clear();
                ui.set_selected_tx_count(0);
                show_success(&ui, format!("Successfully deleted {count} transactions"));
                refresh_ui(&ui, &db_clone);
            }
        });
    }
}
