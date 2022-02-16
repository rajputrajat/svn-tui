use log::debug;
use std::{
    collections::HashMap,
    io,
    sync::{
        mpsc::{channel, Receiver, Sender},
        {Arc, Mutex},
    },
    thread,
    time::{Duration, SystemTime, SystemTimeError},
};
use svn_cmd::{Credentials, ListEntry, SvnCmd, SvnError, SvnList};
use tui::widgets::{ListItem, ListState};

pub(crate) const MAX_VALIDITY_OF_CACHED_LIST: Duration = Duration::from_secs(15 * 60);

pub(crate) type ResultSvnList = Result<SvnList, CustomError>;
pub(crate) type ListFetcher = dyn Fn(String, Sender<ResultSvnList>) + Sync + Send;

pub(crate) trait ListOps {
    fn len(&self) -> usize;
    fn get_list_items(&self) -> Vec<ListItem>;
    fn get_current_selected(&self, state: &impl ListStateOps) -> Option<ListEntry>;
}

pub(crate) trait ListStateOps {
    fn get(&self) -> Option<usize>;
    fn inc(&mut self);
    fn dec(&mut self);
}

pub(crate) type Cache = Arc<Mutex<HashMap<String, (SvnList, SystemTime)>>>;

pub(crate) mod svn_helper {
    use super::*;
    use svn_cmd::SvnInfo;

    pub(crate) fn new() -> SvnCmd {
        let cmd = SvnCmd::new(
            Credentials {
                username: "svc-p-blsrobo".to_owned(),
                password: "Comewel@12345".to_owned(),
            },
            None,
        );
        cmd
    }

    pub(crate) fn info(cmd: &SvnCmd) -> Result<SvnInfo, CustomError> {
        let info = cmd.info(".")?;
        Ok(info)
    }

    pub(crate) fn list_fetcher(cmd: SvnCmd, cache: Cache) -> Arc<ListFetcher> {
        let fetcher = move |target: String, tx: Sender<ResultSvnList>| {
            debug!("request for '{target}'");
            let get_svnlist = || {
                let mut svn_list: Option<SvnList> = None;
                if let Some((cached_list, system_time)) = cache.lock().unwrap().get(&target) {
                    if SystemTime::now().duration_since(*system_time)? < MAX_VALIDITY_OF_CACHED_LIST
                    {
                        svn_list = Some(cached_list.clone());
                    } else {
                        cache.lock().unwrap().remove(&target);
                    }
                }
                if svn_list.is_none() {
                    let list = cmd.list(&target, false)?;
                    cache
                        .lock()
                        .unwrap()
                        .insert(target, (list.clone(), SystemTime::now()));
                    svn_list = Some(list);
                }
                Ok::<SvnList, CustomError>(svn_list.unwrap())
            };
            let svn_list_res = get_svnlist();
            debug!("data: '{svn_list_res:?}'");
            tx.send(svn_list_res).unwrap();
            debug!("info sent");
        };
        Arc::new(fetcher)
    }
}

pub(crate) fn request_new_data(selected: String, cb: Arc<ListFetcher>) -> Receiver<ResultSvnList> {
    let (tx, rx) = channel::<ResultSvnList>();
    thread::spawn(move || (cb)(selected, tx));
    rx
}

pub(crate) fn get_new_data(rx: &Receiver<ResultSvnList>) -> Option<ResultSvnList> {
    rx.try_recv().ok()
}

#[derive(Default)]
pub(crate) struct CustomList {
    items: SvnList,
    pub(crate) base_url: String,
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
        self.items.iter().count()
    }

    fn get_list_items(&self) -> Vec<ListItem> {
        self.items
            .iter()
            .map(|i| ListItem::new(i.name.as_ref()))
            .collect()
    }

    fn get_current_selected(&self, state: &impl ListStateOps) -> Option<ListEntry> {
        if let Some(selected) = state.get() {
            if let Some(item) = self.items.iter().nth(selected) {
                return Some(item.clone());
            }
        }
        None
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

impl From<String> for CustomList {
    fn from(base_url: String) -> Self {
        Self {
            items: SvnList::default(),
            base_url,
        }
    }
}

impl From<(SvnList, String)> for CustomList {
    fn from(pair: (SvnList, String)) -> Self {
        Self {
            items: pair.0,
            base_url: pair.1,
        }
    }
}

#[derive(Debug)]
pub(crate) enum CustomError {
    Io(io::Error),
    Svn(SvnError),
    SystemTime(SystemTimeError),
    NoDataToList,
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

impl From<SystemTimeError> for CustomError {
    fn from(e: SystemTimeError) -> Self {
        CustomError::SystemTime(e)
    }
}

#[derive(PartialEq, Eq)]
pub(crate) enum Request {
    Forward(String),
}

pub(crate) struct CustomLists {
    lists: Vec<CustomList>,
    current: usize,
}

impl From<Vec<CustomList>> for CustomLists {
    fn from(lists: Vec<CustomList>) -> Self {
        CustomLists { lists, current: 0 }
    }
}

impl CustomLists {
    pub(crate) fn add_new_list(&mut self, list: CustomList) {
        self.lists.truncate(self.current + 1);
        self.lists.push(list);
        self.current += 1;
    }

    pub(crate) fn go_back(
        &mut self,
    ) -> (
        Option<&CustomList>,
        Option<&CustomList>,
        Option<&CustomList>,
    ) {
        if self.current > 0 {
            self.current -= 1;
        }
        (
            if self.current == 0 {
                None
            } else {
                self.lists.get(self.current - 1)
            },
            self.lists.get(self.current),
            self.lists.get(self.current + 1),
        )
    }

    pub(crate) fn get_current(
        &mut self,
    ) -> (
        Option<&CustomList>,
        Option<&CustomList>,
        Option<&CustomList>,
    ) {
        (
            if self.current == 0 {
                None
            } else {
                self.lists.get(self.current - 1)
            },
            self.lists.get(self.current),
            self.lists.get(self.current + 1),
        )
    }
}
