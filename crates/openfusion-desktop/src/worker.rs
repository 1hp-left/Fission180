use fission180_core::kernel::Backend;
use fission180_core::{BodyInfo, DisplayMesh, Document, Model, Result, Session, persistence};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, Sender},
};

pub enum Command {
    Replace(Document),
    /// Undoable example load, detached from the previous file's save path.
    Example(Document),
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
    pub display_backend: Option<Backend>,
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
        let mut display_backend = None;
        let result = (|| -> Result<ResponseKind> {
            let kind = match command {
                Command::Replace(d) => {
                    self.committed_mesh = self.session.replace_with_display(d, &mesh)?;
                    ResponseKind::Commit
                }
                Command::Example(d) => {
                    self.committed_mesh = self.session.replace_design_with_display(d, &mesh)?;
                    self.save_path = None;
                    self.saved = Document::default();
                    ResponseKind::Loaded
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
                    display_backend = Some(d.backend);
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
                display_backend = Some(self.session.document.backend);
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
            display_backend,
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
    use crate::engine_examples::Example;
    fn volume(response: &Response) -> f64 {
        response
            .display
            .as_ref()
            .unwrap()
            .1
            .iter()
            .map(|b| b.volume)
            .sum()
    }
    #[test]
    fn rust_examples_use_the_app_worker_and_keep_sources_save_paths_and_history_safe() {
        let dir = tempfile::tempdir().unwrap();
        let original_path = dir.path().join("original.ofrust");
        let mut engine = Engine::new();
        engine.execute(1, Command::Replace(document()), Model::display_mesh);
        engine.execute(2, Command::Save(original_path.clone()), Model::display_mesh);
        let original_bytes = std::fs::read(&original_path).unwrap();
        let original = engine.session.document.clone();
        let lens = 200. * std::f64::consts::PI / 3. - 50. * 3f64.sqrt();
        for example in Example::ALL {
            let response = engine.execute(
                3,
                Command::Example(example.document().unwrap()),
                Model::display_mesh,
            );
            assert_eq!(response.kind, ResponseKind::Loaded, "{:?}", response.error);
            assert_eq!(response.display_backend, Some(Backend::Rust));
            assert_eq!(response.document.backend, Backend::Rust);
            assert!(response.save_path.is_none());
            assert!(response.dirty && response.can_undo);
            assert_eq!(std::fs::read(&original_path).unwrap(), original_bytes);
            let (mesh, bodies) = response.display.as_ref().unwrap();
            assert!(!mesh.indices.is_empty());
            assert!(bodies.iter().all(|b| b.backend == Backend::Rust));
            assert_eq!(mesh.face_ids.len(), mesh.indices.len() / 3);
            assert!(mesh.face_ids.iter().all(Option::is_some));
            let (count, expected) = match example {
                Example::Notch => (1, 4000. - 62.5 * std::f64::consts::PI),
                Example::Join => (1, 5. * (200. * std::f64::consts::PI - lens)),
                Example::Intersect => (1, 5. * lens),
                Example::Split => (2, 3800.),
                Example::Annular => (2, 225. * std::f64::consts::PI),
                Example::Pockets => (1, 36000. - 800. * std::f64::consts::PI + 3. * lens),
                Example::Breakthrough => (1, 36000. - 1200. * std::f64::consts::PI + 2. * lens),
                Example::UnequalJoin => (1, 2000. * std::f64::consts::PI - 5. * lens),
            };
            assert_eq!(bodies.len(), count);
            assert!((volume(&response) - expected).abs() < 1e-5);
            let reopened =
                Session::new(Document::from_json(&response.document.to_json().unwrap()).unwrap())
                    .unwrap();
            assert!(
                (reopened.model.info().iter().map(|b| b.volume).sum::<f64>() - expected).abs()
                    < 1e-5
            );
        }
        for _ in 0..Example::ALL.len() {
            assert_ne!(
                engine.execute(4, Command::Undo, Model::display_mesh).kind,
                ResponseKind::Rejected
            );
        }
        assert_eq!(engine.session.document, original);
        assert_eq!(engine.session.document.backend, Backend::Occt);
        let redo = engine.execute(5, Command::Redo, Model::display_mesh);
        assert_eq!(redo.display_backend, Some(Backend::Rust));
        let accepted = engine.session.document.clone();
        let mut invalid = Example::Pockets.document().unwrap();
        invalid.extrusions[2].distance = 0.;
        invalid.sketches[2].origin[2] = 0.;
        let rejected = engine.execute(6, Command::Example(invalid), Model::display_mesh);
        assert_eq!(rejected.kind, ResponseKind::Rejected);
        assert_eq!(rejected.document, accepted);
        assert!(rejected.display.is_none());
        assert!(rejected.display_backend.is_none());
    }
    #[test]
    fn rust_preview_edit_cancel_and_rejection_retain_the_display_backend() {
        let mut engine = Engine::new();
        let d = Example::Pockets.document().unwrap();
        let accepted = engine.execute(1, Command::Example(d), Model::display_mesh);
        let accepted_volume = volume(&accepted);
        let mut edit = accepted.document.clone();
        edit.extrusions[2].distance = -4.;
        let preview = engine.execute(2, Command::Preview(edit.clone()), Model::display_mesh);
        assert_eq!(preview.kind, ResponseKind::Preview);
        assert_eq!(preview.display_backend, Some(Backend::Rust));
        assert_eq!(preview.document, accepted.document);
        assert!(volume(&preview) > accepted_volume);
        let restored = engine.execute(3, Command::ResetPreview, Model::display_mesh);
        assert_eq!(restored.display_backend, Some(Backend::Rust));
        assert!((volume(&restored) - accepted_volume).abs() < 1e-5);
        let committed = engine.execute(4, Command::Replace(edit), Model::display_mesh);
        assert_eq!(committed.kind, ResponseKind::Commit);
        assert_eq!(committed.display_backend, Some(Backend::Rust));
        let mut invalid = committed.document.clone();
        invalid.sketches[2].origin[2] = 0.;
        invalid.extrusions[2].distance = 0.;
        let rejected = engine.execute(5, Command::Replace(invalid), Model::display_mesh);
        assert_eq!(rejected.kind, ResponseKind::Rejected);
        assert_eq!(rejected.document, committed.document);
        assert!(rejected.display_backend.is_none());
        assert!(rejected.display.is_none());
        let undone = engine.execute(6, Command::Undo, Model::display_mesh);
        assert!((volume(&undone) - accepted_volume).abs() < 1e-5);
        let redone = engine.execute(7, Command::Redo, Model::display_mesh);
        assert!((volume(&redone) - volume(&committed)).abs() < 1e-5);
        // Empty previews still identify the executing engine, independently of
        // the backend of the accepted document.
        let preview = engine.execute(
            8,
            Command::Preview(Document::default()),
            Model::display_mesh,
        );
        assert_eq!(preview.display_backend, Some(Backend::Occt));
        assert_eq!(preview.document.backend, Backend::Rust);
        assert!(preview.display.unwrap().1.is_empty());
    }
    #[test]
    fn failed_backend_switch_retains_the_occt_document_and_saved_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("occt.ofrust");
        let mut engine = Engine::new();
        let mut d = document();
        let occt = engine.execute(0, Command::Replace(d.clone()), Model::display_mesh);
        let mut rust = d.clone();
        rust.backend = Backend::Rust;
        let switched = engine.execute(0, Command::Replace(rust), Model::display_mesh);
        assert_eq!(switched.kind, ResponseKind::Commit);
        assert_eq!(switched.display_backend, Some(Backend::Rust));
        assert!((volume(&switched) - volume(&occt)).abs() < 1e-5);
        let undone = engine.execute(0, Command::Undo, Model::display_mesh);
        assert_eq!(undone.display_backend, Some(Backend::Occt));
        d.push_feature(fission180_core::features::Feature {
            id: 8,
            name: "Fillet".into(),
            target: Some(3),
            outputs: vec![],
            kind: fission180_core::features::FeatureKind::Fillet {
                radius: "1 mm".into(),
            },
        })
        .unwrap();
        let accepted = engine.execute(1, Command::Replace(d), Model::display_mesh);
        assert_eq!(accepted.kind, ResponseKind::Commit);
        engine.execute(2, Command::Save(path.clone()), Model::display_mesh);
        let bytes = std::fs::read(&path).unwrap();
        let mut rust = accepted.document.clone();
        rust.backend = Backend::Rust;
        let rejected = engine.execute(3, Command::Replace(rust), Model::display_mesh);
        assert_eq!(rejected.kind, ResponseKind::Rejected);
        assert_eq!(rejected.document.backend, Backend::Occt);
        assert!(rejected.error.unwrap().contains("unsupported"));
        assert_eq!(rejected.save_path, Some(path.clone()));
        assert!(!rejected.dirty);
        assert_eq!(std::fs::read(path).unwrap(), bytes);
    }
    fn document() -> Document {
        let d = Document::from_json(include_str!(
            "../../../examples/fusion-reference-extrude.ofrust"
        ))
        .unwrap();
        // Compare accepted documents, including materialized region dependencies.
        Model::prepare(&d).unwrap().0
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
        let r = engine.execute(
            2,
            Command::Example(Example::Pockets.document().unwrap()),
            fail,
        );
        assert_eq!(r.kind, ResponseKind::Rejected);
        assert_eq!(r.document, document());
        assert!(r.display.is_none());
        assert!(r.display_backend.is_none());
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
    #[test]
    fn axial_examples_preview_cancel_edit_history_failures_and_save_use_the_native_worker() {
        let dir = tempfile::tempdir().unwrap();
        for (example, index, value) in [
            (Example::Breakthrough, 2, 5.5),
            (Example::UnequalJoin, 1, 9.),
        ] {
            let mut engine = Engine::new();
            let initial = engine.execute(
                1,
                Command::Example(example.document().unwrap()),
                Model::display_mesh,
            );
            let initial_volume = volume(&initial);
            let path = dir.path().join(format!("{}.ofrust", example.key()));
            engine.execute(2, Command::Save(path.clone()), Model::display_mesh);
            let saved = std::fs::read(&path).unwrap();
            let mut edit = initial.document.clone();
            edit.extrusions[index].distance = value;
            let preview = engine.execute(3, Command::Preview(edit.clone()), Model::display_mesh);
            assert_eq!(preview.kind, ResponseKind::Preview);
            assert_eq!(preview.display_backend, Some(Backend::Rust));
            assert_eq!(preview.document, initial.document);
            assert_eq!(preview.save_path, Some(path.clone()));
            let restored = engine.execute(4, Command::ResetPreview, Model::display_mesh);
            assert_eq!(
                restored.display.as_ref().unwrap().0.vertices,
                initial.display.as_ref().unwrap().0.vertices
            );
            let committed = engine.execute(5, Command::Replace(edit), Model::display_mesh);
            assert_eq!(committed.kind, ResponseKind::Commit);
            let edited_volume = volume(&committed);
            assert!((edited_volume - initial_volume).abs() > 1.);
            let fail = |_: &Model| {
                Err(fission180_core::Error::Invalid(
                    "injected axial mesh failure".into(),
                ))
            };
            let mut candidate = committed.document.clone();
            candidate.extrusions[index].distance = value - 0.25;
            let rejected = engine.execute(6, Command::Replace(candidate), fail);
            assert_eq!(rejected.kind, ResponseKind::Rejected);
            assert_eq!(rejected.document, committed.document);
            assert!(rejected.display.is_none());
            assert_eq!(rejected.save_path, Some(path.clone()));
            let mut invalid = committed.document.clone();
            invalid.extrusions[index].distance = 0.;
            let rejected = engine.execute(7, Command::Replace(invalid), Model::display_mesh);
            assert_eq!(rejected.kind, ResponseKind::Rejected);
            assert_eq!(rejected.document, committed.document);
            assert!(rejected.can_undo && rejected.dirty);
            assert_eq!(std::fs::read(&path).unwrap(), saved);
            let undone = engine.execute(8, Command::Undo, Model::display_mesh);
            assert!((volume(&undone) - initial_volume).abs() < 1e-5);
            let redone = engine.execute(9, Command::Redo, Model::display_mesh);
            assert!((volume(&redone) - edited_volume).abs() < 1e-5);
            engine.execute(10, Command::Save(path.clone()), Model::display_mesh);
            let reopened = engine.execute(11, Command::Open(path), Model::display_mesh);
            assert_eq!(reopened.document, committed.document);
            assert!((volume(&reopened) - edited_volume).abs() < 1e-5);
        }
    }
}
