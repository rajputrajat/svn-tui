use log::debug;
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

pub(crate) type DataGenerator =
    dyn Fn(String, Sender<Vec<String>>) -> Result<(), CustomError> + Sync + Send;

pub(crate) trait ListOps {
    fn len(&self) -> usize;
    fn get_list_items(&self) -> Vec<ListItem>;
    fn get_current_selected(&self, state: &impl ListStateOps) -> Option<String>;
}

pub(crate) trait ListStateOps {
    fn get(&self) -> Option<usize>;
    fn inc(&mut self);
    fn dec(&mut self);
}

pub(crate) fn svn_data_generator() -> Result<Arc<DataGenerator>, CustomError> {
    let cmd = SvnCmd::new(
        Credentials {
            username: "svc-p-blsrobo".to_owned(),
            password: "Comewel@12345".to_owned(),
        },
        None,
    )?;

    let generator = move |target: String, tx: Sender<Vec<String>>| {
        debug!("request for '{target}'");
        let slist = cmd.list(&target, false)?;
        let list_vec: Vec<String> = slist.iter().map(|i| i.name.clone()).collect();
        debug!("data: '{list_vec:?}'");
        tx.send(list_vec).unwrap();
        debug!("info sent");
        Ok(())
    };

    Ok(Arc::new(generator))
}

pub(crate) fn request_new_data(selected: String, cb: Arc<DataGenerator>) -> Receiver<Vec<String>> {
    let (tx, rx) = channel::<Vec<String>>();
    thread::spawn(move || {
        (cb)(selected, tx).unwrap();
    });
    rx
}

pub(crate) fn get_new_data<T>(rx: &Receiver<Vec<T>>) -> Option<Vec<T>> {
    rx.try_recv().ok()
}

#[derive(Default)]
pub(crate) struct CustomList {
    items: Vec<String>,
}

#[derive(Default)]
pub(crate) struct CustomListState {
    pub(crate) state: ListState,
    list_size: usize,
}

impl ListStateOps for CustomListState {
    fn get(&self) -> Option<usize> {
        self.state.selected()
    }

    fn inc(&mut self) {
        if let Some(selected) = self.state.selected() {
            if self.list_size - 1 > selected {
                self.state.select(Some(selected + 1));
            } else {
                self.state.select(Some(0));
            }
        }
    }

    fn dec(&mut self) {
        if let Some(selected) = self.state.selected() {
            if selected > 0 {
                self.state.select(Some(selected - 1));
            } else {
                self.state.select(Some(self.list_size - 1));
            }
        }
    }
}

impl ListOps for CustomList {
    fn len(&self) -> usize {
        self.items.len()
    }

    fn get_list_items(&self) -> Vec<ListItem> {
        self.items
            .iter()
            .map(|i| ListItem::new(i.as_ref()))
            .collect()
    }

    fn get_current_selected(&self, state: &impl ListStateOps) -> Option<String> {
        if let Some(selected) = state.get() {
            return self.items.get(selected).cloned();
        } else {
            None
        }
    }
}

impl From<&CustomList> for CustomListState {
    fn from(list: &CustomList) -> Self {
        let mut state = ListState::default();
        let list_size = list.len();
        if list.len() > 0 {
            state.select(Some(0));
        }
        CustomListState { state, list_size }
    }
}

impl From<&[String]> for CustomList {
    fn from(items: &[String]) -> Self {
        CustomList::from(items.to_vec())
    }
}

impl From<Vec<String>> for CustomList {
    fn from(items: Vec<String>) -> Self {
        Self { items }
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
