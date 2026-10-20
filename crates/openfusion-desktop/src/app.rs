use crate::{
    icons::{Icon, arrow, icon},
    view::{Camera, Pick, RenderCache},
    worker::{Command, Worker},
};
use eframe::egui::{
    self, Align2, Color32 as C, FontId, Id, Key, Painter, Pos2, Rect, Sense, Shape, Stroke, Vec2,
    pos2, vec2,
};
use fission180_core::{
    BodyInfo, DisplayMesh, Document, Extrusion, Operation, Plane, Profile, Sketch,
};
use std::path::PathBuf;
#[path = "commands.rs"]
mod commands;

const RIBBON: C = C::from_rgb(58, 68, 83);
const QUICK: C = C::from_rgb(34, 41, 51);
const CANVAS: C = C::from_rgb(47, 52, 63);
const PANEL: C = C::from_rgb(58, 68, 83);
const TEXT: C = C::from_rgb(224, 228, 232);
const MUTED: C = C::from_rgb(165, 178, 194);
const BORDER: C = C::from_rgb(86, 99, 117);
const ACCENT: C = C::from_rgb(59, 154, 196);

fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
    Rect::from_min_size(pos2(x, y), vec2(w, h))
}
fn label(p: &Painter, at: Pos2, text: impl Into<String>, size: f32, col: C) {
    p.text(
        at,
        Align2::LEFT_CENTER,
        text.into(),
        FontId::proportional(size),
        col,
    );
}
fn centered(p: &Painter, r: Rect, text: impl Into<String>, size: f32, col: C) {
    p.text(
        r.center(),
        Align2::CENTER_CENTER,
        text.into(),
        FontId::proportional(size),
        col,
    );
}
fn ctrl_shortcut(ctx: &egui::Context, key: Key) -> bool {
    ctx.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::CTRL, key)))
}
fn button(
    ui: &mut egui::Ui,
    r: Rect,
    id: impl std::hash::Hash + std::fmt::Debug,
    text: &str,
) -> bool {
    let response = ui.interact(r, Id::unique(id), Sense::click());
    ui.painter().rect(
        r,
        1.,
        if response.hovered() {
            C::from_rgb(73, 91, 111)
        } else {
            PANEL
        },
        Stroke::new(1., BORDER),
        egui::StrokeKind::Inside,
    );
    centered(ui.painter(), r, text, 12., TEXT);
    response.clicked()
}
fn tool_button(
    ui: &mut egui::Ui,
    r: Rect,
    kind: Icon,
    name: &str,
    selected: bool,
    enabled: bool,
) -> bool {
    let response = ui.interact(
        r,
        Id::unique((name, r.min.x as i32, r.min.y as i32)),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if selected || response.hovered() {
        ui.painter().rect(
            r,
            1.,
            if selected {
                C::from_rgb(58, 92, 117)
            } else {
                C::from_rgb(71, 84, 101)
            },
            Stroke::new(1., if selected { ACCENT } else { BORDER }),
            egui::StrokeKind::Inside,
        );
    }
    icon(
        ui.painter(),
        r.center(),
        r.width() * if r.height() >= 30. { 1.1 } else { 0.8 },
        kind,
    );
    response.clone().on_hover_text(if enabled {
        name.to_string()
    } else {
        format!("{name} — not implemented yet")
    });
    enabled && response.clicked()
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Design,
    PlaneChoice,
    Sketch,
}
#[derive(Clone, Copy, PartialEq)]
enum Tool {
    Select,
    Rectangle,
    Circle,
    Polyline,
    Line,
    Dimension,
}
struct ExtrudeDialog {
    editing: Option<u64>,
    sketch_id: u64,
    profile: usize,
    distance: String,
    operation: Operation,
    body_id: u64,
    last_preview: String,
    input: fission180_core::expression::DraftInput,
}

#[derive(Default)]
struct SketchHistory {
    undo: Vec<Sketch>,
    redo: Vec<Sketch>,
}
impl SketchHistory {
    fn record(&mut self, sketch: &Sketch) {
        self.undo.push(sketch.clone());
        if self.undo.len() > 100 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }
    fn undo(&mut self, sketch: &mut Sketch) {
        if let Some(previous) = self.undo.pop() {
            self.redo.push(std::mem::replace(sketch, previous));
        }
    }
    fn redo(&mut self, sketch: &mut Sketch) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(sketch, next));
        }
    }
}

