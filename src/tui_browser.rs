use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Span, Text},
    widgets::{Block, Borders, ListState, Paragraph},
    Frame, Terminal,
};
use std::io::{self, stdout};

use std::path::Path;

use crate::image_proc::{ImageConfig, ImageEntry};
use crate::terminal::autodetect;
use image::ImageReader;
use ratatui_image::{picker::Picker, StatefulImage};
use std::collections::HashMap;

pub struct TuiBrowser {
    pub items: Vec<String>,
    pub state: ListState,
    pub current_dir: String,
    pub selected_image: Option<String>,
    pub grid_cols: u16,
    pub grid_rows: u16,
    pub scroll_offset: usize,
    pub image_cache: HashMap<String, image::DynamicImage>,
    pub picker: Option<Picker>,
}

impl TuiBrowser {
    pub fn new(items: Vec<String>, current_dir: String) -> TuiBrowser {
        let mut state = ListState::default();
        state.select(Some(0));

        // Initialize the picker for image protocols
        let picker = match Picker::from_query_stdio() {
            Ok(picker) => Some(picker),
            Err(_) => {
                // Fallback to halfblocks if terminal query fails
                Some(Picker::halfblocks())
            }
        };

        TuiBrowser {
            items,
            state,
            current_dir,
            selected_image: None,
            grid_cols: 5,
            grid_rows: 0,
            scroll_offset: 0,
            image_cache: HashMap::new(),
            picker,
        }
    }

    pub fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.update_selected_image();
        self.ensure_selection_visible();
    }

    pub fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
        self.update_selected_image();
        self.ensure_selection_visible();
    }

    /// Ensure the selected item is visible in the current view
    fn ensure_selection_visible(&mut self) {
        if let Some(selected_idx) = self.state.selected() {
            let items_per_page = (self.grid_cols as usize * self.grid_rows as usize);

            // Calculate which page the selected item is on
            let selected_page = selected_idx / items_per_page;
            let current_start_page = self.scroll_offset / items_per_page;
            let current_end_page = (self.scroll_offset + items_per_page - 1) / items_per_page;

            // If the selected item is not on the current page, adjust the scroll
            if selected_page < current_start_page || selected_page > current_end_page {
                // Center the selected item's page
                self.scroll_offset = (selected_idx / items_per_page) * items_per_page;
            }
        }
    }

    fn update_selected_image(&mut self) {
        if let Some(idx) = self.state.selected() {
            if idx < self.items.len() {
                self.selected_image = Some(self.items[idx].clone());
            }
        }
    }
}

