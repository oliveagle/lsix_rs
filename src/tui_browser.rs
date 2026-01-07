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
use std::fs::OpenOptions;
use std::io::{self, stdout, Write};

use std::path::Path;

fn is_logging_enabled() -> bool {
    std::env::var("LSIX_ENABLE_LOG").is_ok()
}

fn trace_log(msg: &str) {
    if !is_logging_enabled() {
        return;
    }
    
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/lsix_tui.log")
    {
        let timestamp = chrono::Local::now().format("%H:%M:%S%.3f");
        writeln!(file, "[{}] {}", timestamp, msg).ok();
    }
}

use image::{imageops::FilterType, ImageReader};
use ratatui_image::{picker::Picker, Resize, StatefulImage};
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
    pub fullscreen_mode: bool, // Whether we're in fullscreen image view mode
}

impl TuiBrowser {
    pub fn new(items: Vec<String>, current_dir: String) -> TuiBrowser {
        let mut state = ListState::default();
        state.select(Some(0));

        // Don't initialize the picker here - do it after raw mode is enabled
        TuiBrowser {
            items,
            state,
            current_dir,
            selected_image: None,
            grid_cols: 5,
            grid_rows: 0,
            scroll_offset: 0,
            image_cache: HashMap::new(),
            picker: None, // Will be initialized later
            fullscreen_mode: false,
        }
    }

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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
            let items_per_page = self.grid_cols as usize * self.grid_rows as usize;

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
    // Initialize log file if logging is enabled
    if is_logging_enabled() {
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open("/tmp/lsix_tui.log")
        {
            writeln!(file, "=== LSIX TUI Browser Log ===").ok();
            writeln!(file, "Start time: {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")).ok();
            writeln!(file, "Total images: {}\n", image_paths.len()).ok();
        }
    }
    
    trace_log("Starting TUI browser initialization");
    
    // Clear any pending input events before starting TUI
    // This prevents issues from terminal queries done before TUI initialization
    while event::poll(std::time::Duration::from_millis(0))? {
        event::read()?; // Consume and discard any pending events
    }
    
    trace_log("Terminal setup: enabling raw mode");
    
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    trace_log(&format!("Terminal initialized: size = {:?}", terminal.size()));

    // Create app state
    let current_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .to_string_lossy()
        .to_string();

    let mut app = TuiBrowser::new(image_paths, current_dir);
    
    trace_log("Initializing image picker");
    
    // Initialize the picker AFTER raw mode is enabled and terminal is setup
    // This should prevent blocking on terminal queries
    app.picker = Some(crate::term_image::create_picker());

    trace_log("Starting main event loop");

    // Run the main loop
    let res = run_app(&mut terminal, &mut app);

    trace_log("Exiting TUI browser, restoring terminal");

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

    trace_log("TUI browser shutdown complete");

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut TuiBrowser,
) -> io::Result<()> {
    // First draw to show the UI immediately
    terminal.draw(|f| ui(f, app))?;
    
    loop {
        // Use poll to check if there's an event available with a timeout
        // This allows the UI to update even if no key is pressed
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => {
                        if app.fullscreen_mode {
                            // Exit fullscreen mode
                            app.fullscreen_mode = false;
                            terminal.draw(|f| ui(f, app))?;
                        } else {
                            // Exit application
                            return Ok(());
                        }
                    }
                    KeyCode::Esc => {
                        if app.fullscreen_mode {
                            // Exit fullscreen mode
                            app.fullscreen_mode = false;
                            terminal.draw(|f| ui(f, app))?;
                        } else {
                            // Exit application
                            return Ok(());
                        }
                    }
                    KeyCode::Down => {
                        if app.fullscreen_mode {
                            // In fullscreen mode, ignore navigation
                            continue;
                        }
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
                        terminal.draw(|f| ui(f, app))?;
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
                        terminal.draw(|f| ui(f, app))?;
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
                        terminal.draw(|f| ui(f, app))?;
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
                        terminal.draw(|f| ui(f, app))?;
                    }
                    KeyCode::Char('g') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        app.state.select(Some(0));
                        app.update_selected_image();
                        terminal.draw(|f| ui(f, app))?;
                    }
                    KeyCode::Char('G') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        app.state.select(Some(app.items.len().saturating_sub(1)));
                        app.update_selected_image();
                        terminal.draw(|f| ui(f, app))?;
                    }
                    KeyCode::PageUp => {
                        // Move up by one page (grid size)
                        let items_per_page = (app.grid_cols * app.grid_rows) as usize;
                        let current = app.state.selected().unwrap_or(0);
                        let new_index = current.saturating_sub(items_per_page);
                        app.state.select(Some(new_index));
                        app.update_selected_image();
                        app.ensure_selection_visible();
                        terminal.draw(|f| ui(f, app))?;
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
                        terminal.draw(|f| ui(f, app))?;
                    }
                    KeyCode::Home => {
                        app.state.select(Some(0));
                        app.update_selected_image();
                        terminal.draw(|f| ui(f, app))?;
                    }
                    KeyCode::End => {
                        if !app.items.is_empty() {
                            app.state.select(Some(app.items.len() - 1));
                            app.update_selected_image();
                        }
                        terminal.draw(|f| ui(f, app))?;
                    }
                    KeyCode::Enter => {
                        trace_log(&format!(
                            "=== ENTER KEY PRESSED ===\n\
                            Current state:\n\
                            - fullscreen_mode: {}\n\
                            - selected_index: {:?}\n\
                            - selected_image: {:?}\n\
                            - terminal_size: {:?}\n\
                            - grid_cols: {}, grid_rows: {}\n\
                            - scroll_offset: {}\n\
                            - total_items: {}",
                            app.fullscreen_mode,
                            app.state.selected(),
                            app.selected_image.as_ref().and_then(|p| Path::new(p).file_name().map(|n| n.to_string_lossy().to_string())),
                            terminal.size(),
                            app.grid_cols,
                            app.grid_rows,
                            app.scroll_offset,
                            app.items.len()
                        ));
                        
                        // Toggle fullscreen mode
                        app.fullscreen_mode = !app.fullscreen_mode;
                        
                        trace_log(&format!(
                            "Toggling fullscreen mode: {} -> {}",
                            !app.fullscreen_mode,
                            app.fullscreen_mode
                        ));
                        
                        if app.fullscreen_mode {
                            trace_log("Entering fullscreen mode - rendering fullscreen image");
                        } else {
                            trace_log("Exiting fullscreen mode - returning to grid view");
                        }
                        
                        terminal.draw(|f| ui(f, app))?;
                        
                        trace_log("=== ENTER KEY HANDLED ===\n");
                    }
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &mut TuiBrowser) {
    // Check if we're in fullscreen mode
    if app.fullscreen_mode {
        render_fullscreen_image(f, app);
        return;
    }
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(0),    // Main content (thumbnails)
            Constraint::Length(3), // Status bar
        ])
        .split(f.area());

    // Header
    let header_block = Block::default()
        .borders(Borders::ALL)
        .title(format!("TUI Image Browser - {}", app.current_dir));
    f.render_widget(header_block, chunks[0]);

    // Main content - grid of thumbnails
    render_thumbnail_grid(f, app, chunks[1]);

    // Status bar
    let _selected_filename = if let Some(ref path) = app.selected_image {
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
        "q: Quit | Arrows: Nav | Enter: View | PgUp/PgDn: Page | {}/{} | Page {}/{}",
        current_pos,
        app.items.len(),
        page,
        total_pages
    );
    let status_bar = Paragraph::new(Text::from(Span::raw(status_text)))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(status_bar, chunks[2]);
}