pub struct FusionApp {
    document: Document,
    worker: Worker,
    mesh: DisplayMesh,
    bodies: Vec<BodyInfo>,
    committed_bodies: Vec<BodyInfo>,
    camera: Camera,
    cache: RenderCache,
    revision: u64,
    mode: Mode,
    tool: Tool,
    draft: Option<Sketch>,
    sketch_history: SketchHistory,
    anchor: Option<[f64; 2]>,
    polyline: Vec<[f64; 2]>,
    selected: Option<(u64, usize)>,
    selected_topology: Option<Pick>,
    hovered_topology: Option<Pick>,
    extrude: Option<ExtrudeDialog>,
    modeling: Option<commands::ModelingDialog>,
    pending_modeling: Option<String>,
    menu: Option<(&'static str, Pos2)>,
    tab: &'static str,
    home: bool,
    can_undo: bool,
    can_redo: bool,
    busy: bool,
    preview: bool,
    dirty: bool,
    save_path: Option<PathBuf>,
    status: String,
    error: Option<String>,
    show_grid: bool,
    snap: bool,
    show_profiles: bool,
    show_points: bool,
    show_dimensions: bool,
    show_constraints: bool,
    browser_expanded: bool,
    settings_expanded: bool,
    sketches_expanded: bool,
    bodies_expanded: bool,
    origin_expanded: bool,
    show_dimension: bool,
    dim_width: String,
    dim_height: String,
    dim_radius: String,
    capture: Option<PathBuf>,
    capture_frames: u32,
    close_after_capture: bool,
    pending_extrude: bool,
}
impl FusionApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let ctx = &cc.egui_ctx;
        let mut fonts = egui::FontDefinitions::default();
        if let Ok(bytes) = std::fs::read("C:/Windows/Fonts/segoeui.ttf") {
            fonts
                .font_data
                .insert("Segoe UI".into(), egui::FontData::from_owned(bytes).into());
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "Segoe UI".into());
        }
        if let Ok(bytes) = std::fs::read("C:/Windows/Fonts/seguisym.ttf") {
            fonts.font_data.insert(
                "Segoe Symbols".into(),
                egui::FontData::from_owned(bytes).into(),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(1, "Segoe Symbols".into());
        }
        ctx.set_fonts(fonts);
        let mut style = egui::Style::default();
        style.visuals = egui::Visuals::dark();
        style.visuals.panel_fill = PANEL;
        style.visuals.window_fill = PANEL;
        style.visuals.extreme_bg_color = C::from_rgb(43, 54, 69);
        style.visuals.override_text_color = Some(TEXT);
        style.visuals.selection.bg_fill = C::from_rgb(52, 103, 142);
        style.visuals.widgets.inactive.bg_fill = C::from_rgb(43, 54, 69);
        style.visuals.widgets.inactive.fg_stroke = Stroke::new(1., TEXT);
        style.visuals.widgets.inactive.bg_stroke = Stroke::new(1., BORDER);
        style.spacing.item_spacing = vec2(5., 5.);
        style
            .text_styles
            .insert(egui::TextStyle::Body, FontId::proportional(12.));
        style
            .text_styles
            .insert(egui::TextStyle::Button, FontId::proportional(12.));
        ctx.set_global_style(style);
        let mut app = Self {
            document: Document::default(),
            worker: Worker::new(ctx.clone()),
            mesh: DisplayMesh::default(),
            bodies: vec![],
            committed_bodies: vec![],
            camera: Camera::default(),
            cache: RenderCache::default(),
            revision: 0,
            mode: Mode::Design,
            tool: Tool::Select,
            draft: None,
            sketch_history: SketchHistory::default(),
            anchor: None,
            polyline: vec![],
            selected: None,
            selected_topology: None,
            hovered_topology: None,
            extrude: None,
            modeling: None,
            pending_modeling: None,
            menu: None,
            tab: "SOLID",
            home: false,
            can_undo: false,
            can_redo: false,
            busy: false,
            preview: false,
            dirty: false,
            save_path: None,
            status: String::new(),
            error: None,
            show_grid: true,
            snap: true,
            show_profiles: true,
            show_points: true,
            show_dimensions: true,
            show_constraints: true,
            browser_expanded: true,
            settings_expanded: true,
            sketches_expanded: false,
            bodies_expanded: false,
            origin_expanded: false,
            show_dimension: false,
            dim_width: "40".into(),
            dim_height: "30".into(),
            dim_radius: "10".into(),
            capture: None,
            capture_frames: 0,
            close_after_capture: false,
            pending_extrude: false,
        };
        let args: Vec<_> = std::env::args().collect();
        if let Some(i) = args.iter().position(|a| a == "--open") {
            if let Some(path) = args.get(i + 1) {
                app.send(Command::Open(PathBuf::from(path)));
            }
        }
        if let Some(i) = args.iter().position(|a| a == "--capture") {
            app.capture = args.get(i + 1).map(PathBuf::from);
            app.close_after_capture = args.iter().any(|a| a == "--exit-after-capture");
        }
        if args.iter().any(|a| a == "--sketch") {
            app.begin_sketch(Plane::XY);
        }
        if args.iter().any(|a| a == "--demo") {
            let mut d = Document::default();
            d.name = "Native test part".into();
            d.sketches.push(Sketch {
                id: 1,
                name: "Sketch1".into(),
                plane: Plane::XY,
                origin: [0.; 3],
                profiles: vec![Profile::Rectangle {
                    a: [0., 0.],
                    b: [40., 30.],
                }],

                profile_ids: vec![1],
                next_profile_id: 2,
                dimension_expressions: Default::default(),
                graph: None,
                support: None,
            });
            d.extrusions.push(Extrusion {
                id: 2,
                name: "Extrude1".into(),
                sketch_id: 1,
                profile: 0,
                distance: 15.,
                operation: Operation::NewBody,
                body_id: 3,

                region_id: Some(1),
                distance_expression: None,
                result_body_ids: vec![],
            });
            app.send(Command::Replace(d));
        }
        if let Some(i) = args.iter().position(|a| a == "--command") {
            if let Some(name) = args.get(i + 1) {
                if app.busy {
                    app.pending_modeling = Some(name.clone());
                } else {
                    app.start_modeling(name);
                }
            }
        }
        app
    }
    fn send(&mut self, c: Command) {
        self.worker.send(c);
        self.busy = true;
        self.error = None;
    }
    fn begin_sketch(&mut self, plane: Plane) {
        if self.busy {
            return;
        }
        let id = match self.document.next_id() {
            Ok(id) => id,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };
        self.sketch_history = Default::default();
        self.draft = Some(Sketch {
            id,
            name: format!("Sketch{}", self.document.sketches.len() + 1),
            plane,
            origin: [0.; 3],
            profiles: vec![],

            profile_ids: vec![],
            next_profile_id: 1,
            dimension_expressions: Default::default(),
            graph: None,
            support: None,
        });
        self.mode = Mode::Sketch;
        self.tool = Tool::Select;
        self.camera.top = true;
        self.camera.plane = plane;
        self.anchor = None;
        self.polyline.clear();
        self.selected = None;
        self.tab = "SKETCH";
        self.status.clear();
    }
    fn finish_sketch(&mut self) {
        if self.busy {
            return;
        }
        if let Some(sketch) = self.draft.take() {
            let mut d = self.document.clone();
            if let Some(i) = d.sketches.iter().position(|s| s.id == sketch.id) {
                d.sketches[i] = sketch;
            } else {
                d.sketches.push(sketch);
            }
            self.send(Command::Replace(d));
        }
        self.mode = Mode::Design;
        self.tool = Tool::Select;
        self.camera.top = false;
        self.tab = "SOLID";
        self.anchor = None;
        self.polyline.clear();
    }
    fn start_extrude(&mut self) {
        if self.busy {
            return;
        }
        if self.mode == Mode::Sketch {
            self.pending_extrude = true;
            self.finish_sketch();
            return;
        }
        if self.modeling.take().is_some() || self.preview {
            self.send(Command::ResetPreview);
        }
        let selection = self.selected.or_else(|| {
            self.document
                .sketches
                .iter()
                .rev()
                .find_map(|s| (!s.profiles.is_empty()).then_some((s.id, s.profiles.len() - 1)))
        });
        let Some((sketch_id, profile)) = selection else {
            self.status = "Create a sketch profile before starting Extrude".into();
            return;
        };
        self.selected = Some((sketch_id, profile));
        let id = match self.document.next_id().and_then(|id| {
            id.checked_add(1).ok_or_else(|| {
                fission180_core::Error::Invalid("Document ID space is exhausted".into())
            })
        }) {
            Ok(id) => id,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };
        self.extrude = Some(ExtrudeDialog {
            editing: None,
            sketch_id,
            profile,
            distance: "10".into(),
            operation: Operation::NewBody,
            body_id: id,
            last_preview: String::new(),
            input: fission180_core::expression::DraftInput::new(
                "10".into(),
                &self.document.parameters,
            ),
        });
        self.camera.top = false;
    }
    fn extrusion_document(&self) -> Option<Document> {
        let dialog = self.extrude.as_ref()?;
        let distance =
            fission180_core::expression::length(&dialog.distance, &self.document.parameters)
                .ok()?;
        if distance.abs() < 1e-5 {
            return None;
        }
        let mut d = self.document.clone();
        let feature = Extrusion {
            id: dialog.editing.or_else(|| d.next_id().ok())?,
            name: format!("Extrude{}", d.extrusions.len() + 1),
            sketch_id: dialog.sketch_id,
            profile: dialog.profile,
            distance,
            operation: dialog.operation,
            body_id: dialog.body_id,

            region_id: self
                .document
                .sketches
                .iter()
                .find(|s| s.id == dialog.sketch_id)?
                .profile_ids
                .get(dialog.profile)
                .copied(),
            distance_expression: Some(dialog.distance.clone()),
            result_body_ids: dialog
                .editing
                .and_then(|id| d.extrusions.iter().find(|e| e.id == id))
                .map(|e| e.result_body_ids.clone())
                .unwrap_or_default(),
        };
        if let Some(id) = dialog.editing {
            let current = d.extrusions.iter_mut().find(|e| e.id == id)?;
            let name = current.name.clone();
            *current = feature;
            current.name = name;
        } else {
            d.push_extrusion(feature);
        }
        Some(d)
    }
    fn cancel(&mut self) {
        if self.modeling.take().is_some() {
            self.send(Command::ResetPreview);
            return;
        }
        if self.extrude.take().is_some() || self.preview {
            self.send(Command::ResetPreview);
        } else if self.anchor.take().is_some() || !self.polyline.is_empty() {
            self.polyline.clear();
        } else if self.mode == Mode::PlaneChoice {
            self.mode = Mode::Design;
        } else {
            self.tool = Tool::Select;
        }
        self.menu = None;
        self.show_dimension = false;
    }
    fn save(&mut self) {
        if self.mode == Mode::Sketch {
            if self.busy {
                return;
            }
            self.finish_sketch();
        }
        let path = self.save_path.clone().or_else(|| {
            rfd::FileDialog::new()
                .add_filter("Fission 180", &["ofrust"])
                .set_file_name(format!("{}.ofrust", self.document.name))
                .save_file()
        });
        if let Some(path) = path {
            self.send(Command::Save(path));
        }
    }
    fn open(&mut self) {
        if self.busy {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Fission 180", &["ofrust"])
            .pick_file()
        {
            self.send(Command::Open(path));
        }
    }
    fn export(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("STEP", &["step"])
            .add_filter("STL", &["stl"])
            .set_file_name(format!("{}.step", self.document.name))
            .save_file()
        {
            self.send(Command::Export(path));
        }
    }
    fn undo(&mut self) {
        if self.mode == Mode::Sketch {
            if let Some(d) = &mut self.draft {
                self.sketch_history.undo(d);
            }
            self.anchor = None;
            self.polyline.clear();
        } else if !self.busy {
            self.send(Command::Undo);
            self.dirty = true;
        }
    }
    fn redo(&mut self) {
        if self.mode == Mode::Sketch {
            if let Some(d) = &mut self.draft {
                self.sketch_history.redo(d);
            }
        } else if !self.busy {
            self.send(Command::Redo);
            self.dirty = true;
        }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.cancel();
            return;
        }
        // Clicked ribbon and dialog buttons can retain keyboard focus. Only
        // suppress canvas shortcuts while an editable command panel is open.
        if ctx.egui_wants_keyboard_input()
            && (self.extrude.is_some() || self.modeling.is_some() || self.show_dimension)
        {
            return;
        }
        if ctrl_shortcut(ctx, Key::S) {
            self.save();
        }
        if ctrl_shortcut(ctx, Key::N) && !self.busy {
            self.send(Command::New);
        }
        if ctrl_shortcut(ctx, Key::O) {
            self.open();
        }
        if ctrl_shortcut(ctx, Key::Z) {
            self.undo();
        }
        if ctrl_shortcut(ctx, Key::Y) {
            self.redo();
        }
        if ctx.input(|i| !i.modifiers.ctrl && i.key_pressed(Key::E)) {
            self.start_extrude();
        }
        if ctx.input(|i| !i.modifiers.ctrl && i.key_pressed(Key::H)) {
            self.start_modeling("Hole");
        }
        if self.mode == Mode::Sketch {
            if ctx.input(|i| i.key_pressed(Key::R)) {
                self.tool = Tool::Rectangle;
                self.anchor = None;
            }
            if ctx.input(|i| i.key_pressed(Key::C)) {
                self.tool = Tool::Circle;
                self.anchor = None;
            }
            if ctx.input(|i| i.key_pressed(Key::L)) {
                self.tool = Tool::Line;
                self.polyline.clear();
            }
            if ctx.input(|i| i.key_pressed(Key::D)) {
                self.dimension();
            }
            if ctx.input(|i| i.key_pressed(Key::Enter)) && self.polyline.len() >= 3 {
                let points = std::mem::take(&mut self.polyline);
                self.add_profile(Profile::Polygon { points });
            }
        }
        if ctx.input(|i| i.key_pressed(Key::F6)) {
            self.camera = Camera::default();
            if self.mode == Mode::Sketch {
                self.camera.top = true;
            }
        }
    }
    fn add_profile(&mut self, profile: Profile) {
        match profile.validate() {
            Ok(()) => {
                if let Some(d) = &mut self.draft {
                    self.sketch_history.record(d);
                    if let Err(e) = d.add_profile(profile) {
                        self.error = Some(e.to_string());
                        return;
                    }
                    self.selected = Some((d.id, d.profiles.len() - 1));
                    self.dirty = true;
                }
            }
            Err(e) => self.status = e.to_string(),
        }
        self.anchor = None;
    }
    fn dimension(&mut self) {
        if let Some(d) = &self.draft {
            let index = self
                .selected
                .filter(|(id, _)| *id == d.id)
                .map(|(_, i)| i)
                .unwrap_or(d.profiles.len().saturating_sub(1));
            if let Some(p) = d.profiles.get(index) {
                match p {
                    Profile::Rectangle { a, b } => {
                        self.dim_width = format!("{}", (b[0] - a[0]).abs());
                        self.dim_height = format!("{}", (b[1] - a[1]).abs());
                    }
                    Profile::Circle { radius, .. } => self.dim_radius = format!("{radius}"),
                    _ => {}
                }
                if let Some(text) = d
                    .profile_ids
                    .get(index)
                    .and_then(|id| d.dimension_expressions.get(id))
                {
                    if let Some(v) = text.get("width") {
                        self.dim_width = v.clone();
                    }
                    if let Some(v) = text.get("height") {
                        self.dim_height = v.clone();
                    }
                    if let Some(v) = text.get("radius") {
                        self.dim_radius = v.clone();
                    }
                }
                self.selected = Some((d.id, index));
                self.show_dimension = true;
            }
        }
    }

    fn chrome(&mut self, ui: &mut egui::Ui, w: f32) {
        let p = ui.painter().clone();
        p.rect_filled(rect(0., 0., w, 24.), 0., C::from_rgb(31, 31, 32));
        p.rect_filled(rect(3., 3., 14., 17.), 2., C::from_rgb(224, 104, 37));
        centered(&p, rect(3., 2., 14., 17.), "F", 13., C::WHITE);
        label(
            &p,
            pos2(23., 11.),
            format!(
                "{}{}   Fission 180",
                self.document.name,
                if self.dirty { "*" } else { "" }
            ),
            12.,
            TEXT,
        );
        let title = ui.interact(
            rect(20., 0., w - 160., 24.),
            Id::unique("title-drag"),
            Sense::click_and_drag(),
        );
        if title.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
        if title.double_clicked() {
            let maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx()
                .send_viewport_cmd(egui::ViewportCommand::Maximized(!maximized));
        }
        for (i, name) in ["—", "□", "×"].iter().enumerate() {
            let r = rect(w - 138. + i as f32 * 46., 0., 46., 24.);
            let b = ui.interact(r, Id::unique(("win", i)), Sense::click());
            if b.hovered() {
                p.rect_filled(
                    r,
                    0.,
                    if i == 2 {
                        C::from_rgb(183, 55, 64)
                    } else {
                        BORDER
                    },
                );
            }
            centered(&p, r, *name, 15., TEXT);
            if b.clicked() {
                ui.ctx().send_viewport_cmd(match i {
                    0 => egui::ViewportCommand::Minimized(true),
                    1 => egui::ViewportCommand::Maximized(
                        !ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false)),
                    ),
                    _ => egui::ViewportCommand::Close,
                });
            }
        }
        p.rect_filled(rect(0., 24., w, 31.), 0., QUICK);
        if tool_button(
            ui,
            rect(9., 28., 27., 24.),
            Icon::Home,
            "Home",
            self.home,
            true,
        ) {
            self.home = !self.home;
        }
        if tool_button(
            ui,
            rect(40., 28., 23., 24.),
            Icon::Grid,
            "Data Panel",
            false,
            true,
        ) {
            self.menu = Some(("File", pos2(38., 55.)));
        }
        if tool_button(
            ui,
            rect(67., 28., 27., 24.),
            Icon::File,
            "File",
            false,
            true,
        ) {
            self.menu = Some(("File", pos2(65., 55.)));
        }
        arrow(&p, pos2(96., 39.), MUTED, true);
        if tool_button(
            ui,
            rect(108., 28., 24., 24.),
            Icon::Save,
            "Save (Ctrl+S)",
            false,
            true,
        ) {
            self.save();
        }
        if tool_button(
            ui,
            rect(137., 28., 23., 24.),
            Icon::Undo,
            "Undo (Ctrl+Z)",
            false,
            self.can_undo || self.mode == Mode::Sketch,
        ) {
            self.undo();
        }
        arrow(&p, pos2(164., 39.), MUTED, true);
        if tool_button(
            ui,
            rect(177., 28., 23., 24.),
            Icon::Redo,
            "Redo (Ctrl+Y)",
            false,
            self.can_redo,
        ) {
            self.redo();
        }
        arrow(&p, pos2(205., 39.), MUTED, true);
        p.rect_filled(rect(216., 27., w - 430., 28.), 3., RIBBON);
        icon(&p, pos2(w * 0.5 - 30., 41.), 15., Icon::Cube);
        label(
            &p,
            pos2(w * 0.5 - 18., 41.),
            format!(
                "{}{}",
                self.document.name,
                if self.dirty { "*" } else { "" }
            ),
            12.,
            TEXT,
        );
        label(&p, pos2(w - 231., 40.), "×", 15., MUTED);
        label(&p, pos2(w - 204., 39.), "+", 24., MUTED);
        for (i, k) in [Icon::Configure, Icon::History, Icon::Bell, Icon::Help]
            .iter()
            .enumerate()
        {
            tool_button(
                ui,
                rect(w - 183. + i as f32 * 29., 28., 23., 23.),
                *k,
                ["Application", "Activity", "Notifications", "Help"][i],
                false,
                false,
            );
        }
        p.circle_filled(pos2(w - 26., 39.), 12., C::from_rgb(78, 130, 40));
        centered(&p, rect(w - 38., 27., 24., 24.), "F", 11., C::WHITE);
    }
    fn ribbon(&mut self, ui: &mut egui::Ui, w: f32) {
        let p = ui.painter().clone();
        p.rect_filled(rect(0., 55., w, 85.), 0., RIBBON);
        if button(ui, rect(11., 65., 110., 61.), "workspace", "DESIGN") {
            self.menu = Some(("Workspace", pos2(11., 127.)));
        }
        arrow(&p, pos2(93., 96.), TEXT, true);
        let tabs = [
            ("SOLID", 140.),
            ("SURFACE", 223.),
            ("MESH", 324.),
            ("SHEET METAL", 397.),
            ("PLASTIC", 515.),
            ("MANAGE", 615.),
            ("UTILITIES", 707.),
            ("SKETCH", 815.),
        ];
        for (name, x) in tabs {
            if name == "SKETCH" && self.mode != Mode::Sketch {
                continue;
            }
            let r = rect(x, 56., if name == "SHEET METAL" { 100. } else { 70. }, 22.);
            let response = ui.interact(r, Id::unique(("tab", name)), Sense::click());
            centered(&p, r, name, 12., TEXT);
            if self.tab == name {
                p.line_segment(
                    [pos2(x + 4., 76.), pos2(x + r.width() - 6., 76.)],
                    Stroke::new(2., TEXT),
                );
            }
            if response.clicked() {
                if name == "SOLID" && self.mode == Mode::Sketch {
                    self.finish_sketch();
                } else {
                    self.tab = name;
                }
            }
        }
        let sketch = self.mode == Mode::Sketch;
        if sketch {
            for (x, k, name, tool) in [
                (134., Icon::Line, "Line (L)", Tool::Line),
                (
                    177.,
                    Icon::Rectangle,
                    "2-Point Rectangle (R)",
                    Tool::Rectangle,
                ),
                (
                    220.,
                    Icon::Circle,
                    "Center Diameter Circle (C)",
                    Tool::Circle,
                ),
                (263., Icon::Spline, "Fit Point Spline", Tool::Select),
                (306., Icon::Polygon, "Polygon", Tool::Polyline),
                (
                    349.,
                    Icon::Dimension,
                    "Sketch Dimension (D)",
                    Tool::Dimension,
                ),
            ] {
                if tool_button(
                    ui,
                    rect(x, 80., 36., 36.),
                    k,
                    name,
                    self.tool == tool && tool != Tool::Select,
                    name != "Fit Point Spline",
                ) {
                    if tool == Tool::Dimension {
                        self.dimension();
                    } else {
                        self.tool = tool;
                        self.anchor = None;
                        self.polyline.clear();
                    }
                }
            }
            for (x, k, name) in [
                (405., Icon::Fillet, "Sketch Fillet"),
                (448., Icon::Scissors, "Trim"),
                (491., Icon::Offset, "Offset"),
                (534., Icon::Spline, "Extend"),
                (586., Icon::Sketch, "Project"),
                (630., Icon::Constraint, "Horizontal / Vertical"),
                (673., Icon::Constraint, "Perpendicular"),
                (716., Icon::Circle, "Tangent"),
                (759., Icon::Equal, "Equal"),
                (802., Icon::Parallel, "Parallel"),
            ] {
                tool_button(ui, rect(x, 80., 36., 36.), k, name, false, false);
            }
            for (name, x, width) in [
                ("CREATE", 135., 258.),
                ("MODIFY", 399., 185.),
                ("CONSTRAINTS", 585., 258.),
            ] {
                self.group_label(ui, &p, name, x, width);
            }
        } else {
            if tool_button(
                ui,
                rect(134., 80., 36., 36.),
                Icon::Sketch,
                "Create Sketch",
                self.mode == Mode::PlaneChoice,
                true,
            ) {
                self.mode = Mode::PlaneChoice;
                self.extrude = None;
                self.status = "Select a plane to create a sketch".into();
            }
            if tool_button(
                ui,
                rect(177., 80., 36., 36.),
                Icon::Extrude,
                "Extrude (E)",
                self.extrude.is_some(),
                true,
            ) {
                self.start_extrude();
            }
            for (x, k, name) in [
                (220., Icon::Revolve, "Revolve"),
                (263., Icon::Hole, "Hole"),
                (306., Icon::Pattern, "Rectangular Pattern"),
                (349., Icon::Sphere, "Create Form"),
                (392., Icon::Scissors, "Sheet Metal"),
            ] {
                let enabled = matches!(name, "Revolve" | "Hole" | "Rectangular Pattern");
                if tool_button(ui, rect(x, 80., 36., 36.), k, name, false, enabled) {
                    self.start_modeling(name);
                }
            }
            for (x, k, name) in [
                (446., Icon::Extrude, "Press Pull"),
                (489., Icon::Fillet, "Fillet"),
                (532., Icon::Shell, "Shell"),
                (575., Icon::Cube, "Combine"),
                (618., Icon::Cube, "Split Body"),
                (661., Icon::Move, "Move / Copy"),
                (704., Icon::Table, "Change Parameters"),
            ] {
                if tool_button(ui, rect(x, 80., 36., 36.), k, name, false, true) {
                    self.start_modeling(name);
                }
            }
            self.group_label(ui, &p, "CREATE", 134., 304.);
            self.group_label(ui, &p, "MODIFY", 442., 305.);
        }
        let base = if sketch { 855. } else { 760. };
        for (dx, k, name) in [
            (0., Icon::Configure, "Configure"),
            (43., Icon::Table, "Configuration Table"),
            (86., Icon::Pattern, "Configurations"),
        ] {
            tool_button(ui, rect(base + dx, 80., 36., 36.), k, name, false, false);
        }
        self.group_label(ui, &p, "CONFIGURE", base - 5., 128.);
        if sketch {
            for (x, k, name) in [
                (997., Icon::Measure, "Measure"),
                (1040., Icon::Construct, "Section Analysis"),
            ] {
                if tool_button(
                    ui,
                    rect(x, 80., 36., 36.),
                    k,
                    name,
                    false,
                    name == "Measure",
                ) {
                    self.start_modeling(name);
                }
            }
            self.group_label(ui, &p, "INSPECT", 992., 92.);
            for (x, k, name) in [
                (1093., Icon::File, "Insert"),
                (1135., Icon::Insert, "Canvas"),
                (1177., Icon::Cube, "Insert Mesh"),
            ] {
                tool_button(ui, rect(x, 80., 36., 36.), k, name, false, false);
            }
            self.group_label(ui, &p, "INSERT", 1088., 137.);
            tool_button(
                ui,
                rect(1245., 80., 42., 36.),
                Icon::Assemble,
                "Assemble",
                false,
                false,
            );
            self.group_label(ui, &p, "ASSEMBLE", 1227., 100.);
        } else {
            let base = base + 165.;
            tool_button(
                ui,
                rect(base, 80., 36., 36.),
                Icon::Construct,
                "Construction Plane",
                false,
                false,
            );
            self.group_label(ui, &p, "CONSTRUCT", base - 29., 94.);
            let base = base + 77.;
            if tool_button(
                ui,
                rect(base, 80., 36., 36.),
                Icon::Measure,
                "Measure",
                false,
                true,
            ) {
                self.start_modeling("Measure");
            }
            self.group_label(ui, &p, "INSPECT", base - 12., 75.);
            let base = base + 98.;
            for (dx, k, name) in [
                (0., Icon::File, "Insert"),
                (42., Icon::Insert, "Canvas"),
                (84., Icon::Cube, "Insert Mesh"),
            ] {
                tool_button(ui, rect(base + dx, 80., 36., 36.), k, name, false, false);
            }
            self.group_label(ui, &p, "INSERT", base - 1., 121.);
            {
                let base = 1245.;
                tool_button(
                    ui,
                    rect(base, 80., 42., 36.),
                    Icon::Assemble,
                    "Assemble",
                    false,
                    false,
                );
                self.group_label(ui, &p, "ASSEMBLE", base - 18., 90.);
            }
        }
        let select_x = if sketch { w - 198. } else { 1342. };
        if tool_button(
            ui,
            rect(select_x, 79., 41., 39.),
            Icon::Select,
            "Select",
            false,
            true,
        ) {
            self.tool = Tool::Select;
            self.anchor = None;
        }
        self.group_label(ui, &p, "SELECT", select_x - 8., 65.);
        if sketch {
            if tool_button(
                ui,
                rect(w - 92., 80., 38., 36.),
                Icon::Check,
                "Finish Sketch",
                false,
                true,
            ) {
                self.finish_sketch();
            }
            centered(
                &p,
                rect(w - 135., 118., 130., 18.),
                "FINISH SKETCH ⌄",
                12.,
                TEXT,
            );
        }
        p.line_segment(
            [pos2(0., 139.), pos2(w, 139.)],
            Stroke::new(1., C::from_rgb(62, 73, 88)),
        );
    }
    fn group_label(
        &mut self,
        ui: &mut egui::Ui,
        p: &Painter,
        name: &'static str,
        x: f32,
        width: f32,
    ) {
        let r = rect(x, 118., width, 19.);
        let response = ui.interact(r, Id::unique(("group", name)), Sense::click());
        if response.hovered() {
            p.rect_filled(r, 0., C::from_rgb(67, 79, 95));
        }
        centered(p, r, name, 11.5, TEXT);
        arrow(
            p,
            r.center() + vec2(name.len() as f32 * 3.1 + 8., 0.),
            TEXT,
            true,
        );
        p.line_segment(
            [pos2(x + width + 2., 81.), pos2(x + width + 2., 132.)],
            Stroke::new(0.6, BORDER),
        );
        if response.clicked() {
            self.menu = Some((name, pos2(x, 138.)));
        }
    }
    fn browser(&mut self, ui: &mut egui::Ui, h: f32) {
        let p = ui.painter().clone();
        let bg = C::from_rgba_unmultiplied(62, 73, 87, 218);
        p.rect(
            rect(4., 143., 299., 25.),
            0.,
            bg,
            Stroke::new(0.8, BORDER),
            egui::StrokeKind::Inside,
        );
        label(&p, pos2(10., 155.), "«", 12., MUTED);
        label(&p, pos2(31., 155.), "BROWSER", 12., TEXT);
        label(&p, pos2(280., 155.), "−", 12., MUTED);
        let r = rect(16., 172., 176., 23.);
        if ui.interact(r, Id::unique("root"), Sense::click()).clicked() {
            self.browser_expanded = !self.browser_expanded;
        }
        p.rect_filled(r, 0., bg);
        arrow(&p, pos2(25., 184.), TEXT, self.browser_expanded);
        icon(&p, pos2(50., 184.), 15., Icon::Eye);
        icon(&p, pos2(76., 184.), 15., Icon::Cube);
        label(
            &p,
            pos2(96., 184.),
            if self.save_path.is_some() {
                self.document.name.as_str()
            } else {
                "(Unsaved)"
            },
            12.,
            TEXT,
        );
        p.circle_stroke(pos2(178., 184.), 5., Stroke::new(3., TEXT));
        if self.browser_expanded {
            self.browser_row(ui, 206., 1, "Document Settings", Icon::Gear, "settings");
            let mut y = 230.;
            if self.settings_expanded {
                self.browser_row(ui, y, 2, "Units: mm, g", Icon::Measure, "units");
                y += 24.;
                self.browser_row(ui, y, 2, "Part Design", Icon::Cube, "part");
                y += 24.;
            }
            self.browser_row(ui, y, 1, "Named Views", Icon::Folder, "views");
            y += 24.;
            self.browser_row(ui, y, 1, "Origin", Icon::Folder, "origin");
            y += 24.;
            if self.origin_expanded {
                for name in ["X", "Y", "Z", "XY", "XZ", "YZ"] {
                    self.browser_row(ui, y, 2, name, Icon::Construct, "plane-item");
                    y += 24.;
                }
            }
            if !self.bodies.is_empty() {
                self.browser_row(ui, y, 1, "Bodies", Icon::Folder, "bodies");
                y += 24.;
                if self.bodies_expanded {
                    for body in self.bodies.clone() {
                        self.browser_row(ui, y, 2, &body.name, Icon::Cube, "body-item");
                        y += 24.;
                    }
                }
            }
            if !self.document.sketches.is_empty() || self.draft.is_some() {
                self.browser_row(ui, y, 1, "Sketches", Icon::Folder, "sketches");
                y += 24.;
                if self.sketches_expanded {
                    let mut sketches = self.document.sketches.clone();
                    if let Some(d) = &self.draft {
                        if !sketches.iter().any(|s| s.id == d.id) {
                            sketches.push(d.clone());
                        }
                    }
                    for s in sketches {
                        let rr = rect(75., y - 10., 175., 21.);
                        if ui
                            .interact(rr, Id::unique(("sketch-row", s.id)), Sense::click())
                            .double_clicked()
                        {
                            if self.busy {
                                continue;
                            }
                            self.sketch_history = Default::default();
                            self.draft = Some(s.clone());
                            self.mode = Mode::Sketch;
                            self.camera.top = true;
                            self.camera.plane = s.plane;
                            self.tab = "SKETCH";
                        }
                        self.browser_row(ui, y, 2, &s.name, Icon::Sketch, "sketch-item");
                        y += 24.;
                    }
                }
            }
        }
        p.rect(
            rect(4., h - 80., 299., 25.),
            0.,
            bg,
            Stroke::new(0.8, BORDER),
            egui::StrokeKind::Inside,
        );
        label(&p, pos2(31., h - 67.), "COMMENTS", 12., TEXT);
        label(&p, pos2(281., h - 67.), "+", 16., MUTED);
    }
    fn browser_row(
        &mut self,
        ui: &mut egui::Ui,
        y: f32,
        indent: u32,
        text: &str,
        kind: Icon,
        key: &str,
    ) {
        let p = ui.painter().clone();
        let x = 24. + indent as f32 * 18.;
        let r = rect(
            x - 8.,
            y - 11.,
            (text.len() as f32 * 6.7 + 57.).min(253. - x),
            22.,
        );
        let response = ui.interact(r, Id::unique((key, text)), Sense::click());
        p.rect_filled(
            r,
            0.,
            if response.hovered() {
                C::from_rgba_unmultiplied(80, 94, 112, 230)
            } else {
                C::from_rgba_unmultiplied(62, 73, 87, 190)
            },
        );
        if indent == 1 {
            arrow(
                &p,
                pos2(x, y),
                TEXT,
                match key {
                    "settings" => self.settings_expanded,
                    "origin" => self.origin_expanded,
                    "sketches" => self.sketches_expanded,
                    "bodies" => self.bodies_expanded,
                    _ => false,
                },
            );
            if key != "settings" {
                icon(&p, pos2(x + 25., y), 15., Icon::Eye);
            }
            icon(
                &p,
                pos2(x + if key == "settings" { 26. } else { 51. }, y),
                16.,
                kind,
            );
            label(
                &p,
                pos2(x + if key == "settings" { 47. } else { 72. }, y),
                text,
                12.,
                TEXT,
            );
        } else {
            icon(&p, pos2(x + 20., y), 15., kind);
            label(&p, pos2(x + 42., y), text, 12., TEXT);
        }
        if response.clicked() {
            match key {
                "settings" => self.settings_expanded = !self.settings_expanded,
                "origin" => self.origin_expanded = !self.origin_expanded,
                "sketches" => self.sketches_expanded = !self.sketches_expanded,
                "bodies" => self.bodies_expanded = !self.bodies_expanded,
                _ => {}
            }
        }
    }
    fn timeline(&mut self, ui: &mut egui::Ui, w: f32, h: f32) {
        let p = ui.painter().clone();
        p.rect_filled(rect(0., h - 50., w, 50.), 0., RIBBON);
        for (i, text) in ["|◀", "◀", "▶", "▶|", "▶|"].iter().enumerate() {
            let r = rect(3. + i as f32 * 25., h - 38., 23., 26.);
            centered(&p, r, *text, 13., MUTED);
        }
        let mut x = 143.;
        let mut items: Vec<(String, bool, u64)> = self
            .document
            .sketches
            .iter()
            .map(|s| (s.name.clone(), false, s.id))
            .collect();
        items.extend(
            self.document
                .extrusions
                .iter()
                .map(|e| (e.name.clone(), true, e.id)),
        );
        if let Some(d) = &self.draft {
            if !items.iter().any(|i| i.2 == d.id) {
                items.push((d.name.clone(), false, d.id));
            }
        }
        items.sort_by_key(|i| i.2);
        items.extend(
            self.document
                .features
                .iter()
                .map(|f| (f.name.clone(), true, f.id)),
        );
        let order = self.document.feature_order().unwrap_or_default();
        items.sort_by_key(|item| {
            if item.1 {
                order
                    .iter()
                    .position(|id| *id == item.2)
                    .map(|i| i * 2 + 1)
                    .unwrap_or(usize::MAX)
            } else {
                order
                    .iter()
                    .position(|id| {
                        self.document
                            .extrusions
                            .iter()
                            .any(|e| e.id == *id && e.sketch_id == item.2)
                            || self
                                .document
                                .features
                                .iter()
                                .any(|f| f.id == *id && f.dependencies().contains(&item.2))
                    })
                    .map(|i| i * 2)
                    .unwrap_or(order.len() * 2 + 1)
            }
        });
        for (name, solid, id) in items {
            let r = rect(x, h - 38., 25., 28.);
            let response = ui.interact(r, Id::unique(("timeline", id)), Sense::click());
            if response.hovered() {
                p.rect_filled(r, 0., C::from_rgb(70, 97, 121));
            }
            icon(
                &p,
                r.center(),
                21.,
                if solid { Icon::Extrude } else { Icon::Sketch },
            );
            response
                .clone()
                .on_hover_text(format!("{name} — double-click to edit"));
            response.context_menu(|ui| {
                if !self.busy {
                    if solid {
                        let suppressed = self.document.suppressed.contains(&id);
                        if ui
                            .button(if suppressed {
                                "Unsuppress with Dependents"
                            } else {
                                "Suppress with Dependents"
                            })
                            .clicked()
                        {
                            match self.document.with_suppression(id, !suppressed) {
                                Ok(d) => self.send(Command::Replace(d)),
                                Err(e) => self.error = Some(e.to_string()),
                            }
                            ui.close();
                        }
                    }
                    if ui.button("Delete with Dependents").clicked() {
                        match self.document.without_feature(id) {
                            Ok(d) => self.send(Command::Replace(d)),
                            Err(e) => self.error = Some(e.to_string()),
                        }
                        ui.close();
                    }
                }
            });
            if response.double_clicked() {
                if solid {
                    if self.document.features.iter().any(|f| f.id == id) {
                        self.edit_modeling(id);
                        continue;
                    }
                    if let Some(e) = self.document.extrusions.iter().find(|e| e.id == id) {
                        if self.busy {
                            continue;
                        }
                        let distance = e
                            .distance_expression
                            .clone()
                            .unwrap_or_else(|| e.distance.to_string());
                        if let Some(profile) = self
                            .document
                            .sketches
                            .iter()
                            .find(|s| s.id == e.sketch_id)
                            .and_then(|s| {
                                s.profile_ids.iter().position(|id| Some(*id) == e.region_id)
                            })
                        {
                            self.selected = Some((e.sketch_id, profile));
                            self.extrude = Some(ExtrudeDialog {
                                editing: Some(e.id),
                                sketch_id: e.sketch_id,
                                profile,
                                distance: distance.clone(),
                                operation: e.operation,
                                body_id: e.body_id,
                                last_preview: String::new(),
                                input: fission180_core::expression::DraftInput::new(
                                    distance,
                                    &self.document.parameters,
                                ),
                            });
                        }
                    }
                } else if let Some(s) = self.document.sketches.iter().find(|s| s.id == id) {
                    if self.busy {
                        continue;
                    }
                    self.sketch_history = Default::default();
                    self.draft = Some(s.clone());
                    self.mode = Mode::Sketch;
                    self.camera.top = true;
                    self.camera.plane = s.plane;
                    self.tab = "SKETCH";
                }
            }
            x += 30.;
        }
        p.line_segment([pos2(x, h - 38.), pos2(x, h - 8.)], Stroke::new(3., TEXT));
        p.add(Shape::convex_polygon(
            vec![
                pos2(x - 6., h - 40.),
                pos2(x + 6., h - 40.),
                pos2(x, h - 33.),
            ],
            TEXT,
            Stroke::NONE,
        ));
        icon(&p, pos2(w - 21., h - 25.), 16., Icon::Gear);
        if self.busy {
            label(&p, pos2(w - 135., h - 25.), "Computing…", 12., MUTED);
        }
    }
    fn navigation(&mut self, ui: &mut egui::Ui, w: f32, h: f32) {
        let p = ui.painter().clone();
        let x = w * 0.5 - 120.;
        p.rect_filled(
            rect(x, h - 82., 242., 27.),
            0.,
            C::from_rgba_unmultiplied(58, 68, 83, 220),
        );
        for (i, k) in [
            Icon::Orbit,
            Icon::Fit,
            Icon::Pan,
            Icon::Zoom,
            Icon::Cube,
            Icon::Display,
            Icon::Grid,
            Icon::Table,
        ]
        .iter()
        .enumerate()
        {
            let r = rect(x + 2. + i as f32 * 29., h - 81., 22., 24.);
            if tool_button(
                ui,
                r,
                *k,
                [
                    "Orbit",
                    "Fit (F6)",
                    "Pan",
                    "Zoom",
                    "Home View",
                    "Display Settings",
                    "Grid and Snaps",
                    "Viewport",
                ][i],
                false,
                true,
            ) {
                match i {
                    1 => self.fit_view(),
                    4 => self.camera = Camera::default(),
                    6 => self.show_grid = !self.show_grid,
                    0 => self.status = "Shift + middle drag to orbit. Middle drag to pan.".into(),
                    2 => self.status = "Middle drag to pan".into(),
                    3 => self.status = "Scroll to zoom".into(),
                    _ => self.menu = Some(("Display", pos2(x, h - 200.))),
                }
            }
            if i == 0 || i >= 4 {
                arrow(&p, pos2(r.right() + 3., r.center().y), MUTED, true);
            }
        }
    }
    fn fit_view(&mut self) {
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for v in &self.mesh.vertices {
            for i in 0..3 {
                min[i] = min[i].min(v[i]);
                max[i] = max[i].max(v[i]);
            }
        }
        if min[0].is_finite() {
            let span = (max[0] - min[0])
                .max(max[1] - min[1])
                .max(max[2] - min[2])
                .max(1.);
            self.camera.scale = (420. / span) as f32;
            self.camera.pan = Vec2::ZERO;
        } else {
            self.camera = Camera::default();
        }
    }

    fn view_cube(&mut self, ui: &mut egui::Ui, w: f32) {
        let p = ui.painter().clone();
        let c = pos2(w - 61., 202.);
        let r = rect(w - 103., 160., 85., 84.);
        let response = ui.interact(r, Id::unique("viewcube"), Sense::click());
        if self.camera.top {
            p.rect(
                rect(c.x - 25., c.y - 25., 50., 50.),
                0.,
                C::from_rgb(125, 131, 139),
                Stroke::new(1., MUTED),
                egui::StrokeKind::Inside,
            );
            centered(
                &p,
                rect(c.x - 25., c.y - 25., 50., 50.),
                match self.camera.plane {
                    Plane::XY => "TOP",
                    Plane::XZ => "FRONT",
                    Plane::YZ => "RIGHT",
                },
                12.,
                C::from_rgb(70, 77, 87),
            );
        } else {
            let top = vec![
                c + vec2(-32., -17.),
                c + vec2(0., -35.),
                c + vec2(32., -17.),
                c + vec2(0., 1.),
            ];
            let left = vec![
                c + vec2(-32., -17.),
                c + vec2(0., 1.),
                c + vec2(0., 36.),
                c + vec2(-32., 17.),
            ];
            let right = vec![
                c + vec2(0., 1.),
                c + vec2(32., -17.),
                c + vec2(32., 17.),
                c + vec2(0., 36.),
            ];
            for (points, col) in [
                (top, C::from_rgb(142, 150, 162)),
                (left, C::from_rgb(123, 135, 153)),
                (right, C::from_rgb(159, 166, 177)),
            ] {
                p.add(Shape::convex_polygon(
                    points,
                    col,
                    Stroke::new(1., C::from_rgb(99, 112, 131)),
                ));
            }
            label(
                &p,
                c + vec2(-11., -19.),
                "TOP",
                10.,
                C::from_rgb(87, 96, 109),
            );
            label(
                &p,
                c + vec2(-30., 9.),
                "FRONT",
                9.,
                C::from_rgb(67, 81, 103),
            );
            label(&p, c + vec2(4., 9.), "RIGHT", 9., C::from_rgb(86, 97, 112));
        }
        label(&p, c + vec2(-40., -37.), "Z", 12., C::from_rgb(35, 39, 155));
        label(&p, c + vec2(29., 42.), "X", 12., C::from_rgb(140, 49, 66));
        label(&p, c + vec2(-22., -45.), "Y", 12., C::from_rgb(43, 114, 59));
        response
            .clone()
            .on_hover_text("Click for Top view; double-click for Home view");
        if response.double_clicked() {
            self.camera = Camera::default();
        } else if response.clicked() {
            self.camera.top = !self.camera.top;
        }
    }
    fn sketch_palette(&mut self, ui: &mut egui::Ui, w: f32, h: f32) {
        let p = ui.painter().clone();
        let x = w - 254.;
        let y = 258.;
        let feature_options = self.tool == Tool::Rectangle;
        let height = if feature_options {
            526_f32.min(h - 290.)
        } else {
            452_f32.min(h - 315.)
        };
        p.rect(
            rect(x, y, 250., height),
            0.,
            PANEL,
            Stroke::new(1., BORDER),
            egui::StrokeKind::Inside,
        );
        label(&p, pos2(x + 12., y + 12.), "−  SKETCH PALETTE", 12., TEXT);
        label(&p, pos2(w - 17., y + 12.), "»", 12., MUTED);
        p.line_segment(
            [pos2(x, y + 25.), pos2(w - 4., y + 25.)],
            Stroke::new(1., BORDER),
        );
        let mut row = y + 40.;
        if feature_options {
            label(&p, pos2(x + 12., row), "▾ Feature Options", 12., TEXT);
            row += 30.;
            label(&p, pos2(x + 12., row), "Rectangle", 12., TEXT);
            tool_button(
                ui,
                rect(x + 159., row - 13., 27., 27.),
                Icon::Rectangle,
                "2-Point Rectangle",
                true,
                true,
            );
            // Additional observed variants remain disabled until their behavior exists.
            tool_button(
                ui,
                rect(x + 190., row - 13., 27., 27.),
                Icon::Rectangle,
                "3-Point Rectangle",
                false,
                false,
            );
            tool_button(
                ui,
                rect(x + 159., row + 17., 27., 25.),
                Icon::Rectangle,
                "Center Rectangle",
                false,
                false,
            );
            p.line_segment(
                [pos2(x + 10., row + 51.), pos2(x + 236., row + 51.)],
                Stroke::new(1., BORDER),
            );
            row += 70.;
        }
        label(&p, pos2(x + 12., row), "▾ Options", 12., TEXT);
        row += 30.;
        label(&p, pos2(x + 12., row), "Linetype", 12., TEXT);
        icon(&p, pos2(x + 180., row), 17., Icon::Constraint);
        icon(&p, pos2(x + 210., row), 17., Icon::Cube);
        row += 30.;
        label(&p, pos2(x + 12., row), "Look At", 12., TEXT);
        if tool_button(
            ui,
            rect(x + 169., row - 11., 23., 22.),
            Icon::Fit,
            "Look At",
            false,
            true,
        ) {
            self.camera.top = true;
        }
        row += 30.;
        for i in 0..10 {
            let name = [
                "Sketch Grid",
                "Snap",
                "Slice",
                "Profile",
                "Points",
                "Dimensions",
                "Constraints",
                "Projected Geometries",
                "Construction Geometries",
                "3D Sketch",
            ][i];
            label(&p, pos2(x + 12., row), name, 12., TEXT);
            let mut value = match i {
                0 => self.show_grid,
                1 => self.snap,
                3 => self.show_profiles,
                4 => self.show_points,
                5 => self.show_dimensions,
                6 => self.show_constraints,
                7 | 8 => true,
                _ => false,
            };
            let hit = rect(x + 172., row - 9., 18., 18.);
            let response = ui.interact(
                hit,
                Id::unique(("sketch-option", i)),
                if matches!(i, 0 | 1 | 3 | 4 | 5 | 6) {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            let check = rect(x + 174., row - 6., 12., 12.);
            p.rect(
                check,
                0.,
                if value { TEXT } else { C::TRANSPARENT },
                Stroke::new(1., MUTED),
                egui::StrokeKind::Inside,
            );
            if value {
                p.line_segment(
                    [check.min + vec2(2., 6.), check.min + vec2(5., 9.)],
                    Stroke::new(1.6, PANEL),
                );
                p.line_segment(
                    [check.min + vec2(5., 9.), check.min + vec2(10., 2.)],
                    Stroke::new(1.6, PANEL),
                );
            }
            if response.clicked() {
                value = !value;
                match i {
                    0 => self.show_grid = value,
                    1 => self.snap = value,
                    3 => self.show_profiles = value,
                    4 => self.show_points = value,
                    5 => self.show_dimensions = value,
                    6 => self.show_constraints = value,
                    _ => {}
                }
            }
            row += 30.;
        }
        if button(
            ui,
            rect(w - 120., y + height - 30., 101., 24.),
            "finish-sketch",
            "Finish Sketch",
        ) {
            self.finish_sketch();
        }
    }
    fn extrude_palette(&mut self, ui: &mut egui::Ui, w: f32) {
        let Some(mut dialog) = self.extrude.take() else {
            return;
        };
        let p = ui.painter().clone();
        let x = w - 254.;
        let y = 258.;
        p.rect(
            rect(
                x,
                y,
                250.,
                if dialog.operation == Operation::NewBody {
                    314.
                } else {
                    344.
                },
            ),
            0.,
            PANEL,
            Stroke::new(1., BORDER),
            egui::StrokeKind::Inside,
        );
        label(&p, pos2(x + 12., y + 12.), "−  EXTRUDE", 12., TEXT);
        label(&p, pos2(w - 17., y + 12.), "»", 12., MUTED);
        p.line_segment(
            [pos2(x, y + 25.), pos2(w - 4., y + 25.)],
            Stroke::new(1., BORDER),
        );
        for (i, name) in [
            "Type",
            "Profiles",
            "Start",
            "Direction",
            "Extent Type",
            "Distance",
            "Taper Angle",
        ]
        .iter()
        .enumerate()
        {
            label(
                &p,
                pos2(x + 12., y + 40. + i as f32 * 30.),
                *name,
                12.,
                TEXT,
            );
        }
        tool_button(
            ui,
            rect(x + 92., y + 27., 27., 25.),
            Icon::Extrude,
            "Solid",
            true,
            true,
        );
        tool_button(
            ui,
            rect(x + 123., y + 27., 25., 25.),
            Icon::Shell,
            "Thin Extrude",
            false,
            false,
        );
        button(
            ui,
            rect(x + 93., y + 57., 85., 27.),
            "profiles",
            "↖ 1 selected",
        );
        for (i, text) in ["Profile Plane  ⌄", "One Side       ⌄", "Distance       ⌄"]
            .iter()
            .enumerate()
        {
            let r = rect(x + 93., y + 87. + i as f32 * 30., 125., 27.);
            p.rect_filled(r, 0., C::from_rgb(43, 54, 69));
            centered(&p, r, *text, 12., TEXT);
        }
        let distance_response = ui.put(
            rect(x + 93., y + 177., 125., 27.),
            egui::TextEdit::singleline(&mut dialog.distance)
                .font(FontId::proportional(12.))
                .margin(vec2(5., 5.)),
        );
        dialog
            .input
            .update(dialog.distance.clone(), &self.document.parameters);
        match &dialog.input.state {
            fission180_core::expression::InputState::Incomplete => {
                distance_response
                    .on_hover_text("Incomplete expression; last valid preview retained");
            }
            fission180_core::expression::InputState::Error(e) => {
                distance_response.on_hover_text(e);
            }
            fission180_core::expression::InputState::Valid(v) => {
                distance_response.on_hover_text(format!("{} mm", v.millimetres));
            }
        }
        let taper = rect(x + 93., y + 207., 125., 27.);
        p.rect_filled(taper, 0., C::from_rgb(43, 54, 69));
        label(&p, taper.left_center() + vec2(5., 0.), "0.0 deg", 12., TEXT);
        p.line_segment(
            [pos2(x + 10., y + 242.), pos2(w - 15., y + 242.)],
            Stroke::new(0.7, BORDER),
        );
        label(&p, pos2(x + 12., y + 260.), "Operation", 12., TEXT);
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(rect(x + 93., y + 247., 125., 27.)),
            |ui| {
                egui::ComboBox::from_id_salt("operation")
                    .selected_text(dialog.operation.label())
                    .width(118.)
                    .show_ui(ui, |ui| {
                        for operation in [
                            Operation::NewBody,
                            Operation::Join,
                            Operation::Cut,
                            Operation::Intersect,
                        ] {
                            if ui
                                .selectable_value(
                                    &mut dialog.operation,
                                    operation,
                                    operation.label(),
                                )
                                .changed()
                            {
                                dialog.body_id = if operation == Operation::NewBody {
                                    self.document
                                        .next_id()
                                        .ok()
                                        .and_then(|id| id.checked_add(1))
                                        .unwrap_or(0)
                                } else {
                                    self.committed_bodies.first().map(|b| b.id).unwrap_or(0)
                                };
                            }
                        }
                    });
            },
        );
        let footer = if dialog.operation == Operation::NewBody {
            282.
        } else {
            312.
        };
        if dialog.operation != Operation::NewBody {
            label(&p, pos2(x + 12., y + 290.), "Target Body", 12., TEXT);
            ui.scope_builder(
                egui::UiBuilder::new().max_rect(rect(x + 93., y + 277., 125., 27.)),
                |ui| {
                    egui::ComboBox::from_id_salt("target-body")
                        .selected_text(format!("Body{}", dialog.body_id))
                        .show_ui(ui, |ui| {
                            // Use committed body IDs, not transient preview outputs.
                            for b in &self.committed_bodies {
                                ui.selectable_value(&mut dialog.body_id, b.id, &b.name);
                            }
                        });
                },
            );
        }
        let ok = button(ui, rect(x + 115., y + footer, 45., 24.), "extrude-ok", "OK");
        let cancel = button(
            ui,
            rect(x + 172., y + footer, 64., 24.),
            "extrude-cancel",
            "Cancel",
        );
        let signature = format!(
            "{}:{:?}:{}",
            dialog.distance, dialog.operation, dialog.body_id
        );
        let changed = signature != dialog.last_preview;
        dialog.last_preview = signature;
        self.extrude = Some(dialog);
        if cancel {
            self.extrude = None;
            self.send(Command::ResetPreview);
        } else if ok {
            if let Some(d) = self.extrusion_document() {
                self.send(Command::Replace(d));
                self.extrude = None;
                self.selected = None;
                self.dirty = true;
            } else {
                self.error = Some("Enter a nonzero extrusion distance".into());
            }
        } else if changed {
            if let Some(d) = self.extrusion_document() {
                self.send(Command::Preview(d));
            }
        }
    }
    fn dimension_palette(&mut self, ui: &mut egui::Ui, w: f32) {
        if !self.show_dimension {
            return;
        }
        let p = ui.painter().clone();
        let r = rect(w - 540., 286., 260., 170.);
        p.rect(
            r,
            2.,
            PANEL,
            Stroke::new(1., BORDER),
            egui::StrokeKind::Inside,
        );
        label(
            &p,
            r.min + vec2(12., 15.),
            "EDIT SKETCH DIMENSIONS",
            12.,
            TEXT,
        );
        let selected = self
            .selected
            .and_then(|(id, index)| {
                self.draft
                    .as_ref()
                    .filter(|s| s.id == id)
                    .and_then(|s| s.profiles.get(index))
            })
            .cloned();
        match &selected {
            Some(Profile::Rectangle { .. }) => {
                label(&p, r.min + vec2(12., 55.), "Width", 12., TEXT);
                label(&p, r.min + vec2(12., 89.), "Height", 12., TEXT);
                ui.put(
                    Rect::from_min_size(r.min + vec2(95., 42.), vec2(140., 26.)),
                    egui::TextEdit::singleline(&mut self.dim_width),
                );
                ui.put(
                    Rect::from_min_size(r.min + vec2(95., 76.), vec2(140., 26.)),
                    egui::TextEdit::singleline(&mut self.dim_height),
                );
            }
            Some(Profile::Circle { .. }) => {
                label(&p, r.min + vec2(12., 55.), "Radius", 12., TEXT);
                ui.put(
                    Rect::from_min_size(r.min + vec2(95., 42.), vec2(140., 26.)),
                    egui::TextEdit::singleline(&mut self.dim_radius),
                );
            }
            _ => label(
                &p,
                r.min + vec2(12., 55.),
                "Select a rectangle or circle",
                12.,
                TEXT,
            ),
        }
        if button(
            ui,
            Rect::from_min_size(r.min + vec2(92., 128.), vec2(65., 26.)),
            "dimensions-apply",
            "Apply",
        ) {
            if let Some((_, index)) = self.selected {
                if let Some(draft) = &self.draft {
                    let text = match selected {
                        Some(Profile::Rectangle { .. }) => std::collections::BTreeMap::from([
                            ("width".into(), self.dim_width.clone()),
                            ("height".into(), self.dim_height.clone()),
                        ]),
                        Some(Profile::Circle { .. }) => std::collections::BTreeMap::from([(
                            "radius".into(),
                            self.dim_radius.clone(),
                        )]),
                        _ => Default::default(),
                    };
                    match draft.with_dimensions(
                        draft.profile_ids[index],
                        text,
                        &self.document.parameters,
                    ) {
                        Ok(candidate) => {
                            self.sketch_history.record(draft);
                            self.draft = Some(candidate);
                            self.dirty = true;
                            self.show_dimension = false;
                        }
                        Err(e) => self.error = Some(e.to_string()),
                    }
                }
            }
        }

        if button(
            ui,
            Rect::from_min_size(r.min + vec2(171., 128.), vec2(65., 26.)),
            "dimensions-cancel",
            "Cancel",
        ) {
            self.show_dimension = false;
        }
    }
    fn menu(&mut self, ui: &mut egui::Ui) {
        let Some((name, position)) = self.menu else {
            return;
        };
        let entries: Vec<(&str, &str, bool)> = match name {
            "File" => vec![
                ("New Design", "Ctrl+N", true),
                ("Open…", "Ctrl+O", true),
                ("Save", "Ctrl+S", true),
                ("Export…", "", true),
                ("Use OCCT Kernel", "", true),
                ("Use Rust Analytic Kernel", "", true),
            ],
            "CREATE" if self.mode == Mode::Sketch => vec![
                ("Line", "L", true),
                ("2-Point Rectangle", "R", true),
                ("Center Diameter Circle", "C", true),
                ("Polygon", "", true),
                ("Sketch Dimension", "D", true),
            ],
            "CREATE" => vec![
                ("Create Sketch", "", true),
                ("Extrude", "E", true),
                ("Revolve", "", true),
                ("Sweep", "", true),
                ("Loft", "", true),
                ("Hole", "H", true),
                ("Rectangular Pattern", "", true),
                ("Circular Pattern", "", true),
                ("Mirror", "", true),
            ],
            "MODIFY" => vec![
                ("Press Pull", "", true),
                ("Fillet", "", true),
                ("Chamfer", "", true),
                ("Shell", "", true),
                ("Combine", "", true),
                ("Split Body", "", true),
                ("Move / Copy", "", true),
                ("Change Parameters", "", true),
            ],
            "INSPECT" => vec![("Measure", "", true), ("Section Analysis", "", false)],
            "Workspace" => vec![
                ("Design", "", true),
                ("Manufacture", "", false),
                ("Render", "", false),
                ("Simulation", "", false),
                ("Drawing", "", false),
            ],
            "Display" => vec![
                ("Home View", "F6", true),
                ("Top View", "", true),
                ("Toggle Grid", "", true),
            ],
            _ => vec![("Not implemented yet", "", false)],
        };
        let r = Rect::from_min_size(position, vec2(245., entries.len() as f32 * 29. + 10.));
        let p = ui.painter().clone();
        p.rect(
            r,
            1.,
            PANEL,
            Stroke::new(1., BORDER),
            egui::StrokeKind::Inside,
        );
        let mut action = None;
        for (i, (label_text, shortcut, enabled)) in entries.iter().enumerate() {
            let row = rect(position.x + 4., position.y + 5. + i as f32 * 29., 237., 29.);
            let response = ui.interact(
                row,
                Id::unique(("menu-item", label_text)),
                if *enabled {
                    Sense::click()
                } else {
                    Sense::hover()
                },
            );
            if response.hovered() {
                p.rect_filled(row, 0., C::from_rgb(66, 89, 113));
            }
            label(
                &p,
                row.left_center() + vec2(12., 0.),
                *label_text,
                12.,
                if *enabled { TEXT } else { MUTED },
            );
            p.text(
                row.right_center() - vec2(12., 0.),
                Align2::RIGHT_CENTER,
                *shortcut,
                FontId::proportional(12.),
                MUTED,
            );
            if response.clicked() && *enabled {
                action = Some(*label_text);
            }
        }
        if let Some(action) = action {
            self.menu = None;
            match action {
                "New Design" => {
                    if !self.busy {
                        self.send(Command::New);
                    }
                }
                "Open…" => self.open(),
                "Save" => self.save(),
                "Export…" => self.export(),
                "Use OCCT Kernel" | "Use Rust Analytic Kernel" => {
                    if !self.busy {
                        let mut d = self.document.clone();
                        d.backend = if action == "Use OCCT Kernel" {
                            fission180_core::kernel::Backend::Occt
                        } else {
                            fission180_core::kernel::Backend::Rust
                        };
                        self.send(Command::Replace(d));
                    }
                }
                "Create Sketch" => self.mode = Mode::PlaneChoice,
                "Extrude" => self.start_extrude(),
                "Revolve"
                | "Sweep"
                | "Loft"
                | "Hole"
                | "Fillet"
                | "Chamfer"
                | "Shell"
                | "Press Pull"
                | "Move / Copy"
                | "Mirror"
                | "Rectangular Pattern"
                | "Circular Pattern"
                | "Combine"
                | "Split Body"
                | "Change Parameters"
                | "Measure" => self.start_modeling(action),
                "Line" => self.tool = Tool::Line,
                "Polygon" => self.tool = Tool::Polyline,
                "2-Point Rectangle" => self.tool = Tool::Rectangle,
                "Center Diameter Circle" => self.tool = Tool::Circle,
                "Sketch Dimension" => self.dimension(),
                "Home View" => self.camera = Camera::default(),
                "Top View" => self.camera.top = true,
                "Toggle Grid" => self.show_grid = !self.show_grid,
                _ => {}
            }
        } else if ui.input(|i| i.pointer.any_pressed())
            && ui.input(|i| i.pointer.interact_pos().is_some_and(|p| !r.contains(p)))
        {
            self.menu = None;
        }
    }

    fn viewport(&mut self, ui: &mut egui::Ui, r: Rect) {
        let painter = ui.painter().with_clip_rect(r);
        painter.rect_filled(r, 0., CANVAS);
        if self.show_grid {
            self.camera.grid(&painter, r);
        }
        self.cache.paint(
            &painter,
            r,
            self.camera,
            &self.mesh,
            self.revision,
            self.preview,
            self.selected_topology,
            self.hovered_topology,
        );
        let sketch_active = self.mode == Mode::Sketch;
        let mut sketches = self.document.sketches.clone();
        if let Some(draft) = &self.draft {
            sketches.retain(|s| s.id != draft.id);
            sketches.push(draft.clone());
        }
        for sketch in &sketches {
            let consumed = self
                .document
                .extrusions
                .iter()
                .any(|e| e.sketch_id == sketch.id);
            if consumed && !sketch_active && self.extrude.is_none() {
                continue;
            }
            for (index, profile) in sketch.profiles.iter().enumerate() {
                let selected = self.selected == Some((sketch.id, index))
                    && (self.tool == Tool::Select || self.extrude.is_some());
                self.paint_profile(&painter, r, profile, sketch, selected, false);
            }
            if let Some(graph) = &sketch.graph {
                let bound: std::collections::HashSet<_> =
                    graph.profile_entities.values().flatten().collect();
                for (id, entity) in &graph.entities {
                    if bound.contains(id) {
                        continue;
                    }
                    if let fission180_core::sketch::Entity::Line { a, b } = entity {
                        if let (Some(a), Some(b)) = (graph.points.get(a), graph.points.get(b)) {
                            let a = self
                                .camera
                                .project(sketch.plane.point(*a, sketch.origin).to_array(), r)
                                .0;
                            let b = self
                                .camera
                                .project(sketch.plane.point(*b, sketch.origin).to_array(), r)
                                .0;
                            painter.line_segment([a, b], Stroke::new(1.3, ACCENT));
                            if self.show_points {
                                painter.circle_filled(a, 2., TEXT);
                                painter.circle_filled(b, 2., TEXT);
                            }
                        }
                    }
                }
            }
        }
        let cursor = ui.input(|i| i.pointer.hover_pos());
        let interaction_blocked = self.menu.is_some()
            || self.show_dimension
            || cursor.is_some_and(|p| {
                p.x < 305.
                    || ((self.mode == Mode::Sketch
                        || self.extrude.is_some()
                        || self.modeling.is_some())
                        && p.x > r.right() - 258.)
                    || p.y > r.bottom() - 32.
                    || p.x > r.right() - 112. && p.y < 250.
            });
        let response = ui.interact(r, Id::unique("model-viewport"), Sense::click_and_drag());
        if !interaction_blocked {
            self.hovered_topology = if self.mode == Mode::Design && self.extrude.is_none() {
                cursor.and_then(|at| crate::view::pick(self.camera, r, &self.mesh, at))
            } else {
                None
            };
            if response.hovered() {
                let scroll = ui.input(|i| i.smooth_scroll_delta.y);
                if scroll != 0. {
                    self.camera.scale =
                        (self.camera.scale * (scroll * 0.0015).exp()).clamp(0.05, 300.);
                }
            }
            if response.dragged_by(egui::PointerButton::Middle) {
                let delta = ui.input(|i| i.pointer.delta());
                if ui.input(|i| i.modifiers.shift) && !sketch_active {
                    self.camera.top = false;
                    self.camera.yaw += delta.x as f64 * 0.008;
                    self.camera.elevation =
                        (self.camera.elevation + delta.y as f64 * 0.008).clamp(0.05, 1.5);
                } else {
                    self.camera.pan += delta;
                }
            }
            if sketch_active && self.tool != Tool::Select && self.tool != Tool::Dimension {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
            }
            if let Some(pos) = cursor.filter(|p| r.contains(*p)) {
                if self.mode == Mode::PlaneChoice {
                    let face = if response.clicked() {
                        crate::view::planar_surface(self.camera, r, &self.mesh, pos)
                    } else {
                        None
                    };
                    if let Some((plane, origin, body_id)) = face {
                        let axis = match plane {
                            Plane::XY => fission180_core::features::Axis::Z,
                            Plane::XZ => fission180_core::features::Axis::Y,
                            Plane::YZ => fission180_core::features::Axis::X,
                        };
                        let index = axis.index();
                        if let Some(body) = self.committed_bodies.iter().find(|b| b.id == body_id) {
                            let positive = (body.bounds[1][index] - origin[index]).abs()
                                < (body.bounds[0][index] - origin[index]).abs();
                            if (body.bounds[usize::from(positive)][index] - origin[index]).abs()
                                < 1e-5
                            {
                                self.begin_sketch(plane);
                                if let Some(s) = &mut self.draft {
                                    s.origin = origin;
                                    s.support = Some(fission180_core::features::PlanarSupport {
                                        body_id,
                                        axis,
                                        positive,
                                        offset: 0.,
                                    });
                                }
                            } else {
                                self.error = Some(
                                    "Choose a principal planar face at the body boundary".into(),
                                );
                            }
                        }
                    } else {
                        self.planes(ui, r, &painter, pos, response.clicked());
                    }
                } else if sketch_active {
                    let mut local = self.camera.unproject(pos, r);
                    if self.snap {
                        local = local.map(|n| (n / 1.).round() * 1.);
                    }
                    if let Some(anchor) = self.anchor {
                        if self.tool == Tool::Line {
                            if let Some(sketch) = &self.draft {
                                let a = self
                                    .camera
                                    .project(
                                        sketch.plane.point(anchor, sketch.origin).to_array(),
                                        r,
                                    )
                                    .0;
                                let b = self
                                    .camera
                                    .project(sketch.plane.point(local, sketch.origin).to_array(), r)
                                    .0;
                                painter.line_segment([a, b], Stroke::new(1.3, ACCENT));
                            }
                        }
                        let profile = match self.tool {
                            Tool::Rectangle => Some(Profile::Rectangle {
                                a: anchor,
                                b: local,
                            }),
                            Tool::Circle => Some(Profile::Circle {
                                center: anchor,
                                radius: (anchor[0] - local[0]).hypot(anchor[1] - local[1]),
                            }),
                            _ => None,
                        };
                        if let (Some(profile), Some(sketch)) = (profile, self.draft.as_ref()) {
                            self.paint_profile(&painter, r, &profile, sketch, true, true);
                            if self.show_dimensions {
                                label(
                                    &painter,
                                    pos + vec2(14., 17.),
                                    match profile {
                                        Profile::Rectangle { a, b } => format!(
                                            "{:.2} × {:.2} mm",
                                            (b[0] - a[0]).abs(),
                                            (b[1] - a[1]).abs()
                                        ),
                                        Profile::Circle { radius, .. } => {
                                            format!("⌀ {:.2} mm", radius * 2.)
                                        }
                                        _ => String::new(),
                                    },
                                    12.,
                                    TEXT,
                                );
                            }
                        }
                    }
                    if self.tool == Tool::Polyline {
                        if let Some(sketch) = self.draft.as_ref() {
                            let mut points = self.polyline.clone();
                            points.push(local);
                            let points: Vec<_> = points
                                .iter()
                                .map(|p| {
                                    self.camera
                                        .project(
                                            sketch.plane.point(*p, sketch.origin).to_array(),
                                            r,
                                        )
                                        .0
                                })
                                .collect();
                            if points.len() > 1 {
                                painter.add(Shape::line(points, Stroke::new(1.3, ACCENT)));
                            }
                        }
                    }
                    if response.clicked() {
                        match self.tool {
                            Tool::Rectangle | Tool::Circle => {
                                if let Some(a) = self.anchor.take() {
                                    let p = if self.tool == Tool::Rectangle {
                                        Profile::Rectangle { a, b: local }
                                    } else {
                                        Profile::Circle {
                                            center: a,
                                            radius: (a[0] - local[0]).hypot(a[1] - local[1]),
                                        }
                                    };
                                    self.add_profile(p);
                                } else {
                                    self.anchor = Some(local);
                                }
                            }
                            Tool::Polyline => {
                                if self.polyline.len() >= 3
                                    && (local[0] - self.polyline[0][0])
                                        .hypot(local[1] - self.polyline[0][1])
                                        < 5. / self.camera.scale as f64
                                {
                                    let points = std::mem::take(&mut self.polyline);
                                    self.add_profile(Profile::Polygon { points });
                                } else {
                                    self.polyline.push(local);
                                }
                            }
                            Tool::Line => {
                                if let Some(a) = self.anchor {
                                    if let Some(draft) = &self.draft {
                                        match draft.with_line(a, local, &self.document.parameters) {
                                            Ok(candidate) => {
                                                self.sketch_history.record(draft);
                                                self.draft = Some(candidate);
                                                self.dirty = true;
                                                self.anchor = Some(local);
                                            }
                                            Err(e) => self.error = Some(e.to_string()),
                                        }
                                    }
                                } else {
                                    self.anchor = Some(local);
                                }
                            }
                            Tool::Select | Tool::Dimension => {
                                self.pick_profile(pos, r, &sketches);
                                if self.tool == Tool::Dimension {
                                    self.dimension();
                                }
                            }
                        }
                    }
                } else if response.clicked() {
                    self.selected_topology = self.hovered_topology;
                    if let Some(pick) = self.selected_topology {
                        self.status = match pick {
                            Pick::Face(_) => "Face selected",
                            Pick::Edge(_) => "Edge selected",
                            Pick::Body(_) => "Body selected",
                        }
                        .into();
                    } else {
                        self.pick_profile(pos, r, &sketches);
                    }
                }
            }
        } else if self.mode == Mode::PlaneChoice {
            self.planes(ui, r, &painter, Pos2::ZERO, false);
        }
        if self.mode == Mode::Sketch {
            let o = self.camera.origin(r);
            painter.circle_stroke(o, 7., Stroke::new(1.5, C::from_rgb(37, 42, 51)));
            painter.circle_filled(o, 2., C::from_rgb(106, 86, 128));
        }
        let hint = if self.mode == Mode::PlaneChoice {
            "Select a plane or planar face"
        } else if self.mode == Mode::Sketch {
            match self.tool {
                Tool::Rectangle => {
                    if self.anchor.is_some() {
                        "Place opposite corner"
                    } else {
                        "Place first corner"
                    }
                }
                Tool::Circle => {
                    if self.anchor.is_some() {
                        "Specify circle radius"
                    } else {
                        "Place center point"
                    }
                }
                Tool::Polyline => "Click vertices; click the first point or press Enter to close",
                Tool::Line => "Click endpoints; Escape ends the line chain",
                _ => "",
            }
        } else {
            ""
        };
        if !hint.is_empty() {
            label(
                &painter,
                pos2(r.left() + 315., r.bottom() - 16.),
                hint,
                12.,
                TEXT,
            );
        }
    }
    fn planes(&mut self, _ui: &mut egui::Ui, r: Rect, p: &Painter, cursor: Pos2, clicked: bool) {
        for (plane, color) in [
            (Plane::YZ, C::from_rgba_unmultiplied(204, 156, 110, 95)),
            (Plane::XZ, C::from_rgba_unmultiplied(210, 163, 115, 115)),
            (Plane::XY, C::from_rgba_unmultiplied(209, 164, 117, 90)),
        ] {
            let points: Vec<_> = [[0., 0.], [28., 0.], [28., 28.], [0., 28.]]
                .iter()
                .map(|v| {
                    self.camera
                        .project(plane.point(*v, [0.; 3]).to_array(), r)
                        .0
                })
                .collect();
            let hovered = inside(cursor, &points);
            p.add(Shape::convex_polygon(
                points,
                if hovered {
                    C::from_rgba_unmultiplied(228, 187, 134, 160)
                } else {
                    color
                },
                Stroke::new(1., C::from_rgb(178, 149, 123)),
            ));
            if clicked && hovered {
                self.begin_sketch(plane);
                break;
            }
        }
    }
    fn paint_profile(
        &self,
        p: &Painter,
        r: Rect,
        profile: &Profile,
        sketch: &Sketch,
        selected: bool,
        preview: bool,
    ) {
        let points: Vec<_> = profile
            .outline(96)
            .iter()
            .map(|v| {
                self.camera
                    .project(sketch.plane.point(*v, sketch.origin).to_array(), r)
                    .0
            })
            .collect();
        if points.len() < 2 {
            return;
        }
        let col = if selected {
            C::from_rgb(90, 174, 222)
        } else {
            C::from_rgb(66, 153, 192)
        };
        if self.show_profiles && !matches!(profile, Profile::Polygon { .. }) {
            p.add(Shape::convex_polygon(
                points.clone(),
                if selected {
                    C::from_rgba_unmultiplied(67, 131, 213, 140)
                } else {
                    C::from_rgba_unmultiplied(64, 102, 123, 83)
                },
                Stroke::NONE,
            ));
        }
        let mut path = points.clone();
        path.push(path[0]);
        p.add(Shape::line(
            path,
            Stroke::new(if selected { 1.7 } else { 1.1 }, col),
        ));
        if self.show_points
            && self.mode == Mode::Sketch
            && !matches!(profile, Profile::Circle { .. })
        {
            for point in &points {
                p.circle_filled(*point, 2., C::from_rgb(93, 159, 187));
            }
        }
        if self.show_constraints
            && self.mode == Mode::Sketch
            && !preview
            && sketch.graph.as_ref().is_some_and(|g| {
                g.constraints.values().any(|c| {
                    matches!(
                        c,
                        fission180_core::sketch::Constraint::Horizontal { .. }
                            | fission180_core::sketch::Constraint::Vertical { .. }
                    )
                })
            })
        {
            if let Profile::Rectangle { .. } = profile {
                for i in 0..4 {
                    let mid = points[i] + (points[(i + 1) % 4] - points[i]) * 0.5;
                    let offset = if i % 2 == 0 {
                        vec2(0., 8.)
                    } else {
                        vec2(8., 0.)
                    };
                    label(p, mid + offset, "≡", 12., MUTED);
                }
            }
        }
    }
    fn pick_profile(&mut self, pos: Pos2, r: Rect, sketches: &[Sketch]) {
        self.selected = None;
        for s in sketches.iter().rev() {
            for (i, profile) in s.profiles.iter().enumerate().rev() {
                let points: Vec<_> = profile
                    .outline(64)
                    .iter()
                    .map(|v| {
                        self.camera
                            .project(s.plane.point(*v, s.origin).to_array(), r)
                            .0
                    })
                    .collect();
                if inside(pos, &points) {
                    self.selected = Some((s.id, i));
                    return;
                }
            }
        }
    }
    fn home(&mut self, ui: &mut egui::Ui, w: f32, h: f32) {
        let p = ui.painter().clone();
        p.rect_filled(rect(0., 55., w, h - 55.), 0., C::from_rgb(69, 78, 97));
        p.rect_filled(rect(0., 55., 238., h - 55.), 0., CANVAS);
        label(&p, pos2(22., 113.), "Fission 180", 24., TEXT);
        label(&p, pos2(25., 161.), "Local workspace", 12., TEXT);
        for (i, text) in ["Open…", "New…", "My Designs", "Projects", "Samples"]
            .iter()
            .enumerate()
        {
            if button(
                ui,
                rect(24., 210. + i as f32 * 39., 190., 26.),
                ("home", i),
                text,
            ) {
                if i == 0 {
                    self.open();
                    self.home = false;
                } else {
                    self.home = false;
                }
            }
        }
        label(&p, pos2(256., 150.), "Welcome to Fission 180", 26., TEXT);
        label(&p, pos2(256., 282.), "Start working", 18., TEXT);
        if button(
            ui,
            rect(256., 306., 112., 26.),
            "new-design",
            "+ New Design",
        ) {
            self.home = false;
        }
        if button(ui, rect(382., 306., 126., 26.), "open-design", "Open…") {
            self.open();
            self.home = false;
        }
        label(&p, pos2(256., 356.), "Recent", 18., TEXT);
        p.line_segment(
            [pos2(256., 400.), pos2(w - 40., 400.)],
            Stroke::new(1., BORDER),
        );
        label(&p, pos2(280., 424.), "Name", 12., TEXT);
        label(&p, pos2(650., 424.), "Location", 12., TEXT);
        if let Some(path) = &self.save_path {
            label(&p, pos2(280., 458.), &self.document.name, 12., TEXT);
            label(&p, pos2(650., 458.), path.to_string_lossy(), 12., MUTED);
        } else {
            label(
                &p,
                pos2(280., 458.),
                "Your saved designs will appear here",
                12.,
                MUTED,
            );
        }
    }
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod input_tests;

fn inside(point: Pos2, polygon: &[Pos2]) -> bool {
    let mut inside = false;
    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        if (a.y > point.y) != (b.y > point.y)
            && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
        {
            inside = !inside;
        }
    }
    inside
}

