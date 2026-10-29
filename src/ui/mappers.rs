use crate::TransactionItem;
use urpwallet::db::models::{TransactionType, TransactionWithDetails};
use urpwallet::logic::{format_currency, format_date};

/// Maps a `TransactionWithDetails` row into the Slint `TransactionItem` struct.
pub fn map_tx_item(item: &TransactionWithDetails, privacy: bool, date_format: &str) -> TransactionItem {
    let tx = &item.transaction;
    let is_income = tx.tx_type == TransactionType::Income;
    let is_transfer = tx.tx_type == TransactionType::Transfer;
    let prefix = if is_income { "+" } else if is_transfer { "↔ " } else { "-" };

    let acc_name = item.account_name.as_deref().unwrap_or("");
    let to_acc_name = item.to_account_name.as_deref().unwrap_or("");
    let cat_name = item.category_name.as_deref().unwrap_or("Expense");

    let title = match tx.payee.as_deref() {
        Some(p) if !p.is_empty() => p.into(),
        _ if is_transfer => format!("{acc_name} → {to_acc_name}").into(),
        _ => cat_name.into(),
    };

    TransactionItem {
        id: tx.id.as_str().into(),
        date: format_date(tx.date.as_str(), date_format).into(),
        time: tx.time.as_deref().unwrap_or("").into(),
        title,
        subtitle: acc_name.into(),
        amount: format!("{}{}", prefix, format_currency(tx.amount, &tx.currency, privacy)).into(),
        raw_amount: tx.amount.to_string().into(),
        is_income,
        is_transfer,
        color: item.category_color.as_deref().unwrap_or("#64748B").into(),
        account_id: tx.account_id.as_str().into(),
        account_name: acc_name.into(),
        to_account_id: tx.to_account_id.as_deref().unwrap_or("").into(),
        to_account_name: to_acc_name.into(),
        category_id: tx.category_id.as_deref().unwrap_or("").into(),
        category_name: cat_name.into(),
        payee: tx.payee.as_deref().unwrap_or("").into(),
        note: tx.note.as_deref().unwrap_or("").into(),
        payment_type: tx.payment_type.as_str().into(),
        status: tx.status.as_str().into(),
        selected: false,
    }
}
