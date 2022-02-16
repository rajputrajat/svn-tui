use crate::{lister::svn_helper, CustomError};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread::{self, ThreadId},
};
use svn_cmd::{SvnError, SvnInfo, SvnList, SvnLog};

pub(crate) struct DataHandler {
    thread_ids: Arc<Mutex<HashMap<ViewId, (ThreadId, Box<ResponseCb>)>>>,
}

pub(crate) struct TargetUrl(pub(crate) String);

pub(crate) enum DataRequest {
    Info(TargetUrl),
    List(TargetUrl),
    Log(TargetUrl),
    Text(TargetUrl),
}

pub(crate) enum DataResponse {
    Info(SvnInfo),
    List(SvnList),
    Log(SvnLog),
    Text(String),
}

#[derive(Eq, PartialEq, Hash, Clone, Copy)]
pub(crate) enum ViewId {
    MainList,
    BottomInfo,
    RightInfoPane,
}

type ResultSvnList = Result<SvnList, SvnError>;
type ResponseCb = dyn Fn(Result<DataResponse, CustomError>) + Send;

impl DataHandler {
    pub(crate) fn new() -> Self {
        Self {
            thread_ids: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub(crate) fn request<F>(&'static mut self, req: DataRequest, view_id: ViewId, f: F)
    where
        F: Fn(Result<DataResponse, CustomError>) + Send + 'static,
    {
        match req {
            DataRequest::List(target) => {
                let thread_ids = Arc::clone(&self.thread_ids);
                let id = self.create_list_fetcher(target, move |svnlist_result, thread_id| {
                    let locked = thread_ids.lock().unwrap();
                    let (cur_id, cb) = locked.get(&view_id).unwrap();
                    if cur_id == &thread_id {
                        (cb)(svnlist_result.map_or_else(
                            |e| Err(CustomError::Svn(e)),
                            |v| Ok(DataResponse::List(v)),
                        ));
                    }
                });
                self.thread_ids
                    .lock()
                    .unwrap()
                    .insert(view_id, (id, Box::new(f)));
            }
            _ => {}
        }
    }

    fn create_list_fetcher<F>(&self, target: TargetUrl, cb: F) -> ThreadId
    where
        F: Fn(ResultSvnList, ThreadId) + Send + 'static,
    {
        let join_h = thread::spawn(move || {
            let cmd = svn_helper::new();
            let list = cmd.list(&target.0, false);
            (cb)(list, thread::current().id())
        });
        join_h.thread().id()
    }
}
