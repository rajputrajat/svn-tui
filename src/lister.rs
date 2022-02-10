use std::{
    io,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
    thread,
};
use svn_cmd::{Credentials, SvnCmd, SvnError};
use tui::widgets::{ListItem, ListState};

pub(crate) trait ListOps<T = String> {
    fn set_request_handle<F: 'static>(&mut self, hndl: F)
    where
        F: Fn(String, Sender<Vec<String>>) -> Result<(), CustomError> + Sync + Send;
    fn replace_items(&mut self, items: Vec<T>);
    fn next(&mut self);
    fn prev(&mut self);
    fn selected(&mut self);
    fn get_list_items(&mut self) -> Option<Vec<ListItem>>;
    fn get_state_mut_ref(&mut self) -> &mut ListState;
}

pub(crate) fn svn_data() -> Result<CustomList, CustomError> {
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
        let slist = cmd.list(&target, false)?;
        let list_vec: Vec<String> = slist.iter().map(|i| i.name.clone()).collect();
        tx.send(list_vec).unwrap();
        Ok(())
    });

    Ok(list)
}

struct RequestHandle {
    hndl: Arc<dyn Fn(String, Sender<Vec<String>>) -> Result<(), CustomError> + Sync + Send>,
    recv: Option<Receiver<Vec<String>>>,
    requested: bool,
}

pub(crate) struct CustomList {
    items: Vec<String>,
    state: ListState,
    req_hndl: Option<RequestHandle>,
}

impl ListOps for CustomList {
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

    fn get_list_items(&mut self) -> Option<Vec<ListItem>> {
        if let Some(hndl) = &self.req_hndl {
            if hndl.requested {
                if let Some(rx) = &hndl.recv {
                    if let Ok(new_data) = rx.try_recv() {
                        self.replace_items(new_data);
                        let list_items = self
                            .items
                            .iter()
                            .map(|i| ListItem::new(i.as_str()))
                            .collect();
                        return Some(list_items);
                    }
                }
            }
        }
        None
    }

    fn get_state_mut_ref(&mut self) -> &mut ListState {
        &mut self.state
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
pub(crate) enum CustomError {
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
