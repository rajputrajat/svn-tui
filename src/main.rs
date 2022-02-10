mod lister;

use crate::lister::*;
use crossterm::{
    event::{poll, read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::{debug, info};
use std::{
    io,
    sync::{mpsc::Receiver, Arc, Mutex},
    time::Duration,
};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, List},
    Terminal,
};

fn main() -> Result<(), CustomError> {
    env_logger::init();
    let cb = svn_data_generator()?;
    ui(cb)
}

fn ui(data_generator: Arc<DataGenerator>) -> Result<(), CustomError> {
    // start terminal mode
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let base_svn_url = Arc::new(Mutex::new(
        "https://svn.ali.global/GDK_games/GDK_games/BLS/".to_owned(),
    ));

    let mut custom_list = CustomList::from(vec!["HHR".to_owned()]);
    let mut custom_state = CustomListState::from(&custom_list);
    let mut data_requested = false;
    let mut rx: Option<Receiver<Vec<String>>> = None;

    loop {
        if poll(Duration::from_millis(200))? {
            if let Event::Key(KeyEvent { code, .. }) = read()? {
                match code {
                    KeyCode::Esc => break,
                    KeyCode::Char('j') => custom_state.inc(),
                    KeyCode::Char('k') => custom_state.dec(),
                    KeyCode::Char('l') => {
                        if !data_requested {
                            if let Some(selected) = custom_list.get_current_selected(&custom_state)
                            {
                                debug!("requesting new data");
                                let mut base = base_svn_url.lock().unwrap();
                                base.push_str(&selected);
                                base.push('/');
                                data_requested = true;
                                rx = Some(request_new_data(
                                    base.to_string(),
                                    Arc::clone(&data_generator),
                                ))
                            }
                        }
                    }
                    KeyCode::Char('h') => {
                        if !data_requested {
                            if let Some(_selected) = custom_list.get_current_selected(&custom_state)
                            {
                                debug!("requesting new data");
                                let mut base = base_svn_url.lock().unwrap();
                                let splitted: Vec<&str> = base.split('/').collect();
                                let splitted = &splitted[..splitted.len() - 2];
                                let new_str = splitted.iter().fold(String::new(), |mut acc, s| {
                                    acc.push_str(s);
                                    acc.push('/');
                                    acc
                                });
                                *base = new_str;
                                data_requested = true;
                                rx = Some(request_new_data(
                                    base.to_string(),
                                    Arc::clone(&data_generator),
                                ))
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if let Some(rx) = &rx {
            if let Some(new_data) = get_new_data(rx) {
                debug!("data received");
                custom_list = CustomList::from(new_data);
                data_requested = false;
            }
        }

        terminal.draw(|frame| {
            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .margin(0)
                .constraints(
                    [
                        Constraint::Percentage(20),
                        Constraint::Percentage(50),
                        Constraint::Percentage(30),
                    ]
                    .as_ref(),
                )
                .split(frame.size());

            frame.render_widget(
                Block::default().title("left").borders(Borders::ALL),
                chunks[0],
            );

            frame.render_widget(
                Block::default().title("right").borders(Borders::ALL),
                chunks[2],
            );

            let list = List::new(custom_list.get_list_items()).block(
                Block::default()
                    .title("middle")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::LightCyan))
                    .border_type(BorderType::Rounded),
            );
            frame.render_stateful_widget(list, chunks[1], &mut custom_state.state);
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
