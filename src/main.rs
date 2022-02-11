mod lister;

use crate::lister::*;
use crossterm::{
    event::{poll, read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::debug;
use std::{
    collections::HashMap,
    io,
    sync::{mpsc::Receiver, Arc, Mutex},
    time::Duration,
};
use svn_cmd::SvnList;
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, BorderType, Borders, List},
    Terminal,
};

fn main() -> Result<(), CustomError> {
    env_logger::init();
    let list_cache: Cache = Arc::new(Mutex::new(HashMap::new()));
    let cb = svn_data_generator(Arc::clone(&list_cache))?;
    ui(cb)
}

const INITIAL_URL: &str = "https://svn.ali.global/GDK_games/GDK_games/BLS/";

fn ui(data_generator: Arc<DataGenerator>) -> Result<(), CustomError> {
    // start terminal mode
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut custom_lists = CustomLists::from(vec![CustomList::from(INITIAL_URL.to_owned())]);

    let mut custom_state = {
        let (_, custom_list, _) = custom_lists.get_current();
        CustomListState::from(custom_list.unwrap())
    };

    let mut new_data_request: Option<Request> = Some(Request::Forward(INITIAL_URL.to_owned()));
    let mut rx: Option<Receiver<SvnList>> = Some(request_new_data(
        INITIAL_URL.to_owned(),
        Arc::clone(&data_generator),
    ));
    let default_block = Block::default().borders(Borders::ALL);
    loop {
        if poll(Duration::from_millis(200))? {
            if let Event::Key(KeyEvent { code, .. }) = read()? {
                match code {
                    KeyCode::Esc => break,
                    KeyCode::Char('j') | KeyCode::Down => custom_state.inc(),
                    KeyCode::Char('k') | KeyCode::Up => custom_state.dec(),
                    KeyCode::Char('l') | KeyCode::Right => {
                        if new_data_request.is_none() {
                            if let (_, Some(custom_list), _) = custom_lists.get_current() {
                                if let Some(selected) =
                                    custom_list.get_current_selected(&custom_state)
                                {
                                    debug!("requesting new data");
                                    let mut base = custom_list.base_url.clone();
                                    base.push_str(&selected);
                                    base.push('/');
                                    new_data_request = Some(Request::Forward(base.clone()));
                                    rx = Some(request_new_data(
                                        base.to_string(),
                                        Arc::clone(&data_generator),
                                    ))
                                }
                            }
                        }
                    }
                    KeyCode::Char('h') | KeyCode::Left => {
                        if new_data_request.is_none() {
                            if let (_, Some(custom_list), _) = custom_lists.go_back() {
                                custom_state = CustomListState::from(custom_list);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if let Some(Request::Forward(base_url)) = &new_data_request {
            if let Some(rx) = &rx {
                if let Some(new_data) = get_new_data(rx) {
                    debug!("data received");
                    let new_list = CustomList::from((new_data, base_url.to_owned()));
                    custom_lists.add_new_list(new_list);
                    if let (_, Some(list), _) = custom_lists.get_current() {
                        custom_state = CustomListState::from(list);
                    }
                    new_data_request = None;
                }
            }
        }

        terminal.draw(|frame| {
            let vertical_chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints(
                    [
                        Constraint::Percentage(5),
                        Constraint::Percentage(90),
                        Constraint::Percentage(5),
                    ]
                    .as_ref(),
                )
                .split(frame.size());
            frame.render_widget(default_block.clone().title("commands"), vertical_chunks[0]);
            frame.render_widget(default_block.clone().title("messages"), vertical_chunks[2]);
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
                .split(vertical_chunks[1]);

            let (prev, curr, next) = custom_lists.get_current();

            if let Some(prev) = prev {
                frame.render_widget(
                    List::new(prev.get_list_items()).block(default_block.clone().title("previous")),
                    chunks[0],
                );
            } else {
                frame.render_widget(default_block.clone().title("previous"), chunks[0]);
            }

            if let Some(next) = next {
                frame.render_widget(
                    List::new(next.get_list_items()).block(default_block.clone().title("next")),
                    chunks[2],
                );
            } else {
                frame.render_widget(default_block.clone().title("next"), chunks[2]);
            }

            let list = List::new(curr.unwrap().get_list_items())
                .block(
                    default_block
                        .clone()
                        .title("middle")
                        .border_style(Style::default().fg(Color::LightCyan))
                        .border_type(BorderType::Rounded),
                )
                .highlight_style(Style::default().add_modifier(Modifier::BOLD))
                .highlight_symbol(">>");
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
