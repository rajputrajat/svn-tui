use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::info;
use std::{
    io,
    sync::mpsc::{Receiver, Sender},
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
    svn_data()?;
    ui()
}

fn svn_data() -> Result<(), CustomError> {
    let cmd = SvnCmd::new(
        Credentials {
            username: "svc-p-blsrobo".to_owned(),
            password: "Comewel@12345".to_owned(),
        },
        None,
    )?;
    let list = cmd.list("https://svn.ali.global/GDK_games/GDK_games/BLS/HHR", false)?;
    info!("{list:?}");
    Ok(())
}

fn ui() -> Result<(), CustomError> {
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

    thread::sleep(Duration::from_secs(5));

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

struct RequesterHandle<T> {
    handle: Box<dyn Fn(&T)>,
    recv_ch: Receiver<Vec<T>>,
}

struct CustomList<T> {
    items: Vec<T>,
    state: ListState,
    req_hndl: Option<RequesterHandle<T>>,
}

impl<T: Clone> CustomList<T> {
    fn set_request_handle(&mut self, hndl: RequesterHandle<T>) {
        self.req_hndl.replace(hndl);
    }

    fn add_items(&mut self, items: &[T]) {
        self.items.extend_from_slice(items);
    }

    fn replace_items(&mut self, items: Vec<T>) {
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

    fn selected(&self) {
        if let Some(hndl) = &self.req_hndl {
            (hndl.handle)(self.items.get(self.state.selected().unwrap()).unwrap());
        }
    }
}

impl<T: Clone> From<&[T]> for CustomList<T> {
    fn from(items: &[T]) -> Self {
        CustomList::from(items.to_vec())
    }
}

impl<T> From<Vec<T>> for CustomList<T> {
    fn from(items: Vec<T>) -> Self {
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

impl<T> Default for CustomList<T> {
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