// Main function to run the TUI browser
pub fn run_tui_browser(image_paths: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let current_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .to_string_lossy()
        .to_string();

    let mut app = TuiBrowser::new(image_paths, current_dir);

    // Run the main loop
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("Error: {}", err);
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut TuiBrowser,
) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Down => {
                    if let Some(selected) = app.state.selected() {
                        let row = selected / app.grid_cols as usize;
                        let col = selected % app.grid_cols as usize;
                        let next_row = row + 1;
                        let next_idx = next_row * app.grid_cols as usize + col;

                        if next_idx < app.items.len() {
                            app.state.select(Some(next_idx));
                            app.update_selected_image();
                            app.ensure_selection_visible();
                        } else {
                            // If we're at the bottom row, wrap to top
                            let top_idx = col;
                            if top_idx < app.items.len() {
                                app.state.select(Some(top_idx));
                                app.update_selected_image();
                                app.ensure_selection_visible();
                            }
                        }
                    }
                }
                KeyCode::Up => {
                    if let Some(selected) = app.state.selected() {
                        let row = selected / app.grid_cols as usize;
                        let col = selected % app.grid_cols as usize;

                        if row > 0 {
                            // Move up to the same column in the previous row
                            let prev_row = row - 1;
                            let prev_idx = prev_row * app.grid_cols as usize + col;

                            if prev_idx < app.items.len() {
                                app.state.select(Some(prev_idx));
                                app.update_selected_image();
                                app.ensure_selection_visible();
                            }
                        } else {
                            // If we're at the top row, wrap to bottom
                            let total_rows = (app.items.len() + app.grid_cols as usize - 1)
                                / app.grid_cols as usize;
                            if total_rows > 1 {
                                let bottom_row = total_rows - 1;
                                let bottom_idx = bottom_row * app.grid_cols as usize + col;

                                if bottom_idx < app.items.len() {
                                    app.state.select(Some(bottom_idx));
                                    app.update_selected_image();
                                    app.ensure_selection_visible();
                                }
                            }
                        }
                    }
                }
                KeyCode::Left => {
                    // Move left in grid
                    if let Some(selected) = app.state.selected() {
                        if selected > 0 {
                            app.state.select(Some(selected - 1));
                            app.update_selected_image();
                            app.ensure_selection_visible();
                        }
                    }
                }
                KeyCode::Right => {
                    // Move right in grid
                    if let Some(selected) = app.state.selected() {
                        let next_idx = selected + 1;
                        if next_idx < app.items.len() {
                            app.state.select(Some(next_idx));
                            app.update_selected_image();
                            app.ensure_selection_visible();
                        }
                    }
                }
                KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.state.select(Some(0));
                    app.update_selected_image();
                }
                KeyCode::Char('G') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    app.state.select(Some(app.items.len().saturating_sub(1)));
                    app.update_selected_image();
                }
                KeyCode::PageUp => {
                    // Move up by one page (grid size)
                    let items_per_page = (app.grid_cols * app.grid_rows) as usize;
                    let current = app.state.selected().unwrap_or(0);
                    let new_index = current.saturating_sub(items_per_page);
                    app.state.select(Some(new_index));
                    app.update_selected_image();
                    app.ensure_selection_visible();
                }
                KeyCode::PageDown => {
                    // Move down by one page (grid size)
                    let items_per_page = (app.grid_cols * app.grid_rows) as usize;
                    let current = app.state.selected().unwrap_or(0);
                    let new_index =
                        std::cmp::min(current + items_per_page, app.items.len().saturating_sub(1));
                    app.state.select(Some(new_index));
                    app.update_selected_image();
                    app.ensure_selection_visible();
                }
                KeyCode::Home => {
                    app.state.select(Some(0));
                    app.update_selected_image();
                }
                KeyCode::End => {
                    if !app.items.is_empty() {
                        app.state.select(Some(app.items.len() - 1));
                        app.update_selected_image();
                    }
                }
                KeyCode::Enter => {
                    // Display the selected image in full size
                    if let Some(image_path) = &app.selected_image {
                        display_single_image(image_path)?;
                    }
                }
                _ => {}
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut TuiBrowser) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Main content (thumbnails)
            Constraint::Length(3), // Status bar
        ])
        .split(f.size());

    // Header
    let header_block = Block::default()
        .borders(Borders::ALL)
        .title(format!("TUI Image Browser - {}", app.current_dir));
    f.render_widget(header_block, chunks[0]);

    // Main content - grid of thumbnails
    render_thumbnail_grid(f, app, chunks[1]);

    // Status bar
    let selected_filename = if let Some(ref path) = app.selected_image {
        Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.clone())
    } else {
        "None".to_string()
    };

    let current_pos = app.state.selected().unwrap_or(0) + 1;
    let items_per_page = (app.grid_cols * app.grid_rows) as usize;
    let page = (app.scroll_offset / items_per_page) + 1;
    let total_pages = (app.items.len() + items_per_page - 1) / items_per_page;

    let status_text = format!(
        "q: Quit | hjkl: Nav | Enter: View | PgUp/PgDn: Page | {}/{} | Page {}/{}",
        current_pos,
        app.items.len(),
        page,
        total_pages
    );
    let status_bar = Paragraph::new(Text::from(Span::raw(status_text)))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(status_bar, chunks[2]);
}

