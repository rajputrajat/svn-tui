mod lister;

use crate::lister::*;
use crossterm::{
    event::{poll, read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{io, time::Duration};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, List, ListItem},
    Terminal,
};

fn main() -> Result<(), CustomError> {
    env_logger::init();
    let clist = svn_data()?;
    ui(clist)
}

fn ui(custom_list: impl ListOps) -> Result<(), CustomError> {
    // start terminal mode
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut custom_list = custom_list;
    custom_list.selected();

    loop {
        if poll(Duration::from_millis(200))? {
            match read()? {
                Event::Key(KeyEvent { code, .. }) => {
                    match code {
                        KeyCode::Esc => break,
                        KeyCode::Char('j') => custom_list.next(),
                        KeyCode::Char('k') => custom_list.prev(),
                        KeyCode::Char('l') => custom_list.selected(),
                        KeyCode::Char('h') => {} /*go back*/
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        let mut list_items: Vec<ListItem> = vec![];
        if let Some(lstitems) = custom_list.get_list_items() {
            list_items = lstitems;
        }

        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .margin(0)
                .constraints(
                    [
                        Constraint::Percentage(10),
                        Constraint::Percentage(80),
                        Constraint::Percentage(10),
                    ]
                    .as_ref(),
                )
                .split(frame.size());

            frame.render_widget(
                Block::default().title("left").borders(Borders::ALL),
                chunks[0],
            );
            frame.render_widget(
                List::new([
                    ListItem::new("hey"),
                    ListItem::new("what"),
                    ListItem::new("are"),
                    ListItem::new("you"),
                    ListItem::new("doing"),
                ])
                .block(
                    Block::default()
                        .title("middle")
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(Color::LightCyan))
                        .border_type(BorderType::Rounded),
                ),
                chunks[1],
            );
            frame.render_widget(
                Block::default().title("right").borders(Borders::ALL),
                chunks[2],
            );
            frame.render_stateful_widget(
                List::new(list_items),
                chunks[1],
                &mut custom_list.get_state_mut_ref(),
            );
        })?;
    }

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
