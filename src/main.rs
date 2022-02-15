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
    io::{self, Stdout},
    sync::{mpsc::Receiver, Arc, Mutex},
    time::Duration,
};
use svn_cmd::{ListEntry, PathType};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph},
    Terminal,
};

struct Terminal_ {
    term: Terminal<CrosstermBackend<Stdout>>,
}

impl Terminal_ {
    fn create() -> Result<Self, CustomError> {
        // start terminal mode
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        Ok(Self {
            term: Terminal::new(backend)?,
        })
    }

    fn get_int(&mut self) -> &mut Terminal<CrosstermBackend<Stdout>> {
        &mut self.term
    }
}

impl Drop for Terminal_ {
    fn drop(&mut self) {
        // restore terminal
        disable_raw_mode().unwrap();
        execute!(
            self.term.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .unwrap();
        self.term.show_cursor().unwrap();
    }
}

fn main() -> Result<(), CustomError> {
    env_logger::init();
    let list_cache: Cache = Arc::new(Mutex::new(HashMap::new()));
    let cb = svn_data_generator(Arc::clone(&list_cache));
    ui(cb)
}

const INITIAL_URL: &str = "https://svn.ali.global/GDK_games/GDK_games/BLS/";
const PREV: &str = " <--- ";
const NEXT: &str = " ---> ";
const MIDDLE: &str = "SVN list";
const INFO: &str = "info";

fn ui(data_generator: Arc<DataGenerator>) -> Result<(), CustomError> {
    let mut custom_lists = CustomLists::from(vec![CustomList::from(INITIAL_URL.to_owned())]);

    let mut term = Terminal_::create()?;

    let mut custom_state = {
        let (_, custom_list, _) = custom_lists.get_current();
        CustomListState::from(custom_list.ok_or_else(|| CustomError::NoDataToList)?)
    };

    let mut new_data_request: Option<Request> = Some(Request::Forward(INITIAL_URL.to_owned()));
    let mut rx: Option<Receiver<ResultSvnList>> = Some(request_new_data(
        INITIAL_URL.to_owned(),
        Arc::clone(&data_generator),
    ));
    let mut message = format!("requesting svn list for '{}'", INITIAL_URL);
    let default_block = Block::default().borders(Borders::ALL);
    let svn_info_list = Arc::new(Mutex::new(vec![]));
    let update_svn_info_str = |entry: &ListEntry| {
        let mut sis = svn_info_list.lock().unwrap();
        *sis = vec![
            ListItem::new(format!("url: {}", entry.name)),
            ListItem::new(format!("revision: {}", entry.commit.revision)),
            ListItem::new(format!("author: {}", entry.commit.author)),
            ListItem::new(format!("date: {}", entry.commit.date)),
        ];
    };
    loop {
        if poll(Duration::from_millis(200))? {
            if let Event::Key(KeyEvent { code, .. }) = read()? {
                svn_info_list.lock().unwrap().clear();
                match code {
                    KeyCode::Esc => break,
                    KeyCode::Char('j') | KeyCode::Down => custom_state.inc(),
                    KeyCode::Char('k') | KeyCode::Up => custom_state.dec(),
                    KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                        if new_data_request.is_none() {
                            if let (_, Some(custom_list), _) = custom_lists.get_current() {
                                if let Some(selected) =
                                    custom_list.get_current_selected(&custom_state)
                                {
                                    if selected.kind == PathType::Dir {
                                        debug!("requesting new data");
                                        let mut base = custom_list.base_url.clone();
                                        base.push_str(&selected.name);
                                        base.push('/');
                                        message = format!("requesting svn list for '{}'", base);
                                        new_data_request = Some(Request::Forward(base.clone()));
                                        rx = Some(request_new_data(
                                            base.to_string(),
                                            Arc::clone(&data_generator),
                                        ))
                                    } else {
                                        debug!(
                                            "file is not listable, so ignore: {}",
                                            selected.name
                                        );
                                        message = format!(
                                            "'{}' is a file. can't be listed",
                                            selected.name
                                        );
                                    }
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
                    message = format!("displaying new svn list from '{}'", base_url);
                    let new_list = CustomList::from((new_data?, base_url.to_owned()));
                    custom_lists.add_new_list(new_list);
                    if let (_, Some(list), _) = custom_lists.get_current() {
                        custom_state = CustomListState::from(list);
                    }
                    new_data_request = None;
                }
            }
        }
        if let (_, Some(custom_list), _) = custom_lists.get_current() {
            if let Some(selected) = custom_list.get_current_selected(&custom_state) {
                update_svn_info_str(&selected);
            }
        }

        term.get_int().draw(|frame| {
            let vertical_chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints(
                    [
                        Constraint::Percentage(5),
                        Constraint::Percentage(88),
                        Constraint::Percentage(7),
                    ]
                    .as_ref(),
                )
                .split(frame.size());
            frame.render_widget(default_block.clone().title("commands"), vertical_chunks[0]);

            let text = vec![Spans::from(Span::styled(
                &message,
                Style::default().fg(Color::LightMagenta),
            ))];
            frame.render_widget(
                Paragraph::new(text).block(default_block.clone().title("messages")),
                vertical_chunks[2],
            );

            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .margin(0)
                .constraints(
                    [
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                        Constraint::Percentage(25),
                    ]
                    .as_ref(),
                )
                .split(vertical_chunks[1]);

            let (prev, curr, next) = custom_lists.get_current();

            if let Some(prev) = prev {
                frame.render_widget(
                    List::new(prev.get_list_items()).block(default_block.clone().title(PREV)),
                    chunks[0],
                );
            } else {
                frame.render_widget(default_block.clone().title(PREV), chunks[0]);
            }

            if let Some(next) = next {
                frame.render_widget(
                    List::new(next.get_list_items()).block(default_block.clone().title(NEXT)),
                    chunks[3],
                );
            } else {
                frame.render_widget(default_block.clone().title(NEXT), chunks[3]);
            }

            let list = {
                let locked = svn_info_list.lock().unwrap();
                List::new(locked.clone())
            };
            frame.render_widget(
                list.block(
                    default_block
                        .clone()
                        .title(INFO)
                        .border_style(Style::default().fg(Color::LightCyan))
                        .border_type(BorderType::Thick),
                ),
                chunks[2],
            );

            if let Some(curr) = curr {
                let list = List::new(curr.get_list_items())
                    .block(
                        default_block
                            .clone()
                            .title(MIDDLE)
                            .border_style(Style::default().fg(Color::LightCyan))
                            .border_type(BorderType::Thick),
                    )
                    .highlight_style(
                        Style::default()
                            .add_modifier(Modifier::BOLD)
                            .fg(Color::LightYellow),
                    )
                    .style(Style::default().fg(Color::Blue))
                    .highlight_symbol(">>");
                frame.render_stateful_widget(list, chunks[1], &mut custom_state.state);
            } else {
                frame.render_widget(default_block.clone().title(MIDDLE), chunks[1]);
            }
        })?;
    }

    Ok(())
}
