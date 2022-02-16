use crate::{lister::svn_helper, CustomError, MAX_VALIDITY_OF_CACHED_LIST};
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

#[derive(Eq, PartialEq, Hash)]
pub(crate) struct TargetUrl(pub(crate) String);

#[derive(Eq, PartialEq, Hash)]
pub(crate) enum DataRequest {
    Info(TargetUrl),
    List(TargetUrl),
    Log(TargetUrl),
    Text(TargetUrl),
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
type ResultDataResponse = Result<DataResponse, CustomError>;
type ResponseCb = dyn Fn(Result<DataResponse, CustomError>) + Send;

impl DataHandler {
    pub(crate) fn request<F>(&'static mut self, req: DataRequest, view_id: ViewId, f: F)
    where
        F: Fn(ResultDataResponse) + Send + 'static,
    {
        let thread_ids = Arc::clone(&self.thread_ids);
        let id = self.create_fetcher(req, move |svnlist_result, thread_id| {
            let locked = thread_ids.lock().unwrap();
            let (cur_id, cb) = locked.get(&view_id).unwrap();
            if cur_id == &thread_id {
                (cb)(svnlist_result);
            }
        });
        self.thread_ids
            .lock()
            .unwrap()
            .insert(view_id, (id, Box::new(f)));
    }

    fn create_fetcher<F>(&'static self, req: DataRequest, cb: F) -> ThreadId
    where
        F: Fn(ResultDataResponse, ThreadId) + Send + 'static,
    {
        let join_h = thread::spawn(move || {
            let res_resp = self.get_cached(req);
            (cb)(res_resp, thread::current().id());
        });
        join_h.thread().id()
    }

    fn get_cached(&self, req: DataRequest) -> ResultDataResponse {
        let mut ret: Option<_> = None;
        {
            let locked = self.cache.lock().unwrap();
            if let Some((resp, sys_time)) = locked.get(&req) {
                if SystemTime::now().duration_since(*sys_time)? < MAX_VALIDITY_OF_CACHED_LIST {
                    ret = Some(Ok(resp.clone()));
                }
            };
        }
        if ret.is_none() {
            let cmd = svn_helper::new();
            let int_ret: ResultDataResponse = match &req {
                DataRequest::List(TargetUrl(url)) => cmd
                    .list(url, false)
                    .map_or_else(|e| Err(e.into()), |v| Ok(v.into())),
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
        }
        ret.unwrap()
    }
}
