mod data_handler;
mod lister;

use crate::{data_handler::*, lister::*};
use crossterm::{
    event::{poll, read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::debug;
use std::{
    io::{self, Stdout},
    sync::{mpsc, Arc, Mutex},
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
    ui()
}

const INITIAL_URL: &str = "https://svn.ali.global/GDK_games/GDK_games/BLS/";
const PREV: &str = " <-- ";
const PPREV: &str = " <---- ";
const MIDDLE: &str = "SVN list";
const INFO: &str = "info";
const MESSAGES: &str = "messages";

enum ScrollReq {
    Up,
    Down,
}

#[derive(Default)]
struct ViewScroller {
    size: u16,
    current: u16,
}

impl From<u16> for ViewScroller {
    fn from(size: u16) -> Self {
        Self {
            size,
            ..Default::default()
        }
    }
}

impl ViewScroller {
    fn handle(&mut self, height: u16, req: ScrollReq) -> u16 {
        match req {
            ScrollReq::Up => {
                if self.current >= height {
                    self.current -= height;
                }
            }
            ScrollReq::Down => {
                if self.current + height < self.size {
                    self.current += height;
                }
            }
        }
        self.current
    }

    fn cur(&self) -> u16 {
        self.current
    }
}

fn ui() -> Result<(), CustomError> {
    let base_url = if let Ok(info_entry) = svn_helper::info(&svn_helper::new()) {
        let mut url = info_entry.entry.url;
        url.push('/');
        url
    } else {
        INITIAL_URL.to_owned()
    };
    let custom_lists = Arc::new(Mutex::new(CustomLists::from(vec![CustomList::from(
        base_url.clone(),
    )])));
    let mut term = Terminal_::create()?;
    let custom_state = Arc::new(Mutex::new({
        let custom_lists = Arc::clone(&custom_lists);
        let locked_lists = custom_lists.lock().unwrap();
        let CustomListsToDisplay { cur, .. } = locked_lists.get_current();
        CustomListState::from(cur.ok_or_else(|| CustomError::NoDataToList)?)
    }));
    let mut new_data_request: Option<DataRequest> =
        Some(DataRequest::List(TargetUrl(base_url.clone())));
    let message = Arc::new(Mutex::new(format!("requesting svn list for '{base_url}'")));
    let default_block = Block::default().borders(Borders::ALL);
    let svn_info_list = Arc::new(Mutex::new(vec![]));
    let update_svn_info_str = |entry: &ListEntry| {
        let mut sis = svn_info_list.lock().unwrap();
        *sis = vec![
            ListItem::new(format!("      url: {}", entry.name)),
            ListItem::new(format!(" revision: {}", entry.commit.revision)),
            ListItem::new(format!("   author: {}", entry.commit.author)),
            ListItem::new(format!("     date: {}", entry.commit.date)),
        ];
    };
    let data_handler = Arc::new(DataHandler::default());
    let (error_tx, error_rx) = mpsc::channel::<CustomError>();
    let text_view = Arc::new(Mutex::new(Option::<(Paragraph, String)>::None));
    let text_view_scroller = Arc::new(Mutex::new(Option::<ViewScroller>::None));
    let mut text_view_scroll_req: Option<ScrollReq>;

    loop {
        text_view_scroll_req = None;
        if poll(Duration::from_millis(200))? {
            if let Event::Key(KeyEvent { code, .. }) = read()? {
                svn_info_list.lock().unwrap().clear();
                match code {
                    KeyCode::Esc => break,
                    KeyCode::PageUp | KeyCode::Char('u') => {
                        text_view_scroll_req = Some(ScrollReq::Up)
                    }
                    KeyCode::PageDown | KeyCode::Char('d') => {
                        text_view_scroll_req = Some(ScrollReq::Down)
                    }
                    KeyCode::Char('j') | KeyCode::Down => custom_state.lock().unwrap().inc(),
                    KeyCode::Char('k') | KeyCode::Up => custom_state.lock().unwrap().dec(),
                    KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                        if new_data_request.is_none() {
                            if let CustomListsToDisplay {
                                cur: Some(custom_list),
                                ..
                            } = custom_lists.lock().unwrap().get_current()
                            {
                                if let Some(selected) =
                                    custom_list.get_current_selected(Arc::clone(&custom_state))
                                {
                                    let mut base = custom_list.base_url.clone();
                                    base.push_str(&selected.name);
                                    base.push('/');
                                    if selected.kind == PathType::Dir {
                                        debug!("requesting new data");
                                        *message.lock().unwrap() =
                                            format!("requesting svn list for '{base}'");
                                        new_data_request =
                                            Some(DataRequest::List(TargetUrl(base.clone())));
                                    } else {
                                        let name = selected.name;
                                        debug!("file is not listable, so ignore: {name}");
                                        *message.lock().unwrap() =
                                            format!("'opening file: '{name}'");
                                        new_data_request = Some(DataRequest::Text(TargetUrl(base)));
                                    }
                                }
                            }
                        }
                    }
                    KeyCode::Char('h') | KeyCode::Left => {
                        if new_data_request.is_none() {
                            if let CustomListsToDisplay {
                                cur: Some(custom_list),
                                ..
                            } = custom_lists.lock().unwrap().go_back()
                            {
                                *custom_state.lock().unwrap() = CustomListState::from(custom_list);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if let Some(req) = new_data_request {
            let dh = Arc::clone(&data_handler);
            let custom_lists = Arc::clone(&custom_lists);
            let custom_state = Arc::clone(&custom_state);
            let message = Arc::clone(&message);
            let err_tx = error_tx.clone();
            let text_view = Arc::clone(&text_view);
            let scroller = Arc::clone(&text_view_scroller);
            dh.request(req.clone(), ViewId::MainList, move |res_resp| {
                debug!("data received");
                match res_resp {
                    Ok(v) => match v {
                        DataResponse::List(svn_list) => {
                            *message.lock().unwrap() =
                                format!("displaying new svn list from '{req:?}'");
                            let new_list = CustomList::from((
                                svn_list.clone(),
                                TargetUrl::from(req.clone()).into(),
                            ));
                            custom_lists.lock().unwrap().add_new_list(new_list);
                            if let CustomListsToDisplay {
                                cur: Some(list), ..
                            } = custom_lists.lock().unwrap().get_current()
                            {
                                *custom_state.lock().unwrap() = CustomListState::from(list);
                            }
                        }
                        DataResponse::Log(log) => {}
                        DataResponse::Info(i) => {}
                        DataResponse::Text(t) => {
                            let mut text = Vec::<Spans>::new();
                            for line in t.lines() {
                                let spans = Spans::from(vec![Span::raw(line.to_owned())]);
                                text.push(spans);
                            }
                            *scroller.lock().unwrap() = Some(ViewScroller::from(text.len() as u16));
                            let para = Paragraph::new(text);
                            *text_view.lock().unwrap() =
                                Some((para, TargetUrl::from(req.clone()).into()));
                        }
                    },
                    Err(e) => err_tx.send(e).unwrap(),
                }
            });
            debug!("out here");
            new_data_request = None;
        }
        if let Ok(e) = error_rx.try_recv() {
            return Err(e);
        }

        if let CustomListsToDisplay {
            cur: Some(custom_list),
            ..
        } = custom_lists.lock().unwrap().get_current()
        {
            if let Some(selected) = custom_list.get_current_selected(Arc::clone(&custom_state)) {
                update_svn_info_str(&selected);
            }
        }

        term.get_int().draw(|frame| {
            let vertical_chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints(
                    [
                        Constraint::Percentage(7),
                        Constraint::Percentage(83),
                        Constraint::Percentage(10),
                    ]
                    .as_ref(),
                )
                .split(frame.size());

            let text_str = {
                let locked_msg = message.lock().unwrap();
                locked_msg.to_string()
            };
            let text = {
                vec![Spans::from(Span::styled(
                    &text_str,
                    Style::default().fg(Color::LightMagenta),
                ))]
            };
            frame.render_widget(
                Paragraph::new(text).block(default_block.clone().title(MESSAGES)),
                vertical_chunks[0],
            );

            let chunks = Layout::default()
                .direction(Direction::Horizontal)
                .margin(0)
                .constraints(
                    [
                        Constraint::Percentage(15),
                        Constraint::Percentage(15),
                        Constraint::Percentage(20),
                        Constraint::Percentage(50),
                    ]
                    .as_ref(),
                )
                .split(vertical_chunks[1]);

            let CustomListsToDisplay { cur, prev, pprev } = {
                let locked_lists = custom_lists.lock().unwrap();
                locked_lists.get_current()
            };

            if let Some(pprev) = pprev {
                frame.render_widget(
                    List::new(pprev.get_list_items()).block(default_block.clone().title(PPREV)),
                    chunks[0],
                );
            } else {
                frame.render_widget(default_block.clone().title(PPREV), chunks[0]);
            }

            if let Some(prev) = prev {
                frame.render_widget(
                    List::new(prev.get_list_items()).block(default_block.clone().title("")),
                    chunks[1],
                );
            } else {
                frame.render_widget(default_block.clone().title(""), chunks[3]);
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
                vertical_chunks[2],
            );
            frame.render_widget(default_block.clone().title(PREV), chunks[1]);

            if let Some(curr) = cur {
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
                frame.render_stateful_widget(
                    list,
                    chunks[2],
                    &mut custom_state.lock().unwrap().state,
                );
            } else {
                frame.render_widget(default_block.clone().title(MIDDLE), chunks[2]);
            }

            let scroll = if let Some(req) = text_view_scroll_req {
                let rect = chunks[3];
                if let Some(scroller) = &mut *text_view_scroller.lock().unwrap() {
                    scroller.handle(rect.height, req)
                } else {
                    0
                }
            } else {
                if let Some(scroller) = &*text_view_scroller.lock().unwrap() {
                    scroller.cur()
                } else {
                    0
                }
            };
            if let Some((para, url)) = &*text_view.lock().unwrap() {
                frame.render_widget(default_block.clone(), chunks[3]);
                frame.render_widget(
                    para.clone()
                        .block(
                            default_block
                                .clone()
                                .title(url.as_ref())
                                .border_style(Style::default().fg(Color::LightMagenta)),
                        )
                        .scroll((scroll, 0)),
                    chunks[3],
                );
            } else {
                frame.render_widget(default_block.clone(), chunks[3]);
            }
        })?;
    }

    Ok(())
}
