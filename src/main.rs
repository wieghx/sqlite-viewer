use eframe::egui;
use rusqlite::{Connection, Result as SqliteResult, types::Value as SqlValue};
use std::cell::RefCell;

struct AppState {
    current_db: RefCell<Option<Connection>>,
    current_table: RefCell<Option<String>>,
    table_list: RefCell<Vec<String>>,
    table_data: RefCell<Vec<Vec<String>>>,
    column_names: RefCell<Vec<String>>,
    error_message: RefCell<Option<String>>,
    total_rows: RefCell<Option<usize>>,
    page_size: RefCell<usize>,
    current_page: RefCell<usize>,
    raw_data: RefCell<Vec<Vec<SqlValue>>>,
    inspector_open: RefCell<bool>,
    inspector_title: RefCell<String>,
    inspector_content: RefCell<String>,
}

impl AppState {
    fn new() -> Self {
        Self {
            current_db: RefCell::new(None),
            current_table: RefCell::new(None),
            table_list: RefCell::new(Vec::new()),
            table_data: RefCell::new(Vec::new()),
            column_names: RefCell::new(Vec::new()),
            error_message: RefCell::new(None),
            total_rows: RefCell::new(None),
            page_size: RefCell::new(500),
            current_page: RefCell::new(0),
            raw_data: RefCell::new(Vec::new()),
            inspector_open: RefCell::new(false),
            inspector_title: RefCell::new(String::new()),
            inspector_content: RefCell::new(String::new()),
        }
    }

    fn get_tables(&self, connection: &Connection) -> SqliteResult<Vec<String>> {
        let mut stmt = connection.prepare(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
        )?;
        let table_iter = stmt.query_map([], |row| row.get::<_, String>(0))?;
        table_iter.collect()
    }

    fn get_table_data(&self, connection: &mut Connection, table_name: &str) -> SqliteResult<()> {
        let tx = connection.transaction()?;

        let count_sql = format!("SELECT COUNT(*) FROM \"{}\"", table_name);
        let total: i64 = tx.query_row(&count_sql, [], |row| row.get(0))?;
        let total_usize = total as usize;
        *self.total_rows.borrow_mut() = Some(total_usize);

        let mut page_size = *self.page_size.borrow();
        if page_size == 0 {
            page_size = 10000;
        }
        let mut page = *self.current_page.borrow();
        let total_pages = if page_size == 0 { 1 } else { (total_usize + page_size - 1) / page_size };
        if total_pages > 0 && page >= total_pages {
            page = total_pages - 1;
            *self.current_page.borrow_mut() = page;
        }
        let offset = page * page_size;

        let select_sql = format!(
            "SELECT * FROM \"{}\" ORDER BY rowid LIMIT {} OFFSET {}",
            table_name, page_size, offset
        );
        let mut stmt = tx.prepare(&select_sql)?;
        let column_names: Vec<String> = stmt.column_names().into_iter().map(|s| s.to_string()).collect();

        let rows = stmt.query_map([], |row| {
            let mut display_row = Vec::new();
            let mut raw_row = Vec::new();
            for i in 0..column_names.len() {
                let val: SqlValue = row.get(i).unwrap_or(SqlValue::Null);
                raw_row.push(val.clone());
                let s = format_value_for_grid(&val);
                display_row.push(s);
            }
            Ok((display_row, raw_row))
        })?;

        let collected: Vec<(Vec<String>, Vec<SqlValue>)> = rows.collect::<SqliteResult<_>>()?;
        let (display_rows, raw_rows): (Vec<_>, Vec<_>) = collected.into_iter().unzip();

        *self.column_names.borrow_mut() = column_names;
        *self.table_data.borrow_mut() = display_rows;
        *self.raw_data.borrow_mut() = raw_rows;

        Ok(())
    }

    fn load_database(&mut self, file_path: &str) {
        *self.error_message.borrow_mut() = None;
        match Connection::open(file_path) {
            Ok(conn) => {
                match self.get_tables(&conn) {
                    Ok(tables) => {
                        *self.current_db.borrow_mut() = Some(conn);
                        *self.table_list.borrow_mut() = tables;
                        *self.table_data.borrow_mut() = Vec::new();
                        *self.column_names.borrow_mut() = Vec::new();
                        *self.raw_data.borrow_mut() = Vec::new();
                        *self.current_table.borrow_mut() = None;
                        *self.total_rows.borrow_mut() = None;
                        *self.current_page.borrow_mut() = 0;
                        *self.inspector_open.borrow_mut() = false;
                    }
                    Err(e) => {
                        *self.error_message.borrow_mut() = Some(format!("Failed to list tables: {}", e));
                    }
                }
            }
            Err(e) => {
                *self.error_message.borrow_mut() = Some(format!("Failed to open database '{}': {}", file_path, e));
            }
        }
    }