impl eframe::App for FusionApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let responses: Vec<_> = self.worker.poll().collect();
        for response in responses {
            if std::env::var_os("FISSION180_TIMINGS").is_some()
                || std::env::var_os("OPENFUSION_TIMINGS").is_some()
            {
                eprintln!("geometry: {:.3} ms", response.elapsed_ms);
            }
            self.busy = response.id != self.worker.latest;
            let kind = response.kind;
            if matches!(
                kind,
                crate::worker::ResponseKind::Commit | crate::worker::ResponseKind::Loaded
            ) {
                self.document = response.document;
                self.mode = Mode::Design;
                self.draft = None;
                if kind == crate::worker::ResponseKind::Loaded {
                    self.camera = Camera::default();
                    self.selected = None;
                    self.extrude = None;
                    self.modeling = None;
                    self.selected_topology = None;
                    self.hovered_topology = None;
                    self.sketch_history = Default::default();
                }
            }
            if let Some((mesh, bodies)) = response.display {
                self.mesh = mesh;
                if matches!(
                    kind,
                    crate::worker::ResponseKind::Commit | crate::worker::ResponseKind::Loaded
                ) {
                    self.committed_bodies = bodies.clone();
                }
                self.bodies = bodies;
                self.preview = kind == crate::worker::ResponseKind::Preview;
                self.revision += 1;
            }
            self.save_path = response.save_path;
            self.dirty = response.dirty || (self.mode == Mode::Sketch && self.draft.is_some());
            self.can_undo = response.can_undo;
            self.can_redo = response.can_redo;
            self.error = response.error;
            self.status = if self.error.is_none() && !self.bodies.is_empty() {
                format!(
                    "{} {} | Volume: {:.2} mm³",
                    self.bodies.len(),
                    if self.bodies.len() == 1 {
                        "Body"
                    } else {
                        "Bodies"
                    },
                    self.bodies.iter().map(|b| b.volume).sum::<f64>()
                )
            } else {
                String::new()
            };
            if self.pending_extrude && !self.busy {
                self.pending_extrude = false;
                if self.error.is_none() {
                    self.start_extrude();
                }
            }
            if !self.busy {
                if let Some(name) = self.pending_modeling.take() {
                    if self.error.is_none() {
                        self.start_modeling(&name);
                    }
                }
            }
        }
        self.shortcuts(ui.ctx());
        let r = ui.max_rect();
        let (w, h) = (r.width(), r.height());
        ui.set_clip_rect(r);
        if self.home {
            self.home(ui, w, h);
        } else {
            self.viewport(ui, rect(0., 140., w, h - 190.));
        }
        self.chrome(ui, w);
        if !self.home {
            self.ribbon(ui, w);
            self.browser(ui, h);
            self.view_cube(ui, w);
            self.navigation(ui, w, h);
            self.timeline(ui, w, h);
            if self.mode == Mode::Sketch {
                self.sketch_palette(ui, w, h);
            }
            if self.extrude.is_some() {
                self.extrude_palette(ui, w);
            }
            if self.modeling.is_some() {
                self.modeling_palette(ui, w);
            }
            self.dimension_palette(ui, w);
        }
        self.menu(ui);
        if let Some(error) = &self.error {
            let p = ui.painter();
            let rr = rect(w * 0.5 - 240., h - 119., 480., 30.);
            p.rect_filled(rr, 3., C::from_rgb(102, 49, 57));
            centered(p, rr, error, 12., TEXT);
        } else if !self.status.is_empty() && !self.home {
            ui.painter().text(
                pos2(w - 20., h - 68.),
                Align2::RIGHT_CENTER,
                &self.status,
                FontId::proportional(12.),
                MUTED,
            );
        }
        if self.capture.is_some() {
            self.capture_frames = self.capture_frames.saturating_add(1);
            ui.ctx().request_repaint();
            if self.capture_frames >= 8 && !self.busy {
                if let Some(path) = self.capture.take() {
                    let ctx = ui.ctx().clone();
                    let close = self.close_after_capture;
                    ui.ctx().request_screenshot(move |image| {
                        if let Some(parent) = path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        let data: Vec<u8> =
                            image.pixels.iter().flat_map(|p| p.to_array()).collect();
                        let _ = image::save_buffer(
                            &path,
                            &data,
                            image.width() as u32,
                            image.height() as u32,
                            image::ColorType::Rgba8,
                        );
                        if close {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                        ctx.request_repaint();
                    });
                }
            }
        }
    }
}
