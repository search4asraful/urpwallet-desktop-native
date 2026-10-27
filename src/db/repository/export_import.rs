use rusqlite::{params, Connection};
use chrono::Utc;
use crate::db::error::{DbError, DbResult};
use crate::db::models::{
    BillSubscription, FullBackupData, TransactionFilter,
};
use crate::db::repository::{
    accounts::list_accounts,
    bills::list_bills,
    budgets::list_budgets,
    categories::list_categories,
    debts::list_debts,
    settings::get_all_settings,
    transactions::{list_transactions, map_transaction_row},
};

pub fn export_full_backup(conn: &Connection) -> DbResult<FullBackupData> {
    let accounts = list_accounts(conn, true)?;
    let categories = list_categories(conn, None)?;

    // Fetch all transactions raw
    let mut stmt = conn.prepare(
        "SELECT id, account_id, to_account_id, category_id, type, amount, currency,
                date, time, payee, note, status, payment_type, created_at
         FROM transactions ORDER BY date DESC, time DESC, created_at DESC",
    )?;
    let tx_rows = stmt.query_map([], map_transaction_row)?;
    let mut transactions = Vec::new();
    for r in tx_rows {
        transactions.push(r?);
    }

    let budgets = list_budgets(conn)?;
    let bills_with_details = list_bills(conn, false)?;
    let bills_subscriptions: Vec<BillSubscription> = bills_with_details.into_iter().map(|b| b.bill).collect();
    let debts = list_debts(conn, false)?;
    let settings = get_all_settings(conn)?;

    Ok(FullBackupData {
        exported_at: Utc::now().to_rfc3339(),
        accounts,
        categories,
        transactions,
        budgets,
        bills_subscriptions,
        debts,
        settings,
    })
}

pub fn export_transactions_csv(conn: &Connection) -> DbResult<String> {
    let filter = TransactionFilter {
        limit: Some(1_000_000), // All
        ..Default::default()
    };
    let transactions = list_transactions(conn, &filter)?;

    let mut wtr = csv::Writer::from_writer(vec![]);

    // Write header
    wtr.write_record([
        "id",
        "date",
        "time",
        "type",
        "amount",
        "currency",
        "account",
        "to_account",
        "category",
        "payee",
        "note",
        "payment_type",
        "status",
    ])?;

    for item in transactions {
        let tx = item.transaction;
        wtr.write_record([
            &tx.id,
            &tx.date,
            tx.time.as_deref().unwrap_or(""),
            tx.tx_type.as_str(),
            &tx.amount.to_string(),
            &tx.currency,
            item.account_name.as_deref().unwrap_or(&tx.account_id),
            item.to_account_name.as_deref().unwrap_or(""),
            item.category_name.as_deref().unwrap_or(""),
            tx.payee.as_deref().unwrap_or(""),
            tx.note.as_deref().unwrap_or(""),
            tx.payment_type.as_str(),
            tx.status.as_str(),
        ])?;
    }

    let data = wtr.into_inner().map_err(|e| DbError::Validation(e.to_string()))?;
    String::from_utf8(data).map_err(|e| DbError::Validation(e.to_string()))
}

pub fn import_full_backup(conn: &mut Connection, backup: &FullBackupData) -> DbResult<()> {
    let tx = conn.transaction()?;

    // Clear existing data cleanly in correct foreign key order
    tx.execute_batch(
        "DELETE FROM transactions;
         DELETE FROM budgets;
         DELETE FROM bills_subscriptions;
         DELETE FROM debts;
         DELETE FROM accounts;
         DELETE FROM categories;",
    )?;

    // 1. Categories
    for c in &backup.categories {
        tx.execute(
            "INSERT INTO categories (id, name, type, icon, color, parent_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![c.id, c.name, c.category_type.as_str(), c.icon, c.color, c.parent_id, c.created_at],
        )?;
    }

    // 2. Accounts
    for a in &backup.accounts {
        tx.execute(
            "INSERT INTO accounts (id, name, type, currency, balance, color, icon, is_archived, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                a.id,
                a.name,
                a.account_type.as_str(),
                a.currency,
                a.balance,
                a.color,
                a.icon,
                if a.is_archived { 1 } else { 0 },
                a.created_at,
                a.updated_at
            ],
        )?;
    }

    // 3. Transactions
    for t in &backup.transactions {
        tx.execute(
            "INSERT INTO transactions (
                id, account_id, to_account_id, category_id, type, amount, currency,
                date, time, payee, note, status, payment_type, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                t.id,
                t.account_id,
                t.to_account_id,
                t.category_id,
                t.tx_type.as_str(),
                t.amount,
                t.currency,
                t.date,
                t.time,
                t.payee,
                t.note,
                t.status.as_str(),
                t.payment_type.as_str(),
                t.created_at
            ],
        )?;
    }

    // 4. Budgets
    for b in &backup.budgets {
        tx.execute(
            "INSERT INTO budgets (id, name, amount, period, category_id, start_date, end_date, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                b.id,
                b.name,
                b.amount,
                b.period.as_str(),
                b.category_id,
                b.start_date,
                b.end_date,
                b.created_at
            ],
        )?;
    }

    // 5. Bills / Subscriptions
    for b in &backup.bills_subscriptions {
        tx.execute(
            "INSERT INTO bills_subscriptions (
                id, name, amount, type, account_id, category_id, payee, frequency, next_date, auto_record, is_active, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                b.id,
                b.name,
                b.amount,
                b.bill_type.as_str(),
                b.account_id,
                b.category_id,
                b.payee,
                b.frequency.as_str(),
                b.next_date,
                if b.auto_record { 1 } else { 0 },
                if b.is_active { 1 } else { 0 },
                b.created_at
            ],
        )?;
    }

    // 6. Debts
    for d in &backup.debts {
        tx.execute(
            "INSERT INTO debts (id, type, person_name, amount, settled_amount, due_date, notes, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                d.id,
                d.debt_type.as_str(),
                d.person_name,
                d.amount,
                d.settled_amount,
                d.due_date,
                d.notes,
                d.status.as_str(),
                d.created_at
            ],
        )?;
    }

    // 7. Settings
    for (k, v) in &backup.settings {
        tx.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![k, v],
        )?;
    }

    tx.commit()?;
    Ok(())
}
