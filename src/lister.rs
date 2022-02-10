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
    fn next(&mut self);
    fn prev(&mut self);
    fn get_list_items(&self) -> Vec<ListItem>;
    fn get_current_selected(&self) -> Option<String>;
    fn get_state_mut_ref(&mut self) -> &mut ListState;
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

pub(crate) struct CustomList {
    items: Vec<String>,
    state: ListState,
}

impl ListOps for CustomList {
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

    fn get_list_items(&self) -> Vec<ListItem> {
        self.items
            .iter()
            .map(|i| ListItem::new(i.as_ref()))
            .collect()
    }

    fn get_current_selected(&self) -> Option<String> {
        if let Some(selected) = self.state.selected() {
            return self.items.get(selected).cloned();
        } else {
            None
        }
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
        let v = Self {
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
