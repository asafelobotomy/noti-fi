//! The D-Bus side that apps talk to: org.kde.JobViewServer (V1 and V2) and org.kde.kuiserver at /JobViewServer,
//! and one org.kde.JobViewV2/V3 object per job. Methods only record what changed; the worker shows it.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use zbus::message::Header;
use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{ObjectServer, fdo, interface};

use crate::apps;
use crate::model::{Job, Val};

/// More live jobs than this from all apps together is a runaway client: further requests are refused.
const MAX_JOBS: usize = 256;

pub(crate) enum Ev {
    Changed(u32),
    Action(u32, String),
    Closed(u32, u32),
    OwnerGone(String),
}

pub(crate) struct Entry {
    pub job: Job,
    /// The unique bus name that asked for the view; only it may update the job.
    pub owner: String,
    pub v3: bool,
    pub path: String,
    pub created: Instant,
    /// The notification showing it (0 = none yet, or the user closed it).
    pub nid: u32,
    pub shown: bool,
    /// The user closed the progress popup: no more progress updates, only the final result.
    pub hidden: bool,
    pub last: Option<Instant>,
    pub due: Option<Instant>,
}

pub(crate) struct Shared {
    pub jobs: Mutex<HashMap<u32, Entry>>,
    tx: Sender<Ev>,
    next: AtomicU32,
}

impl Shared {
    pub fn new(tx: Sender<Ev>) -> Shared {
        Shared { jobs: Mutex::new(HashMap::new()), tx, next: AtomicU32::new(1) }
    }

    pub fn send(&self, ev: Ev) {
        let _ = self.tx.send(ev);
    }

    /// Change one job, if `sender` owns it.
    fn with(&self, id: u32, hdr: &Header<'_>, f: impl FnOnce(&mut Job)) {
        let sender = hdr.sender().map(|s| s.as_str()).unwrap_or("");
        {
            let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
            let Some(e) = jobs.get_mut(&id) else { return };
            if e.owner != sender || e.job.is_done() {
                return;
            }
            f(&mut e.job);
        }
        self.send(Ev::Changed(id));
    }

    async fn open(self: &Arc<Self>, server: &ObjectServer, hdr: &Header<'_>, mut job: Job, v3: bool) -> fdo::Result<OwnedObjectPath> {
        apps::fill(&mut job);
        let owner = hdr.sender().map(|s| s.to_string()).unwrap_or_default();
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let path = format!("/org/kde/notificationmanager/jobs/JobView_{id}");
        {
            let mut jobs = self.jobs.lock().unwrap_or_else(|e| e.into_inner());
            if jobs.len() >= MAX_JOBS {
                return Err(fdo::Error::LimitsExceeded("too many jobs".into()));
            }
            let entry = Entry { job, owner, v3, path: path.clone(), created: Instant::now(), nid: 0, shown: false, hidden: false, last: None, due: None };
            jobs.insert(id, entry);
        }
        let added = if v3 { server.at(path.as_str(), ViewV3 { id, sh: self.clone() }).await } else { server.at(path.as_str(), ViewV2 { id, sh: self.clone() }).await };
        if let Err(e) = added {
            self.jobs.lock().unwrap_or_else(|e| e.into_inner()).remove(&id);
            return Err(fdo::Error::Failed(e.to_string()));
        }
        self.send(Ev::Changed(id));
        OwnedObjectPath::try_from(path).map_err(|e| fdo::Error::Failed(e.to_string()))
    }
}

pub(crate) fn val(v: &Value<'_>) -> Option<Val> {
    Some(match v {
        Value::U8(x) => Val::U(*x as u64),
        Value::U16(x) => Val::U(*x as u64),
        Value::U32(x) => Val::U(*x as u64),
        Value::U64(x) => Val::U(*x),
        Value::I16(x) => Val::I(*x as i64),
        Value::I32(x) => Val::I(*x as i64),
        Value::I64(x) => Val::I(*x),
        Value::Bool(b) => Val::B(*b),
        Value::Str(s) => Val::S(s.to_string()),
        Value::Value(inner) => return val(inner),
        _ => return None,
    })
}

pub(crate) fn vals(m: &HashMap<String, OwnedValue>) -> HashMap<String, Val> {
    m.iter().filter_map(|(k, v)| val(v).map(|v| (k.clone(), v))).collect()
}

pub(crate) struct ServerV2(pub Arc<Shared>);

#[interface(name = "org.kde.JobViewServerV2")]
impl ServerV2 {
    #[zbus(name = "requestView")]
    async fn request_view(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(object_server)] server: &ObjectServer,
        desktop_entry: String,
        capabilities: i32,
        hints: HashMap<String, OwnedValue>,
    ) -> fdo::Result<OwnedObjectPath> {
        self.0.open(server, &hdr, Job::new(&desktop_entry, capabilities, &vals(&hints)), true).await
    }
}

pub(crate) struct ServerV1(pub Arc<Shared>);

