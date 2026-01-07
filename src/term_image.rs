use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use image::ImageReader;
use ratatui::backend::CrosstermBackend;
use ratatui_image::{picker::Picker, StatefulImage};
use std::io::stdout;

pub fn create_picker() -> Picker {
    // Use from_query_stdio which should work fine when called after raw mode is enabled
    match Picker::from_query_stdio() {
        Ok(picker) => picker,
        Err(_) => {
            // Fallback to halfblocks if terminal query fails
            Picker::halfblocks()
        }
    }
}

#[allow(dead_code)]
pub fn render_single_image(image_path: &str) -> Result<()> {
    let picker = create_picker();

    let dyn_img = ImageReader::open(image_path)?
        .decode()
        .context("Failed to decode image")?;

    let mut image_protocol = picker.new_resize_protocol(dyn_img);

    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    enable_raw_mode()?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    terminal.draw(|f| {
        let widget = StatefulImage::default();
        f.render_stateful_widget(widget, f.area(), &mut image_protocol);
    })?;

    if let Some(Err(e)) = image_protocol.last_encoding_result() {
        eprintln!("Encoding error: {}", e);
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}

#[allow(dead_code)]
pub fn render_image_grid(image_paths: &[String], num_columns: u32) -> Result<()> {
    use ratatui::{
        layout::{Constraint, Direction, Layout, Rect},
        text::{Span, Text},
        widgets::{Block, Borders, Paragraph},
    };

    let picker = create_picker();

    let images: Result<Vec<image::DynamicImage>> = image_paths
        .iter()
        .map(|path| {
            ImageReader::open(path)?
                .decode()
                .context(format!("Failed to decode image: {}", path))
        })
        .collect();

    let images = images?;

    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    enable_raw_mode()?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let mut protocols: Vec<_> = images
        .into_iter()
        .map(|img| picker.new_resize_protocol(img))
        .collect();

    loop {
        terminal.draw(|f| {
            let area = f.area();

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(0), Constraint::Length(1)])
                .split(area);

            let images_area = chunks[0];
            let status_area = chunks[1];

            let num_cols = num_columns as u16;
            let num_rows = ((protocols.len() as u16) + num_cols - 1) / num_cols;

            let cell_width = images_area.width / num_cols;
            let cell_height = images_area.height / num_rows.max(1);

            for (i, protocol) in protocols.iter_mut().enumerate() {
                let row = (i as u16) / num_cols;
                let col = (i as u16) % num_cols;

                let cell_area = Rect {
                    x: images_area.x + col * cell_width,
                    y: images_area.y + row * cell_height,
                    width: cell_width,
                    height: cell_height,
                };

                let widget = StatefulImage::default();
                f.render_stateful_widget(widget, cell_area, protocol);
            }

            let status_text = Span::raw("Press 'q' to quit");
            let status_bar = Paragraph::new(Text::from(status_text))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(status_bar, status_area);
        })?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}

#[allow(dead_code)]
pub fn display_single_image_interactive(image_path: &str) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let picker = create_picker();

    let dyn_img = ImageReader::open(image_path)?
        .decode()
        .context("Failed to decode image")?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let mut image_protocol = picker.new_resize_protocol(dyn_img);

    loop {
        terminal.draw(|f| {
            let area = f.area();
            let widget = StatefulImage::default();
            f.render_stateful_widget(widget, area, &mut image_protocol);
        })?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Enter => break,
                _ => {}
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    Ok(())
}