    fn load_table(&mut self, table_name: &str) {
        *self.error_message.borrow_mut() = None;
        if let Some(conn) = self.current_db.borrow_mut().as_mut() {
            match self.get_table_data(conn, table_name) {
                Ok(()) => {
                    *self.current_table.borrow_mut() = Some(table_name.to_string());
                }
                Err(e) => {
                    *self.error_message.borrow_mut() = Some(format!("Error loading table '{}': {}", table_name, e));
                }
            }
        }
    }
}

fn format_value_for_grid(val: &SqlValue) -> String {
    match val {
        SqlValue::Null => "NULL".to_string(),
        SqlValue::Integer(i) => i.to_string(),
        SqlValue::Real(f) => f.to_string(),
        SqlValue::Text(t) => {
            if t.len() > 120 {
                let mut s = t.chars().take(100).collect::<String>();
                s.push('…');
                s
            } else {
                t.clone()
            }
        }
        SqlValue::Blob(b) => {
            let len = b.len();
            let preview: String = b.iter().take(8).map(|byte| format!("{:02X}", byte)).collect::<Vec<_>>().join(" ");
            if len > 8 {
                format!("[BLOB {}B] {}", len, preview)
            } else if len == 0 {
                "[BLOB 0B]".to_string()
            } else {
                format!("[BLOB] {}", preview)
            }
        }
    }
}

fn format_value_for_inspector(val: &SqlValue) -> String {
    match val {
        SqlValue::Null => "NULL".to_string(),
        SqlValue::Integer(i) => format!("INTEGER: {}", i),
        SqlValue::Real(f) => format!("REAL: {}", f),
        SqlValue::Text(t) => format!("TEXT ({} chars):\n{}", t.len(), t),
        SqlValue::Blob(b) => {
            let mut out = format!("BLOB ({} bytes, 0x{:X})\n\n", b.len(), b.len());
            let max = b.len().min(4096);
            for (i, chunk) in b[..max].chunks(16).enumerate() {
                let off = i * 16;
                let hexs: Vec<String> = chunk.iter().map(|x| format!("{:02x}", x)).collect();
                let hex_part = hexs.join(" ");
                let ascii: String = chunk.iter().map(|&x| if (32..127).contains(&x) { x as char } else { '.' }).collect();
                out.push_str(&format!("{:08x}  {:<47}  |{}|\n", off, hex_part, ascii));
            }
            if b.len() > max {
                out.push_str(&format!("\n... ({} more bytes truncated for display)\n", b.len() - max));
            }
            out
        }
    }
}

struct SQLiteViewer {
    app_state: AppState,
}

impl SQLiteViewer {
    fn new() -> Self {
        Self {
            app_state: AppState::new(),
        }
    }

    fn new_with_db(path: &str) -> Self {
        let mut viewer = Self::new();
        viewer.app_state.load_database(path);
        viewer
    }
}