fn render_fullscreen_image(f: &mut Frame, app: &mut TuiBrowser) {
    trace_log("=== RENDER_FULLSCREEN_IMAGE START ===");
    
    // Get the selected image
    if let Some(ref image_path) = app.selected_image {
        let filename = Path::new(image_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| image_path.clone());
        
        let current_pos = app.state.selected().unwrap_or(0) + 1;
        
        trace_log(&format!(
            "Fullscreen render:\n\
            - image_path: {}\n\
            - filename: {}\n\
            - position: {}/{}\n\
            - frame_area: {:?}",
            image_path, filename, current_pos, app.items.len(), f.area()
        ));
        
        // Use the entire screen for image, overlay status text
        let full_area = f.area();
        
        // Try to load and display the image
        if !app.image_cache.contains_key(image_path) {
            trace_log(&format!("Image not in cache, loading: {}", image_path));
            
            match ImageReader::open(image_path) {
                Ok(reader) => match reader.decode() {
                    Ok(img) => {
                        trace_log(&format!(
                            "Image loaded successfully:\n\
                            - dimensions: {}x{}\n\
                            - color_type: {:?}",
                            img.width(), img.height(), img.color()
                        ));
                        app.image_cache.insert(image_path.to_string(), img);
                    }
                    Err(e) => {
                        trace_log(&format!("Failed to decode image: {}", e));
                        let error_text = Paragraph::new("Error: Failed to decode image")
                            .block(Block::default().borders(Borders::ALL));
                        f.render_widget(error_text, full_area);
                        trace_log("=== RENDER_FULLSCREEN_IMAGE END (decode error) ===\n");
                        return;
                    }
                },
                Err(e) => {
                    trace_log(&format!("Failed to open image: {}", e));
                    let error_text = Paragraph::new("Error: Failed to open image")
                        .block(Block::default().borders(Borders::ALL));
                    f.render_widget(error_text, full_area);
                    trace_log("=== RENDER_FULLSCREEN_IMAGE END (open error) ===\n");
                    return;
                }
            }
        } else {
            trace_log("Image already in cache");
        }
        
        if let Some(image_data) = app.image_cache.get(image_path) {
            if let Some(ref picker) = app.picker {
                // Calculate pixel dimensions for better quality
                let font_size = picker.font_size();
                let display_height = full_area.height.saturating_sub(1);
                
                // Calculate target pixel size based on terminal area and font size
                let target_pixel_width = (full_area.width as u32) * (font_size.0 as u32);
                let target_pixel_height = (display_height as u32) * (font_size.1 as u32);
                
                trace_log(&format!(
                    "Creating image protocol:\n\
                    - original_size: {}x{}\n\
                    - display_area (cells): {}x{}\n\
                    - font_size: {:?}\n\
                    - target_pixels: {}x{}",
                    image_data.width(), image_data.height(),
                    full_area.width, display_height,
                    font_size,
                    target_pixel_width, target_pixel_height
                ));
                
                // Resize image to fit within 1920x1920 while maintaining aspect ratio
                let max_dimension = 1920;
                let (img_width, img_height) = (image_data.width(), image_data.height());
                
                let resized_image = {
                    // Calculate the scaling factor to fit within max_dimension
                    let scale = (max_dimension as f32) / img_width.max(img_height) as f32;
                    let new_width = (img_width as f32 * scale) as u32;
                    let new_height = (img_height as f32 * scale) as u32;
                    
                    trace_log(&format!(
                        "Resizing image: {}x{} -> {}x{} (scale: {:.2})",
                        img_width, img_height, new_width, new_height, scale
                    ));
                    
                    // Use Lanczos3 filter for high-quality downscaling
                    image_data.resize(new_width, new_height, FilterType::Lanczos3)
                };
                
                trace_log(&format!("Final image size: {}x{}", resized_image.width(), resized_image.height()));
                
                // Use new_resize_protocol which handles resizing automatically
                let mut image_protocol = picker.new_resize_protocol(resized_image);
                
                // Use Resize::Fit to maintain aspect ratio
                let image_widget = StatefulImage::new().resize(Resize::Fit(None));
                
                // Use almost the full screen (leave 1 line for status)
                let image_area = Rect {
                    x: 0,
                    y: 0,
                    width: full_area.width,
                    height: display_height,
                };
                
                trace_log(&format!("Rendering image to area: {:?}", image_area));
                
                f.render_stateful_widget(image_widget, image_area, &mut image_protocol);
                
                trace_log("Image rendered successfully");
            } else {
                trace_log("ERROR: picker is None!");
            }
        }
        
        // Render status bar at the bottom (overlay)
        let status_area = Rect {
            x: 0,
            y: full_area.height.saturating_sub(1),
            width: full_area.width,
            height: 1,
        };
        
        let status_text = format!(
            "{} | q/ESC: Back | {}/{}",
            filename,
            current_pos,
            app.items.len()
        );
        
        trace_log(&format!("Rendering status bar: '{}' at {:?}", status_text, status_area));
        
        let status_bar = Paragraph::new(Text::from(Span::raw(status_text)))
            .style(Style::default().bg(Color::Black).fg(Color::White));
        f.render_widget(status_bar, status_area);
    } else {
        trace_log("No image selected for fullscreen view");
    }
    
    trace_log("=== RENDER_FULLSCREEN_IMAGE END ===\n");
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
    let items_per_page = app.grid_cols as usize * app.grid_rows as usize;
    let end_idx = std::cmp::min(start_idx + items_per_page, app.items.len());

    trace_log(&format!(
        "=== RENDER START ====\nscroll_offset: {}, grid: {}x{}, area: {:?}\ncells: {}x{}, start_idx: {}, end_idx: {}, items_to_render: {}",
        app.scroll_offset, app.grid_cols, app.grid_rows, area,
        app.grid_cols, app.grid_rows, start_idx, end_idx, end_idx - start_idx
    ));

    let items_to_render: Vec<_> = app.items[start_idx..end_idx].to_vec();

    let clear_block = Paragraph::new("").style(Style::default().bg(Color::Black));
    f.render_widget(clear_block, area);

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

        if cell_area.width > 2 {
            cell_area.x += 1;
            cell_area.width -= 1;
        }
        if cell_area.height > 2 {
            cell_area.y += 1;
            cell_area.height -= 1;
        }

        trace_log(&format!(
            "[{:2}] pos=({},{}) area=({},{},{},{}) file={}",
            i, row, col, cell_area.x, cell_area.y, cell_area.width, cell_area.height, item_path
        ));

        // Draw a border around the selected image cell
        if let Some(selected_idx) = app.state.selected() {
            let actual_idx = start_idx + i;
            if selected_idx == actual_idx && cell_area.width > 2 && cell_area.height > 1 {
                let clear_block = Paragraph::new("").style(Style::default().bg(Color::Black));
                f.render_widget(clear_block, cell_area);

                let selection_block = Block::default().borders(Borders::ALL).border_style(
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                );
                f.render_widget(selection_block, cell_area);
            }
        }
        if cell_area.height > 2 {
            cell_area.y += 1;
            cell_area.height -= 1;
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

                f.render_stateful_widget(image_widget, image_area, &mut image_protocol);
            }
        }
    }

    trace_log(&format!(
        "=== RENDER END ====\nTotal items rendered: {}\n",
        items_to_render.len()
    ));

    // Add a border around the grid area with pagination info
    let page = (app.scroll_offset / items_per_page) + 1;
    let total_pages = (app.items.len() + items_per_page - 1) / items_per_page;
    let grid_block = Block::default().borders(Borders::ALL).title(format!(
        "Image Grid ({}x{}) - Page {}/{}",
        app.grid_cols, app.grid_rows, page, total_pages
    ));
    f.render_widget(grid_block, area);
}

