use crossterm::{
    event::{read, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::info;
use std::{
    io,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    thread,
    time::Duration,
};
use svn_cmd::{Credentials, SvnCmd, SvnError};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, List, ListItem, ListState},
    Terminal,
};

fn main() -> Result<(), CustomError> {
    env_logger::init();
    let mut clist = svn_data()?;
    ui(&mut clist)
}

fn svn_data() -> Result<CustomList, CustomError> {
    let cmd = SvnCmd::new(
        Credentials {
            username: "svc-p-blsrobo".to_owned(),
            password: "Comewel@12345".to_owned(),
        },
        None,
    )?;

    let mut list = CustomList::from(vec![
        "https://svn.ali.global/GDK_games/GDK_games/BLS/HHR".to_owned()
    ]);
    list.set_request_handle(move |target, tx| {
        info!("svn info requested!");
        let slist = cmd.list(&target, false)?;
        let list_vec: Vec<String> = slist.iter().map(|i| i.name.clone()).collect();
        info!("{list_vec:?}");
        tx.send(list_vec).unwrap();
        info!("svn info responded!");
        Ok(())
    });

    Ok(list)
}

fn ui(custom_list: &mut CustomList) -> Result<(), CustomError> {
    // start terminal mode
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

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
    })?;

    custom_list.selected();

    loop {
        match read()? {
            Event::Key(KeyEvent {
                code: KeyCode::Esc, ..
            }) => {
                break;
            }
            _ => {}
        }
        if let Some(hndl) = &custom_list.req_hndl {
            if hndl.requested {
                if let Some(rx) = &hndl.recv {
                    if let Ok(new_data) = rx.try_recv() {
                        custom_list.replace_items(new_data);
                        info!("{:?}", custom_list.items);
                    }
                }
            }
        }
        let lst: Vec<ListItem> = custom_list
            .items
            .iter()
            .map(|i| ListItem::new(i.as_str()))
            .collect();
        // terminal.draw(|frame| {
        //     frame.render_stateful_widget(List::new(lst), frame.size(), &mut custom_list.state);
        // })?;
    }

    //thread::sleep(Duration::from_secs(5));

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

struct RequestHandle {
    hndl: Arc<dyn Fn(String, Sender<Vec<String>>) -> Result<(), CustomError> + Sync + Send>,
    recv: Option<Receiver<Vec<String>>>,
    requested: bool,
}

struct CustomList {
    items: Vec<String>,
    state: ListState,
    req_hndl: Option<RequestHandle>,
}

impl CustomList {
    fn set_request_handle<F: 'static>(&mut self, hndl: F)
    where
        F: Fn(String, Sender<Vec<String>>) -> Result<(), CustomError> + Sync + Send,
    {
        self.req_hndl.replace(RequestHandle {
            hndl: Arc::new(hndl),
            recv: None,
            requested: false,
        });
    }

    fn add_items(&mut self, items: &[String]) {
        self.items.extend_from_slice(items);
    }

    fn replace_items(&mut self, items: Vec<String>) {
        if let Some(hndl) = &mut self.req_hndl {
            hndl.requested = false;
        }
        self.items = items;
    }

    fn next(&mut self) {
        if let Some(cur) = self.state.selected() {
            if self.items.len() + 1 > cur {
                self.state.select(Some(cur + 1));
            } else {
                self.state.select(Some(0));
            }
        }
    }

    fn prev(&mut self) {
        if let Some(cur) = self.state.selected() {
            if cur > 1 {
                self.state.select(Some(cur - 1));
            } else {
                self.state.select(Some(self.items.len() - 1))
            }
        }
    }

    fn selected(&mut self) {
        let (tx, rx) = channel();
        let req_data = self
            .items
            .get(self.state.selected().unwrap())
            .cloned()
            .unwrap();
        if let Some(hndl) = &mut self.req_hndl {
            hndl.recv = Some(rx);
            hndl.requested = true;
            let hndl = Arc::clone(&hndl.hndl);
            thread::spawn(move || {
                (hndl)(req_data, tx).unwrap();
            });
        }
        // let new_data = rx.recv().unwrap();
        // self.replace_items(new_data);
    }
}

impl From<&[String]> for CustomList {
    fn from(items: &[String]) -> Self {
        CustomList::from(items.to_vec())
    }
}

impl From<Vec<String>> for CustomList {
    fn from(items: Vec<String>) -> Self {
        let mut v = Self {
            items,
            ..Default::default()
        };
        if !v.items.is_empty() {
            v.state.select(Some(0));
        }
        v
    }
}

impl Default for CustomList {
    fn default() -> Self {
        Self {
            items: vec![],
            state: Default::default(),
            req_hndl: None,
        }
    }
}

#[derive(Debug)]
enum CustomError {
    Io(io::Error),
    Svn(SvnError),
}

impl From<io::Error> for CustomError {
    fn from(e: io::Error) -> Self {
        CustomError::Io(e)
    }
}

impl From<SvnError> for CustomError {
    fn from(e: SvnError) -> Self {
        CustomError::Svn(e)
    }
}
