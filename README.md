# urpwallet

A local-first, lightweight native personal finance tracker and budget planner built with **Rust** and **Slint**.

---

## Highlights

- **Local-First & Private**: Data stays on your machine in an SQLite database (WAL mode). Zero third-party telemetry.
- **PIN Lock & Privacy Mode**: Argon2-hashed master PIN overlay and one-click balance blurring for public use.
- **Complete Money Management**:
  - **Accounts**: Cash, Bank, Savings, Credit Card, Investment, and Loan with multi-currency support.
  - **Transactions**: Categorized expenses, income, and internal transfers with search and filtering.
  - **Safe-to-Spend**: Dynamic algorithm calculating expendable money after fixed bills, debts, and budgets.
  - **Budgets & Bills**: Category budget thresholds and recurring bill reminders.
  - **Debts**: Track who owes you and what you owe.
  - **Reports**: Net worth trends, cash flow analysis, and spending breakdowns.
- **Optional Cloud Sync**: In-app background daemon syncing encrypted/hashed snapshots to Google Drive.
- **Multiple Themes**: Dark, Light, OLED/Midnight, and Emerald.

---

## Tech Stack

| Layer | Technology |
|---|---|
| **Language** | Rust (Edition 2024) |
| **GUI Framework** | Slint 1.18 |
| **Database** | SQLite via `rusqlite` (bundled, WAL mode) |
| **Security** | Argon2 (PIN hashing), SHA-256 (backup integrity) |
| **Cloud** | Google Drive API via `ureq` + local OAuth2 loopback |

---

## Quickstart

### Prerequisites

- [Rust & Cargo](https://rustup.rs/) (1.85+)
- Standard C compiler and graphics libraries (`libfontconfig`, `libxkbcommon`, `wayland` / `x11` on Linux)

### Build & Run

```bash
# Clone the repository
git clone https://github.com/your-username/urpwallet.git
cd urpwallet

# Run in debug mode
cargo run

# Build optimized release binary
cargo build --release
```

The release binary will be available at `./target/release/urpwallet`.

---

## Data & Backups

### Storage Locations

| OS | Default Database Path |
|---|---|
| **Linux** | `~/.local/share/urpwallet/wallet.db` |
| **macOS** | `~/Library/Application Support/urpwallet/wallet.db` |
| **Windows** | `%LOCALAPPDATA%\urpwallet\wallet.db` |

> *Note: A custom database location can be configured inside **Settings > Database Path**.*

### Export & Import

- **Manual**: Export and restore your complete database state to/from JSON or CSV under **Settings**.
- **Google Drive**: Authenticate once via OAuth2; a background thread automatically syncs changes whenever state hash changes.

---

## Project Structure

```text
├── assets/             # Icons, illustrations, and static assets
├── src/
│   ├── db/             # SQLite connection, migrations, and CRUD repositories
│   ├── logic/          # Safe-to-spend, currency conversion, and metric calculations
│   ├── ui/             # Slint event handlers, models, and UI state refreshers
│   ├── gdrive.rs       # Google Drive OAuth2 & automated sync daemon
│   ├── lib.rs          # Core library exports
│   └── main.rs         # Application entrypoint & event loop
├── ui/
│   ├── views/          # Slint screens (Dashboard, Accounts, Transactions, etc.)
│   ├── modals/         # Slint dialogs (Transaction editor, PIN lock, Cloud sync)
│   ├── components.slint# Reusable UI widgets
│   ├── theme.slint     # Color palettes & typography tokens
│   └── main.slint      # Root Slint window declaration
└── Cargo.toml          # Rust dependencies & build profile
```

---

## License

Distributed under the [MIT License](LICENSE).