#[interface(name = "org.kde.JobViewServer")]
impl ServerV1 {
    #[zbus(name = "requestView")]
    async fn request_view(
        &self,
        #[zbus(header)] hdr: Header<'_>,
        #[zbus(object_server)] server: &ObjectServer,
        app_name: String,
        app_icon_name: String,
        capabilities: i32,
    ) -> fdo::Result<OwnedObjectPath> {
        // V1 sends only a name ("dolphin"): find its desktop entry like Plasma does
        let entry = apps::entry_for_name(&app_name).unwrap_or_default();
        let mut job = Job::new(&entry, capabilities, &HashMap::new());
        if entry.is_empty() {
            job.app_name = app_name;
        }
        job.app_icon = app_icon_name;
        self.0.open(server, &hdr, job, false).await
    }
}

/// The old kuiserver interface: KIO only asks whether it must also show its own progress windows (no).
pub(crate) struct Kuiserver;

#[interface(name = "org.kde.kuiserver")]
impl Kuiserver {
    #[zbus(name = "registerService")]
    fn register_service(&self, _service: String, _object_path: String) {}
    #[zbus(name = "emitJobUrlsChanged")]
    fn emit_job_urls_changed(&self) {}
    #[zbus(name = "requiresJobTracker")]
    fn requires_job_tracker(&self) -> bool {
        false
    }
    #[zbus(name = "registeredJobContacts")]
    fn registered_job_contacts(&self) -> Vec<String> {
        Vec::new()
    }
}

pub(crate) struct ViewV3 {
    id: u32,
    sh: Arc<Shared>,
}

#[interface(name = "org.kde.JobViewV3")]
impl ViewV3 {
    #[zbus(name = "update")]
    fn update(&self, #[zbus(header)] hdr: Header<'_>, properties: HashMap<String, OwnedValue>) {
        self.sh.with(self.id, &hdr, |j| j.update(&vals(&properties)));
    }
    #[zbus(name = "terminate")]
    fn terminate(&self, #[zbus(header)] hdr: Header<'_>, error_code: u32, error_message: String, _hints: HashMap<String, OwnedValue>) {
        self.sh.with(self.id, &hdr, |j| j.terminate(error_code, &error_message));
    }
}

pub(crate) struct ViewV2 {
    id: u32,
    sh: Arc<Shared>,
}

#[interface(name = "org.kde.JobViewV2")]
impl ViewV2 {
    #[zbus(name = "terminate")]
    fn terminate(&self, #[zbus(header)] hdr: Header<'_>, error_message: String) {
        self.sh.with(self.id, &hdr, |j| j.terminate(j.pending_error, &error_message));
    }
    #[zbus(name = "setSuspended")]
    fn set_suspended(&self, #[zbus(header)] hdr: Header<'_>, suspended: bool) {
        self.sh.with(self.id, &hdr, |j| j.set_suspended(suspended));
    }
    #[zbus(name = "setTotalAmount")]
    fn set_total_amount(&self, #[zbus(header)] hdr: Header<'_>, amount: u64, unit: String) {
        self.sh.with(self.id, &hdr, |j| j.set_amount(false, amount, &unit));
    }
    #[zbus(name = "setProcessedAmount")]
    fn set_processed_amount(&self, #[zbus(header)] hdr: Header<'_>, amount: u64, unit: String) {
        self.sh.with(self.id, &hdr, |j| j.set_amount(true, amount, &unit));
    }
    #[zbus(name = "setPercent")]
    fn set_percent(&self, #[zbus(header)] hdr: Header<'_>, percent: u32) {
        self.sh.with(self.id, &hdr, |j| j.percent = Some(percent.min(100)));
    }
    #[zbus(name = "setSpeed")]
    fn set_speed(&self, #[zbus(header)] hdr: Header<'_>, bytes_per_second: u64) {
        self.sh.with(self.id, &hdr, |j| j.speed = bytes_per_second);
    }
    #[zbus(name = "setInfoMessage")]
    fn set_info_message(&self, #[zbus(header)] hdr: Header<'_>, message: String) {
        self.sh.with(self.id, &hdr, |j| j.set_info_message(&message));
    }
    #[zbus(name = "setDescriptionField")]
    fn set_description_field(&self, #[zbus(header)] hdr: Header<'_>, number: u32, name: String, value: String) -> bool {
        self.sh.with(self.id, &hdr, |j| j.set_description(number, &name, &value));
        false
    }
    #[zbus(name = "clearDescriptionField")]
    fn clear_description_field(&self, #[zbus(header)] hdr: Header<'_>, number: u32) {
        self.sh.with(self.id, &hdr, |j| j.set_description(number, "", ""));
    }
    #[zbus(name = "setDestUrl")]
    fn set_dest_url(&self, #[zbus(header)] hdr: Header<'_>, dest_url: OwnedValue) {
        if let Some(Val::S(u)) = val(&dest_url) {
            self.sh.with(self.id, &hdr, |j| j.dest_url = u);
        }
    }
    #[zbus(name = "setError")]
    fn set_error(&self, #[zbus(header)] hdr: Header<'_>, error_code: u32) {
        self.sh.with(self.id, &hdr, |j| j.pending_error = error_code);
    }
}
