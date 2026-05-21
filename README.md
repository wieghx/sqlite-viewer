# sqlite-watch

A lightweight cross-platform SQLite database viewer written in Rust using egui + eframe + rusqlite.

## Features

- Open SQLite databases (.db, .sqlite, .sqlite3, etc.) via native file dialog or command line
- Browse tables in left sidebar (clickable, highlights current)
- View table contents in scrollable monospace-aligned grid
- Handles all SQLite data types: INTEGER, REAL, TEXT, BLOB, NULL
- Shows total row count and limits display to first 1000 rows (safe for large tables)
- Refresh current table, close DB, error display
- CLI support for direct open: `sqlite-viewer /path/to/your.db`
- Production-ready release binary (~18 MB)

## Building

```bash
cd sqlite-viewer
cargo build --release
# binary at target/release/sqlite-viewer
```

## Running

```bash
# GUI with open button
./target/release/sqlite-viewer

# Directly open a database
./target/release/sqlite-viewer /home/user/data/mydb.sqlite3
```

On first run, use the "📂 Open Database" button in the top bar.

## Dependencies

- rusqlite (bundled SQLite)
- eframe/egui (immediate mode GUI)
- rfd (native file dialogs)

## Status

Completed and functional as of 2026-05-21. See Plans.md for history.

## License

MIT or whatever, no license file yet.
