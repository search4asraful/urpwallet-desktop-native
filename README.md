# urpwallet

A local-first, lightweight native personal finance tracker and budget planner built with **Rust** and **Slint**.

---

## Features

- **Local-First & Private**: Your financial data stays entirely on your local machine in an SQLite database (WAL mode, foreign keys enabled). Zero analytics, third-party tracking, or telemetry.
- **Privacy Controls**: Secure Argon2 master PIN lock with lockout overlay and one-click balance masking for privacy in public spaces.
- **Comprehensive Money Management**:
  - **Accounts**: Multi-currency support across Cash, Bank, Savings, Credit Card, Investment, and Loan accounts.
  - **Transactions**: Categorized income, expenses, and internal transfers with real-time search, category filtering, and date ranges.
  - **Safe-to-Spend**: Dynamic algorithm calculating expendable money after accounting for scheduled bills, active debts, and budget limits.
  - **Budgets & Bills**: Category budget thresholds with visual progress meters and recurring bill tracking with due-date reminders.
  - **Debts**: Track money you owe and money owed to you, with one-click repayment logging.
  - **Reports**: Visual spending breakdowns, income vs. expense cash flow trends, and net worth tracking.
- **Cloud Synchronization**: Optional automated background daemon that syncs JSON ledger snapshots to your personal Google Drive via OAuth2, featuring SHA-256 state hashing for smart sync and configurable retention pruning.
- **Data Portability**: Full JSON backup and restore, CSV transaction import and export, and configurable database location switcher with built-in data migration.
- **Customizable UI**: Native hardware-accelerated interface with Dark, Light, OLED/Midnight, and Emerald themes.

---

## Downloads

Official release packages are available on the [GitHub Releases](https://github.com/search4asraful/urpwallet-desktop-native/releases) page:

| Platform | Format / Architecture | Release Asset Name |
| :--- | :--- | :--- |
| **Windows** | NSIS Installer (`.exe`) | `urpwallet-x86_64-setup.exe` |
| **Linux** | Debian / Ubuntu Package (`.deb`) | `urpwallet_<version>-1_amd64.deb` |
| **Linux** | Universal AppImage (`.AppImage`) | `urpwallet-x86_64.AppImage` |
| **macOS** | Disk Image (`.dmg`) | `urpwallet-macOS.dmg` |
| **macOS** | Apple Silicon Archive (`.tar.xz`) | `urpwallet-aarch64-apple-darwin.tar.xz` |
| **macOS** | Intel x86_64 Archive (`.tar.xz`) | `urpwallet-x86_64-apple-darwin.tar.xz` |

> **Integrity Verification**: Every release includes a consolidated `sha256.sum` file. Verify downloaded assets via:
> ```bash
> sha256sum -c sha256.sum --ignore-missing
> ```

---

## Installation

### Windows
1. Download `urpwallet-x86_64-setup.exe`.
2. Run the installer to install the application to `Program Files`. The setup wizard automatically sets up Start Menu and Desktop shortcuts and integrates with Windows Add/Remove Programs.

### Linux

#### Debian / Ubuntu / Mint / Pop!_OS (`.deb`)
Download the `.deb` package and install it via terminal or software center:
```bash
sudo apt install ./urpwallet_*_amd64.deb
```
*Automatically integrates into your system application launcher and desktop search.*

#### Universal Linux (`.AppImage`)
Download the `.AppImage` file, grant executable permissions, and run:
```bash
chmod +x urpwallet-x86_64.AppImage
./urpwallet-x86_64.AppImage
```

### macOS
1. Download `urpwallet-macOS.dmg`.
2. Open the disk image and drag **urpwallet** to your `/Applications` directory.
*(Alternatively, extract standalone archives `urpwallet-*-apple-darwin.tar.xz` directly.)*

---

## Building from Source

### Prerequisites

- [Rust & Cargo](https://rustup.rs/) (1.85+)
- C compiler and platform graphics dependencies:
  - **Ubuntu / Debian**:
    ```bash
    sudo apt-get install -y build-essential libfontconfig1-dev libfreetype6-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev
    ```
  - **Fedora**:
    ```bash
    sudo dnf install fontconfig-devel freetype-devel libxkbcommon-devel libxkbcommon-x11-devel wayland-devel
    ```
  - **Arch Linux**:
    ```bash
    sudo pacman -S base-devel fontconfig freetype2 libxkbcommon wayland
    ```

### Build & Run

```bash
# 1. Clone the repository
git clone https://github.com/search4asraful/urpwallet-desktop-native.git
cd urpwallet-desktop-native

# 2. Run in development mode (with console logging enabled)
cargo run

# 3. Build optimized release binary
cargo build --release
```

The compiled release executable will be available at `./target/release/urpwallet` (or `urpwallet.exe` on Windows, built with the Windows subsystem to run purely as a GUI without a terminal popup).

---

## Data Storage & Backups

### Default Database Locations

| Operating System | Default Path |
| :--- | :--- |
| **Linux** | `~/.local/share/urpwallet/wallet.db` |
| **macOS** | `~/Library/Application Support/urpwallet/wallet.db` |
| **Windows** | `%LOCALAPPDATA%\urpwallet\wallet.db` |

> **Custom Location**: You can change your active database location or relocate your existing ledger anytime under **Settings > Database Location**.

### Export & Synchronization

- **Manual Exports**: Full ledger state exports (JSON) and transaction history exports (CSV) can be generated on demand under **Settings > Export / Import Data**.
- **Google Drive Sync**: Authenticate securely using Google OAuth2 to enable automated, differential ledger backups with configurable backup frequencies and retention limits.

---

## Tech Stack

| Component | Technology |
| :--- | :--- |
| **Core Language** | [Rust](https://www.rust-lang.org/) (2024 edition) |
| **GUI Framework** | [Slint](https://slint.dev/) (1.18) |
| **Embedded Database** | SQLite 3 via [`rusqlite`](https://crates.io/crates/rusqlite) (bundled, WAL mode) |
| **Security & Hashing** | Argon2 (PIN hashing) & SHA-256 (backup state integrity) |
| **Installer (Windows)** | NSIS (Modern UI 2) |
| **Packaging (Linux/macOS)** | `cargo-deb`, AppImageKit, and `create-dmg` |

---

## License

Distributed under the [MIT License](LICENSE).