fn render_thumbnail_grid(f: &mut Frame, app: &mut TuiBrowser, area: Rect) {
    let min_cell_width = 12;
    let min_cell_height = 8;

    let max_cols = std::cmp::max(1, area.width / min_cell_width);
    let max_rows = std::cmp::max(1, area.height / min_cell_height);

    app.grid_cols = std::cmp::min(max_cols, 5);
    app.grid_rows = std::cmp::min(max_rows, 3);

    let cell_width = area.width / app.grid_cols;
    let cell_height = area.height / app.grid_rows;

    let start_idx = app.scroll_offset;
    let items_per_page = (app.grid_cols as usize * app.grid_rows as usize);
    let end_idx = std::cmp::min(start_idx + items_per_page, app.items.len());

    let items_to_render: Vec<_> = app.items[start_idx..end_idx].to_vec();

    for (i, item_path) in items_to_render.iter().enumerate() {
        let row = (i / app.grid_cols as usize) as u16;
        let col = (i % app.grid_cols as usize) as u16;

        // Calculate the area for this specific image
        let mut cell_area = Rect {
            x: area.x + col * cell_width,
            y: area.y + row * cell_height,
            width: cell_width,
            height: cell_height,
        };

        // Add a small margin between cells
        if cell_area.width > 2 {
            cell_area.x += 1;
            cell_area.width -= 1;
        }
        if cell_area.height > 2 {
            cell_area.y += 1;
            cell_area.height -= 1;
        }

        // Draw a border around the selected image cell
        if let Some(selected_idx) = app.state.selected() {
            let actual_idx = start_idx + i;
            if selected_idx == actual_idx && cell_area.width > 2 && cell_area.height > 1 {
                let selection_block = Block::default().borders(Borders::ALL).border_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                );
                f.render_widget(selection_block, cell_area);
            }
        }

        // Try to load the image if not already cached
        if !app.image_cache.contains_key(item_path) {
            match ImageReader::open(item_path) {
                Ok(reader) => match reader.decode() {
                    Ok(img) => {
                        app.image_cache.insert(item_path.to_string(), img);
                    }
                    Err(_) => {
                        continue;
                    }
                },
                Err(_) => {
                    continue;
                }
            }
        }

        if let Some(image_data) = app.image_cache.get(item_path) {
            if let Some(ref picker) = app.picker {
                let mut image_protocol = picker.new_resize_protocol(image_data.clone());

                let image_widget = StatefulImage::new();

                let image_area = Rect {
                    x: cell_area.x + 2,
                    y: cell_area.y + 1,
                    width: if cell_area.width > 4 {
                        cell_area.width - 4
                    } else {
                        cell_area.width
                    },
                    height: if cell_area.height > 2 {
                        cell_area.height - 2
                    } else {
                        cell_area.height
                    },
                };

                let clear_block = Paragraph::new("").style(Style::default().bg(Color::Black));
                f.render_widget(clear_block, image_area);
                f.render_stateful_widget(image_widget, image_area, &mut image_protocol);
            }
        }
    }

    // Add a border around the grid area with pagination info
    let page = (app.scroll_offset / items_per_page) + 1;
    let total_pages = (app.items.len() + items_per_page - 1) / items_per_page;
    let grid_block = Block::default().borders(Borders::ALL).title(format!(
        "Image Grid ({}x{}) - Page {}/{}",
        app.grid_cols, app.grid_rows, page, total_pages
    ));
    f.render_widget(grid_block, area);
}

fn display_single_image(image_path: &str) -> Result<(), std::io::Error> {
    use crossterm::execute;
    use std::io::stdout;

    // Temporarily exit the TUI to display the image
    disable_raw_mode().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    let mut stdout = stdout();
    execute!(stdout, LeaveAlternateScreen)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    // Auto-detect terminal capabilities
    let term_config = autodetect().unwrap_or_else(|_| {
        eprintln!("Warning: Could not detect terminal capabilities");
        crate::terminal::TerminalConfig {
            width: 80,
            num_colors: 256,
            background: "black".to_string(),
            foreground: "white".to_string(),
            has_sixel: false,
        }
    });

    // Create image config based on terminal
    let img_config = ImageConfig::from_terminal_width(
        term_config.width,
        term_config.num_colors,
        &term_config.background,
        &term_config.foreground,
    );

    // Create a single ImageEntry for the selected image
    let image_entry = ImageEntry {
        path: image_path.to_string(),
        label: std::path::Path::new(image_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| image_path.to_string()),
    };

    // Process and display the single image
    if let Err(e) = crate::image_proc::process_images_concurrent(vec![image_entry], &img_config) {
        eprintln!("Error displaying image: {}", e);
    }

    // Wait for a key press before returning to TUI
    eprintln!("Press Enter to return to browser...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();

    // Re-enter raw mode and alternate screen for TUI
    enable_raw_mode().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    execute!(stdout, EnterAlternateScreen)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    Ok(())
}
