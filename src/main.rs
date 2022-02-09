use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::info;
use std::{io, thread, time::Duration};
use svn_cmd::{Credentials, SvnCmd, SvnError};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, BorderType, Borders, List, ListItem},
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