impl eframe::App for SQLiteViewer {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("SQLite Viewer");
                if ui.button("📂 Open Database").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .set_title("Open SQLite Database")
                        .add_filter("SQLite", &["db", "sqlite", "sqlite3", "db3"])
                        .add_filter("All files", &["*"])
                        .pick_file()
                    {
                        if let Some(p) = path.to_str() {
                            self.app_state.load_database(p);
                        }
                    }
                }
                if ui.button("🔄 Refresh").clicked() {
                    let table_to_refresh = self.app_state.current_table.borrow().clone();
                    if let Some(table) = table_to_refresh {
                        self.app_state.load_table(&table);
                    }
                }
                if ui.button("✖ Close DB").clicked() {
                    *self.app_state.current_db.borrow_mut() = None;
                    *self.app_state.table_list.borrow_mut() = Vec::new();
                    *self.app_state.table_data.borrow_mut() = Vec::new();
                    *self.app_state.column_names.borrow_mut() = Vec::new();
                    *self.app_state.raw_data.borrow_mut() = Vec::new();
                    *self.app_state.current_table.borrow_mut() = None;
                    *self.app_state.error_message.borrow_mut() = None;
                    *self.app_state.total_rows.borrow_mut() = None;
                    *self.app_state.current_page.borrow_mut() = 0;
                    *self.app_state.inspector_open.borrow_mut() = false;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(table) = self.app_state.current_table.borrow().as_ref() {
                        ui.label(format!("📋 {}", table));
                    }
                    if let Some(err) = self.app_state.error_message.borrow().as_ref() {
                        ui.colored_label(egui::Color32::RED, format!("⚠ {}", err));
                    }
                });
            });
        });

        egui::SidePanel::left("tables_panel")
            .resizable(true)
            .default_width(220.0)
            .show(ctx, |ui| {
                ui.heading("Tables");
                ui.separator();

                let table_list = self.app_state.table_list.borrow().clone();
                let current = self.app_state.current_table.borrow().clone();

                if table_list.is_empty() {
                    ui.label("No database loaded.\nUse 'Open Database' above.");
                    return;
                }

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for table in table_list.iter() {
                        let is_current = current.as_ref() == Some(table);
                        if ui.selectable_label(is_current, table).clicked() {
                            *self.app_state.current_page.borrow_mut() = 0;
                            *self.app_state.inspector_open.borrow_mut() = false;
                            self.app_state.load_table(table);
                        }
                    }
                });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Data");

            let has_db = self.app_state.current_db.borrow().is_some();
            if !has_db {
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("Open a SQLite database file to begin browsing.\n\nSupports .db, .sqlite, .sqlite3 files.\n\nPagination + accurate BLOB hex + large value inspector.").size(14.0));
                });
                return;
            }

            let table_data = self.app_state.table_data.borrow().clone();
            let column_names = self.app_state.column_names.borrow().clone();
            let current_table = self.app_state.current_table.borrow().clone();
            let total_rows = *self.app_state.total_rows.borrow();
            let raw_data = self.app_state.raw_data.borrow().clone();
            let page_size = *self.app_state.page_size.borrow();
            let current_page = *self.app_state.current_page.borrow();
            let total_pages = {
                let t = total_rows.unwrap_or(0);
                if page_size == 0 { 1 } else { (t + page_size - 1) / page_size }
            };

            if table_data.is_empty() {
                if let Some(t) = current_table {
                    ui.label(format!("Loading table '{}' or table is empty...", t));
                } else {
                    ui.label("Select a table from the left panel to view its data.");
                }
                return;
            }

            let row_display = if let Some(total) = total_rows {
                if total_pages > 1 {
                    let start = current_page * page_size + 1;
                    let end = (start + table_data.len().saturating_sub(1)).min(total);
                    format!("{} (page {}/{} — rows {}-{} of {})", current_table.clone().unwrap_or_default(), current_page + 1, total_pages, start, end, total)
                } else {
                    format!("{} ({} rows)", current_table.clone().unwrap_or_default(), total)
                }
            } else {
                format!("{} ({} rows)", current_table.clone().unwrap_or_default(), table_data.len())
            };
            ui.horizontal(|ui| {
                ui.strong(&row_display);
                ui.label(format!(" | {} columns", column_names.len()));
            });

            ui.horizontal_wrapped(|ui| {
                ui.strong("Page size:");
                for &sz in &[50usize, 100, 200, 500, 1000, 2000, 5000] {
                    let sel = sz == page_size;
                    if ui.selectable_label(sel, sz.to_string()).clicked() {
                        *self.app_state.page_size.borrow_mut() = sz;
                        *self.app_state.current_page.borrow_mut() = 0;
                        if let Some(ref t) = current_table {
                            self.app_state.load_table(t);
                        }
                    }
                }
                ui.separator();
                if ui.button("⏮ First").clicked() && current_page > 0 {
                    *self.app_state.current_page.borrow_mut() = 0;
                    if let Some(ref t) = current_table {
                        self.app_state.load_table(t);
                    }
                }
                if ui.button("◀ Prev").clicked() && current_page > 0 {
                    *self.app_state.current_page.borrow_mut() = current_page - 1;
                    if let Some(ref t) = current_table {
                        self.app_state.load_table(t);
                    }
                }
                ui.label(format!(" Page {} / {} ", current_page + 1, total_pages.max(1)));
                if ui.button("Next ▶").clicked() && current_page + 1 < total_pages {
                    *self.app_state.current_page.borrow_mut() = current_page + 1;
                    if let Some(ref t) = current_table {
                        self.app_state.load_table(t);
                    }
                }
                if ui.button("Last ⏭").clicked() && current_page + 1 < total_pages {
                    *self.app_state.current_page.borrow_mut() = total_pages - 1;
                    if let Some(ref t) = current_table {
                        self.app_state.load_table(t);
                    }
                }
            });
            ui.separator();

            let mut widths: Vec<usize> = column_names.iter().map(|c| c.len().min(40)).collect();
            for row in table_data.iter() {
                for (i, cell) in row.iter().enumerate() {
                    if i < widths.len() {
                        widths[i] = widths[i].max(cell.len().min(40));
                    }
                }
            }

            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        for (i, c) in column_names.iter().enumerate() {
                            let w = widths[i].max(8);
                            let txt = format!("{: <w$}", c, w = w);
                            ui.label(egui::RichText::new(txt).strong().monospace().size(12.0));
                            ui.add_space(3.0);
                        }
                    });
                    ui.separator();

                    let tbl_name = current_table.clone().unwrap_or_default();
                    for (ridx, row) in table_data.iter().enumerate() {
                        ui.horizontal(|ui| {
                            for (cidx, cell) in row.iter().enumerate() {
                                let w = if cidx < widths.len() { widths[cidx].max(8) } else { 8 };
                                let txt = format!("{: <w$}", cell, w = w);
                                let label = egui::Label::new(egui::RichText::new(txt).monospace().size(11.0))
                                    .sense(egui::Sense::click());
                                let resp = ui.add(label).on_hover_text("Click to inspect full value / hex dump");
                                if resp.clicked() {
                                    if let Some(raw_row) = raw_data.get(ridx) {
                                        if let Some(raw_val) = raw_row.get(cidx) {
                                            let col_name = column_names.get(cidx).cloned().unwrap_or_default();
                                            let type_str = match raw_val {
                                                SqlValue::Null => "NULL",
                                                SqlValue::Integer(_) => "INTEGER",
                                                SqlValue::Real(_) => "REAL",
                                                SqlValue::Text(_) => "TEXT",
                                                SqlValue::Blob(_) => "BLOB",
                                            };
                                            let title = format!("{}  •  row {}  •  '{}' ({})", tbl_name, ridx + 1, col_name, type_str);
                                            let content = format_value_for_inspector(raw_val);
                                            *self.app_state.inspector_title.borrow_mut() = title;
                                            *self.app_state.inspector_content.borrow_mut() = content;
                                            *self.app_state.inspector_open.borrow_mut() = true;
                                        }
                                    }
                                }
                            }
                        });
                    }

                    ui.add_space(8.0);
                    ui.label(egui::RichText::new("💡 Tip: Click any cell to open full text or accurate BLOB hex dump (up to 4KB preview)").size(10.0).italics());
                });
        });

        {
            let open = *self.app_state.inspector_open.borrow();
            if open {
                let title = self.app_state.inspector_title.borrow().clone();
                let content = self.app_state.inspector_content.borrow().clone();
                let mut win_open = true;
                egui::Window::new(&title)
                    .resizable(true)
                    .default_width(720.0)
                    .default_height(440.0)
                    .show(ctx, |ui| {
                        ui.label(egui::RichText::new("Full value (TEXT) or hex+ASCII dump (BLOB, first 4 KiB)").size(10.0).italics());
                        egui::ScrollArea::vertical().max_height(340.0).show(ui, |ui| {
                            ui.monospace(&content);
                        });
                        ui.horizontal(|ui| {
                            if ui.button("📋 Copy full content to clipboard").clicked() {
                                ui.ctx().copy_text(content.clone());
                            }
                            if ui.button("Close").clicked() {
                                win_open = false;
                            }
                        });
                    });
                if !win_open {
                    *self.app_state.inspector_open.borrow_mut() = false;
                }
            }
        }

        egui::TopBottomPanel::bottom("status_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.app_state.current_db.borrow().is_some() {
                    ui.label("✅ Database connected");
                } else {
                    ui.label("No database open");
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label("SQLite Viewer v0.1 | Powered by egui + rusqlite");
                });
            });
        });
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let viewer = if args.len() > 1 {
        SQLiteViewer::new_with_db(&args[1])
    } else {
        SQLiteViewer::new()
    };

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("SQLite Viewer"),
        ..Default::default()
    };

    eframe::run_native(
        "SQLite Viewer",
        options,
        Box::new(move |_cc| Ok(Box::new(viewer))),
    )
}
