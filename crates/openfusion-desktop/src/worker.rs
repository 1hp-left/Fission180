use fission180_core::{BodyInfo, DisplayMesh, Document, Model, Result, Session, persistence};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
};

pub enum Command {
    Replace(Document),
    Open(PathBuf),
    New,
    Save(PathBuf),
    Preview(Document),
    ResetPreview,
    Undo,
    Redo,
    Export(PathBuf),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResponseKind {
    Commit,
    Loaded,
    Preview,
    Restore,
    Saved,
    Exported,
    Rejected,
}
pub struct Response {
    pub id: u64,
    pub kind: ResponseKind,
    pub document: Document,
    /// None retains the last valid display, including an earlier valid preview.
    pub display: Option<(DisplayMesh, Vec<BodyInfo>)>,
    pub can_undo: bool,
    pub can_redo: bool,
    pub error: Option<String>,
    pub save_path: Option<PathBuf>,
    pub dirty: bool,
    pub elapsed_ms: f64,
}
struct Engine {
    session: Session,
    committed_mesh: DisplayMesh,
    save_path: Option<PathBuf>,
    saved: Document,
}
impl Engine {
    fn new() -> Self {
        let saved = Document::default();
        Self {
            session: Session::new(saved.clone()).expect("empty document"),
            committed_mesh: DisplayMesh::default(),
            save_path: None,
            saved,
        }
    }
    fn execute(
        &mut self,
        id: u64,
        command: Command,
        mesh: impl Fn(&Model) -> Result<DisplayMesh>,
    ) -> Response {
        let start = std::time::Instant::now();
        let mut display = None;
        let result = (|| -> Result<ResponseKind> {
            let kind = match command {
                Command::Replace(d) => {
                    self.committed_mesh = self.session.replace_with_display(d, &mesh)?;
                    ResponseKind::Commit
                }
                Command::Open(path) => {
                    let d = Document::from_json(&std::fs::read_to_string(&path)?)?;
                    let candidate = Session::new(d)?;
                    let display = mesh(&candidate.model)?;
                    self.session = candidate;
                    self.committed_mesh = display;
                    self.saved = self.session.document.clone();
                    self.save_path = Some(path);
                    ResponseKind::Loaded
                }
                Command::New => {
                    let candidate = Session::new(Document::default())?;
                    let display = mesh(&candidate.model)?;
                    self.session = candidate;
                    self.committed_mesh = display;
                    self.saved = self.session.document.clone();
                    self.save_path = None;
                    ResponseKind::Loaded
                }
                Command::Undo => {
                    if let Some(m) = self.session.undo_with_display(&mesh)? {
                        self.committed_mesh = m;
                    }
                    ResponseKind::Commit
                }
                Command::Redo => {
                    if let Some(m) = self.session.redo_with_display(&mesh)? {
                        self.committed_mesh = m;
                    }
                    ResponseKind::Commit
                }
                Command::Save(path) => {
                    persistence::save(&self.session.document, &path)?;
                    self.save_path = Some(path);
                    self.saved = self.session.document.clone();
                    ResponseKind::Saved
                }
                Command::Preview(d) => {
                    let (_, model) = Model::prepare(&d)?;
                    display = Some((mesh(&model)?, model.info()));
                    ResponseKind::Preview
                }
                Command::ResetPreview => ResponseKind::Restore,
                Command::Export(path) => {
                    self.session.model.export(&path)?;
                    ResponseKind::Exported
                }
            };
            if matches!(
                kind,
                ResponseKind::Commit | ResponseKind::Loaded | ResponseKind::Restore
            ) {
                display = Some((self.committed_mesh.clone(), self.session.model.info()));
            }
            Ok(kind)
        })();
        let (kind, error) = match result {
            Ok(k) => (k, None),
            Err(e) => (ResponseKind::Rejected, Some(e.to_string())),
        };
        Response {
            id,
            kind,
            document: self.session.document.clone(),
            display,
            can_undo: self.session.can_undo(),
            can_redo: self.session.can_redo(),
            error,
            save_path: self.save_path.clone(),
            dirty: self.session.document != self.saved,
            elapsed_ms: start.elapsed().as_secs_f64() * 1000.,
        }
    }
}
pub struct Worker {
    tx: Sender<(u64, Command)>,
    rx: Receiver<Response>,
    pub latest: u64,
}
impl Worker {
    pub fn new(ctx: eframe::egui::Context) -> Self {
        Self::with_mesher(ctx, Model::display_mesh)
    }
    fn with_mesher(
        ctx: eframe::egui::Context,
        mesh: impl Fn(&Model) -> Result<DisplayMesh> + Send + 'static,
    ) -> Self {
        let (tx, requests) = mpsc::channel();
        let (responses, rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("fission180-geometry".into())
            .spawn(move || {
                let mut engine = Engine::new();
                while let Ok((id, command)) = requests.recv() {
                    if responses.send(engine.execute(id, command, &mesh)).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            })
            .expect("create geometry worker");
        Self { tx, rx, latest: 0 }
    }
    pub fn send(&mut self, command: Command) {
        self.latest = self
            .latest
            .checked_add(1)
            .expect("worker request ID space exhausted");
        self.tx
            .send((self.latest, command))
            .expect("geometry worker unavailable");
    }
    pub fn poll(&self) -> impl Iterator<Item = Response> + '_ {
        self.rx.try_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn document() -> Document {
        Document::from_json(include_str!(
            "../../../examples/fusion-reference-extrude.ofrust"
        ))
        .unwrap()
    }
    #[test]
    fn rejected_open_preserves_path_dirty_history_geometry_and_file() {
        let dir = tempfile::tempdir().unwrap();
        let good = dir.path().join("good.ofrust");
        let bad = dir.path().join("bad.ofrust");
        let mut engine = Engine::new();
        engine.execute(1, Command::Replace(document()), Model::display_mesh);
        engine.execute(2, Command::Save(good.clone()), Model::display_mesh);
        let mut edited = document();
        edited.extrusions[0].distance = 12.;
        engine.execute(3, Command::Replace(edited.clone()), Model::display_mesh);
        let mut invalid = document();
        invalid.extrusions[0].operation = fission180_core::Operation::Cut;
        let bytes = invalid.to_json().unwrap();
        std::fs::write(&bad, &bytes).unwrap();
        let r = engine.execute(4, Command::Open(bad.clone()), Model::display_mesh);
        assert_eq!(r.kind, ResponseKind::Rejected);
        assert_eq!(r.document, edited);
        assert_eq!(r.save_path, Some(good.clone()));
        assert!(r.dirty && r.can_undo);
        assert!(r.display.is_none());
        engine.execute(5, Command::Save(good.clone()), Model::display_mesh);
        assert_eq!(std::fs::read_to_string(bad).unwrap(), bytes);
        assert_eq!(
            Document::from_json(&std::fs::read_to_string(&good).unwrap()).unwrap(),
            edited
        );
        engine.execute(6, Command::Open(good), Model::display_mesh);
        assert!(!engine.session.can_undo());
        assert!(!engine.session.can_redo());
        engine.execute(7, Command::New, Model::display_mesh);
        assert!(!engine.session.can_undo());
        assert!(engine.save_path.is_none());
    }
    #[test]
    fn meshing_failure_and_preview_never_commit_or_destroy_display() {
        let mut engine = Engine::new();
        let accepted = engine.execute(1, Command::Replace(document()), Model::display_mesh);
        let vertices = accepted.display.unwrap().0.vertices;
        let mut edit = document();
        edit.extrusions[0].distance = 12.;
        let fail = |_: &Model| {
            Err(fission180_core::Error::Invalid(
                "injected mesh failure".into(),
            ))
        };
        let r = engine.execute(2, Command::Replace(edit.clone()), fail);
        assert_eq!(r.document, document());
        assert!(r.display.is_none());
        let r = engine.execute(3, Command::Preview(edit), Model::display_mesh);
        assert_eq!(r.kind, ResponseKind::Preview);
        assert_eq!(r.document, document());
        let r = engine.execute(4, Command::ResetPreview, fail);
        assert_eq!(r.display.unwrap().0.vertices, vertices);
        let r = engine.execute(5, Command::Undo, fail);
        assert_eq!(r.kind, ResponseKind::Rejected);
        assert_eq!(r.document, document());
        assert!(r.can_undo);
    }
    #[test]
    fn queued_commit_preview_save_retains_every_ack_and_saves_accepted_edit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("queued.ofrust");
        let gate = std::sync::Arc::new(std::sync::Barrier::new(2));
        let thread_gate = gate.clone();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let first = std::sync::atomic::AtomicBool::new(true);
        let mut worker = Worker::with_mesher(eframe::egui::Context::default(), move |model| {
            if first.swap(false, std::sync::atomic::Ordering::SeqCst) {
                started_tx.send(()).unwrap();
                thread_gate.wait();
            }
            Model::display_mesh(model)
        });
        worker.send(Command::Replace(document()));
        started_rx
            .recv_timeout(std::time::Duration::from_secs(20))
            .unwrap();
        let mut preview = document();
        preview.extrusions[0].distance = 99.;
        worker.send(Command::Preview(preview));
        worker.send(Command::Save(path.clone()));
        assert!(!path.exists(), "Save must wait behind the unaccepted edit");
        gate.wait();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        let mut responses = vec![];
        while responses.len() < 3 && std::time::Instant::now() < deadline {
            responses.extend(worker.poll());
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert_eq!(
            responses.iter().map(|r| r.kind).collect::<Vec<_>>(),
            vec![
                ResponseKind::Commit,
                ResponseKind::Preview,
                ResponseKind::Saved
            ]
        );
        assert_eq!(
            Document::from_json(&std::fs::read_to_string(path).unwrap()).unwrap(),
            document()
        );
    }
    #[cfg(windows)]
    #[test]
    fn failed_save_preserves_destination_dirty_snapshot_and_path() {
        use std::os::windows::fs::OpenOptionsExt;
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("original.ofrust");
        let locked = dir.path().join("locked.ofrust");
        let mut e = Engine::new();
        e.execute(1, Command::Replace(document()), Model::display_mesh);
        e.execute(2, Command::Save(old.clone()), Model::display_mesh);
        let mut changed = document();
        changed.extrusions[0].distance = 9.;
        e.execute(3, Command::Replace(changed.clone()), Model::display_mesh);
        std::fs::write(&locked, b"unchanged").unwrap();
        let _lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&locked)
            .unwrap();
        let r = e.execute(4, Command::Save(locked.clone()), Model::display_mesh);
        assert_eq!(r.kind, ResponseKind::Rejected);
        assert!(r.dirty && r.can_undo);
        assert_eq!(r.save_path, Some(old));
        assert_eq!(r.document, changed);
        assert_eq!(std::fs::read(locked).unwrap(), b"unchanged");
    }
}
