use crate::{lister::svn_helper, CustomError};
use std::{
    collections::HashMap,
    thread::{self, ThreadId},
};
use svn_cmd::{SvnError, SvnInfo, SvnList, SvnLog};

pub(crate) struct DataHandler {
    thread_ids: HashMap<ViewId, ThreadId>,
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

#[derive(Eq, PartialEq, Hash)]
pub(crate) enum ViewId {
    MainList,
    BottomInfo,
    RightInfoPane,
}

type ResultSvnList = Result<SvnList, SvnError>;

impl DataHandler {
    pub(crate) fn new() -> Self {
        Self {
            thread_ids: HashMap::new(),
        }
    }

    pub(crate) fn request<F>(&mut self, req: DataRequest, view_id: ViewId, f: F)
    where
        F: Fn(DataResponse) -> Result<(), CustomError>,
    {
        match req {
            DataRequest::List(target) => {
                let id = self.create_list_fetcher(target, |svnlist_result, thread_id| {});
                self.thread_ids.insert(view_id, id);
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
