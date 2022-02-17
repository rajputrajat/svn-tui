use crate::{lister::svn_helper, CustomError, MAX_VALIDITY_OF_CACHED_LIST};
use log::debug;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    thread::{self, ThreadId},
    time::SystemTime,
};
use svn_cmd::{SvnError, SvnInfo, SvnList, SvnLog};

#[derive(Default)]
pub(crate) struct DataHandler {
    thread_ids: Arc<Mutex<HashMap<ViewId, (ThreadId, Box<ResponseCb>)>>>,
    cache: Arc<Mutex<HashMap<DataRequest, (DataResponse, SystemTime)>>>,
}

#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub(crate) struct TargetUrl(pub(crate) String);

#[derive(Eq, PartialEq, Hash, Clone, Debug)]
pub(crate) enum DataRequest {
    Info(TargetUrl),
    List(TargetUrl),
    Log(TargetUrl),
    Text(TargetUrl),
}

impl From<TargetUrl> for String {
    fn from(t: TargetUrl) -> Self {
        t.0
    }
}

impl From<DataRequest> for TargetUrl {
    fn from(r: DataRequest) -> Self {
        match r {
            DataRequest::Log(u) => u,
            DataRequest::List(u) => u,
            DataRequest::Info(u) => u,
            DataRequest::Text(u) => u,
        }
    }
}

#[derive(Clone)]
pub(crate) enum DataResponse {
    Info(SvnInfo),
    List(SvnList),
    Log(SvnLog),
    Text(String),
}

impl From<SvnInfo> for DataResponse {
    fn from(i: SvnInfo) -> Self {
        DataResponse::Info(i)
    }
}
impl From<SvnList> for DataResponse {
    fn from(l: SvnList) -> Self {
        DataResponse::List(l)
    }
}
impl From<SvnLog> for DataResponse {
    fn from(l: SvnLog) -> Self {
        DataResponse::Log(l)
    }
}
impl From<String> for DataResponse {
    fn from(t: String) -> Self {
        DataResponse::Text(t)
    }
}

#[derive(Eq, PartialEq, Hash, Clone, Copy)]
pub(crate) enum ViewId {
    MainList,
    BottomInfo,
    RightInfoPane,
}

type ResultSvnList = Result<SvnList, SvnError>;
pub(crate) type ResultDataResponse = Result<DataResponse, CustomError>;
type ResponseCb = dyn FnMut(ResultDataResponse) + Send;

impl DataHandler {
    pub(crate) fn request<F>(self: Arc<Self>, req: DataRequest, view_id: ViewId, f: F)
    where
        F: FnMut(ResultDataResponse) + Send + 'static,
    {
        let thread_ids = Arc::clone(&self.thread_ids);
        let id = Arc::clone(&self).create_fetcher(req, move |svnlist_result, thread_id| {
            let mut locked = thread_ids.lock().unwrap();
            let (cur_id, cb) = locked.get_mut(&view_id).unwrap();
            if cur_id == &thread_id {
                (cb)(svnlist_result);
            }
        });
        self.thread_ids
            .lock()
            .unwrap()
            .insert(view_id, (id, Box::new(f)));
    }

    fn create_fetcher<F>(self: Arc<Self>, req: DataRequest, mut cb: F) -> ThreadId
    where
        F: FnMut(ResultDataResponse, ThreadId) + Send + 'static,
    {
        let join_h = thread::spawn(move || {
            let res_resp = self.get_cached(req);
            let cur_id = thread::current().id();
            debug!("current thread id: {cur_id:?}");
            (cb)(res_resp, cur_id);
        });
        let id = join_h.thread().id();
        debug!("thread id: {id:?}");
        id
    }

    fn get_cached(self: Arc<Self>, req: DataRequest) -> ResultDataResponse {
        {
            let locked = self.cache.lock().unwrap();
            if let Some((resp, sys_time)) = locked.get(&req) {
                if SystemTime::now().duration_since(*sys_time)? < MAX_VALIDITY_OF_CACHED_LIST {
                    return Ok(resp.clone());
                }
            };
        }
        let cmd = svn_helper::new();
        let int_ret: ResultDataResponse = match &req {
            DataRequest::List(TargetUrl(url)) => {
                debug!("list requested for {url}");
                let list = cmd
                    .list(url, false)
                    .map_or_else(|e| Err(e.into()), |v| Ok(v.into()));
                debug!("got list");
                list
            }
            DataRequest::Log(TargetUrl(url)) => cmd
                .log(url)
                .map_or_else(|e| Err(e.into()), |v| Ok(v.into())),
            DataRequest::Info(TargetUrl(url)) => cmd
                .info(url)
                .map_or_else(|e| Err(e.into()), |v| Ok(v.into())),
            DataRequest::Text(TargetUrl(url)) => cmd
                .cat(url)
                .map_or_else(|e| Err(e.into()), |v| Ok(v.into())),
        };
        if let Ok(resp) = &int_ret {
            let mut locked = self.cache.lock().unwrap();
            locked.insert(req, (resp.clone(), SystemTime::now()));
        }
        return int_ret;
    }
}
