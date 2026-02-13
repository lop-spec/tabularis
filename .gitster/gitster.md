# Tabularis

**Version:** 0.8.12

## What is Tabularis?

Tabularis is a lightweight database management tool designed to make working with databases simple and intuitive. It provides a modern interface to connect, explore, query, and manage your databases without complexity.

## Key Features

### 🔌 Connection Management

**Easily connect to your databases**
- Connect to MySQL and MariaDB databases (PostgreSQL and SQLite support coming soon)
- Save your connection profiles for quick access
- Securely store passwords in your system keychain
- Connect through SSH tunnels for remote databases
- Manage multiple SSH connections from one place

### 🗂️ Database Explorer

**Navigate and understand your database structure**
- Browse through your tables, columns, and relationships in a visual tree
- View interactive diagrams showing how your tables connect to each other
- See table data, row counts, and schema details with a single click
- Export entire databases or import them back
- Duplicate or delete tables directly from the interface
- Explore views and stored procedures with full metadata

### ✍️ SQL Editor

**Write and execute queries with ease**
- Write SQL queries with smart suggestions and syntax highlighting
- Work with multiple queries in separate tabs
- Run entire scripts, selected portions, or single statements
- Save frequently used queries for later
- Each tab maintains its own database connection

### 🎨 Visual Query Builder

**Build queries without writing SQL**
- Drag and drop tables to build queries visually
- Create relationships between tables by connecting them
- Add filters, sorting, and limits using simple controls
- Use aggregation functions like COUNT, SUM, and AVG
- See the SQL code generated in real-time as you build
- Perfect for learning SQL or building complex queries quickly

### 📊 Data Grid

**View and edit your data**
- Browse query results in a fast, responsive table
- Edit data directly in the grid or make bulk changes
- Add new rows or delete existing ones
- Select and copy multiple rows to clipboard
- Export results to CSV or JSON files
- Smart mode: read-only for aggregated data, editable for tables

### 🤖 AI Assistant (Optional)

**Let AI help you write queries**
- Describe what you want in plain English and get SQL queries
- Get explanations of what complex queries do
- Works with OpenAI, Anthropic (Claude), or other AI providers
- Use Ollama for completely local AI (no data leaves your computer)
- Compatible with various AI services including Groq and Perplexity

### 📝 Logging & Preferences

**Stay in control**
- View application logs in real-time to troubleshoot issues
- Filter logs by severity level (DEBUG, INFO, WARN, ERROR)
- Export logs for sharing or debugging
- Your workspace is automatically saved: tabs, queries, and layouts are restored when you reconnect
- Customize theme, language, font size, and page size
- All settings stored locally on your computer

### 🌍 Multiple Languages

**Use Tabularis in your language**
- Available in English, Italian, and Spanish
- Automatic language detection or manual selection
- More languages coming soon

## Supported Databases

- **MySQL** (Full support)
- **MariaDB** (Full support)
- **PostgreSQL** (In development)
- **SQLite** (In development)

## Available On

- Windows (64-bit)
- macOS (Intel, available via Homebrew)
- Linux (AppImage, AUR package for Arch Linux)

## Updates

Tabularis automatically checks for updates when you start the app. When a new version is available, you'll be notified and can install it with one click.

## Links

- **Download:** [GitHub Releases](https://github.com/debba/tabularis/releases)
- **Website:** [tabularis.dev](https://tabularis.dev)
- **Community:** [Discord Server](https://discord.gg/YrZPHAwMSG)
- **Source Code:** [GitHub Repository](https://github.com/debba/tabularis)

## Coming Soon

- Command palette for quick actions
- JSON/JSONB editor and viewer
- SQL formatting and beautification
- Visual explain plans for query optimization
- Data comparison and diff tools
- Team collaboration features
- Plugin system for extensions
- Query history tracking
