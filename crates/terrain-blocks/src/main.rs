//! terrain-blocks <crate dir>: the crate's structure as blocks on a 2D canvas.
//! A thin egui layer on the Workspace; checked live with the mouse and with hands
//! through terrain-desktop.

use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Color32, Pos2, Rect, RichText, Sense, pos2, vec2};
use terrain_blocks::{Block, Id, MoveError, Workspace, cargo_check};

const FILE_WIDTH: f32 = 340.0;
/// Label size; with the frame margins a block's header row is about 32 px, enough
/// to hit with a hand pointer.
const LABEL_SIZE: f32 = 16.0;
const POLL: Duration = Duration::from_secs(1);

fn main() -> eframe::Result {
    let root = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("."), PathBuf::from);
    let ws = Workspace::open(&root).unwrap_or_else(|e| {
        eprintln!("terrain-blocks: can't open {}: {e}", root.display());
        std::process::exit(1);
    });
    eframe::run_native(
        "terrain-blocks",
        eframe::NativeOptions::default(),
        Box::new(|_| {
            let mut app = App {
                ws,
                scene: Rect::ZERO,
                selected: None,
                dragging: None,
                file_drag: None,
                containers: Vec::new(),
                message: String::new(),
                check: Check::Running,
                check_rx: None,
                last_poll: Instant::now(),
            };
            app.start_check();
            Ok(Box::new(app))
        }),
    )
}

enum Check {
    Running,
    Pass,
    Fail,
}

struct App {
    ws: Workspace,
    /// The part of the canvas in view; zero until the first frame fits it.
    scene: Rect,
    selected: Option<Id>,
    dragging: Option<Id>,
    /// Where the file block being dragged is, until it is let go and saved.
    file_drag: Option<Pos2>,
    /// Drop targets drawn this frame, outermost first: id, rect, centre y of each child.
    containers: Vec<(Id, Rect, Vec<f32>)>,
    message: String,
    check: Check,
    check_rx: Option<mpsc::Receiver<bool>>,
    last_poll: Instant,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if let Some(Ok(ok)) = self.check_rx.as_ref().map(|rx| rx.try_recv()) {
            self.check = if ok { Check::Pass } else { Check::Fail };
            self.check_rx = None;
        }
        if self.last_poll.elapsed() >= POLL {
            self.last_poll = Instant::now();
            match self.ws.refresh() {
                Ok(changed) if !changed.is_empty() => {
                    self.selected = None;
                    self.start_check();
                }
                Ok(_) => {}
                Err(e) => self.message = format!("refresh failed: {e}"),
            }
        }
        // Keeps the poll and the check badge ticking while idle.
        ui.ctx().request_repaint_after(Duration::from_millis(250));

        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z)) {
            self.undo();
        }
        let zoom = ui.input(|i| {
            if i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals) {
                1.25
            } else if i.key_pressed(egui::Key::Minus) {
                0.8
            } else {
                1.0
            }
        });
        if zoom != 1.0 && self.scene.is_positive() {
            self.scene = Rect::from_center_size(self.scene.center(), self.scene.size() / zoom);
        }

        egui::Panel::top("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                let (badge, color) = match self.check {
                    Check::Running => ("⏳ checking", Color32::GRAY),
                    Check::Pass => ("✔ cargo check", Color32::from_rgb(80, 180, 100)),
                    Check::Fail => ("✖ cargo check", Color32::from_rgb(220, 80, 80)),
                };
                ui.label(RichText::new(badge).size(LABEL_SIZE).color(color));
                ui.separator();
                ui.label(self.ws.root().display().to_string());
                ui.separator();
                ui.label(&self.message);
            });
        });
        if let Some(text) = self.selected.as_ref().and_then(|id| self.ws.text(id)) {
            egui::Panel::right("code")
                .default_size(420.0)
                .show(ui, |ui| {
                    egui::ScrollArea::both().show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(&mut &*text)
                                .code_editor()
                                .desired_width(f32::INFINITY),
                        );
                    });
                });
        }
        egui::CentralPanel::default().show(ui, |ui| {
            let mut scene_rect = self.scene;
            egui::Scene::new()
                .zoom_range(0.1..=3.0)
                .show(ui, &mut scene_rect, |ui| self.canvas(ui));
            self.scene = scene_rect;
        });
    }
}

impl App {
    fn canvas(&mut self, ui: &mut egui::Ui) {
        self.containers.clear();
        let to_local = ui
            .ctx()
            .layer_transform_from_global(ui.layer_id())
            .unwrap_or_default();
        let pointer = ui.ctx().pointer_latest_pos().map(|p| to_local * p);
        let files = self.ws.files().to_vec();
        let mut dropped = None;
        for (i, file) in files.iter().enumerate() {
            let id = Id(vec![i]);
            let pos = match (self.dragging.as_ref() == Some(&id), self.file_drag) {
                (true, Some(p)) => p,
                _ => self
                    .ws
                    .position(&id)
                    .map_or(pos2(i as f32 * (FILE_WIDTH + 40.0), 0.0), |[x, y]| {
                        pos2(x, y)
                    }),
            };
            let max = Rect::from_min_size(pos, vec2(FILE_WIDTH, f32::INFINITY));
            ui.scope_builder(egui::UiBuilder::new().max_rect(max), |ui| {
                self.block(ui, file, id, &mut dropped);
            });
        }

        let (Some(dragging), Some(p)) = (self.dragging.clone(), pointer) else {
            return;
        };
        if dragging.0.len() > 1 {
            if let Some((into, index, y, rect)) = self.drop_target(&dragging, p) {
                let x = rect.x_range().shrink(6.0);
                ui.painter().hline(
                    x,
                    y,
                    egui::Stroke::new(3.0, Color32::from_rgb(90, 160, 255)),
                );
                if dropped.as_ref() == Some(&dragging) {
                    self.drop(dragging.clone(), into, index);
                }
            }
            let label = self.ws.block(&dragging).map(label).unwrap_or_default();
            ui.painter().text(
                p + vec2(14.0, 0.0),
                egui::Align2::LEFT_CENTER,
                label,
                egui::FontId::proportional(LABEL_SIZE),
                ui.visuals().strong_text_color(),
            );
        }
        if dropped.is_some() {
            if let (1, Some(at)) = (dragging.0.len(), self.file_drag)
                && let Err(e) = self.ws.place(&dragging, [at.x, at.y]) {
                    self.message = format!("couldn't save the layout: {e}");
                }
            self.dragging = None;
            self.file_drag = None;
        }
    }

    /// Draws `b` and its stack; returns its rect.
    fn block(&mut self, ui: &mut egui::Ui, b: &Block, id: Id, dropped: &mut Option<Id>) -> Rect {
        let frame = egui::Frame::group(ui.style())
            .fill(fill(b.kind, ui.visuals().dark_mode))
            .inner_margin(6.0);
        let inner = frame.show(ui, |ui| {
            ui.set_width(ui.available_width());
            let color = if b.error {
                Color32::from_rgb(220, 80, 80)
            } else {
                ui.visuals().strong_text_color()
            };
            let head = ui
                .add(
                    egui::Label::new(RichText::new(label(b)).size(LABEL_SIZE).color(color))
                        .sense(Sense::click_and_drag())
                        .selectable(false),
                )
                .on_hover_cursor(egui::CursorIcon::Grab);
            if b.error {
                ui.label("can't parse: left untouched");
            }
            let centers: Vec<f32> = b
                .children
                .iter()
                .enumerate()
                .map(|(j, child)| {
                    let mut child_id = id.0.clone();
                    child_id.push(j);
                    self.block(ui, child, Id(child_id), dropped).center().y
                })
                .collect();
            (head, centers)
        });
        let (head, centers) = inner.inner;
        let rect = inner.response.rect;
        if matches!(b.kind, "file" | "impl" | "trait" | "mod") && !b.error {
            self.containers.push((id.clone(), rect, centers));
        }

        if head.drag_started() {
            self.dragging = Some(id.clone());
            self.file_drag = (b.kind == "file").then_some(rect.min);
        }
        if let (true, Some(at)) = (head.dragged(), self.file_drag.as_mut()) {
            *at += head.drag_delta();
        }
        if head.drag_stopped() {
            *dropped = Some(id.clone());
        }
        if head.clicked() {
            self.selected = Some(id.clone());
        }
        if head.secondary_clicked() {
            self.open_in_nvim(&id, b.line);
        }
        rect
    }

    /// The innermost container under `p` that `dragging` may go into, the index in its
    /// stack, the insertion line's y and the container's rect.
    fn drop_target(&self, dragging: &Id, p: Pos2) -> Option<(Id, usize, f32, Rect)> {
        let (into, rect, centers) = self
            .containers
            .iter()
            .rev()
            .find(|(id, rect, _)| rect.contains(p) && !id.0.starts_with(&dragging.0))?;
        let index = centers.iter().filter(|&&y| y < p.y).count();
        let y = match (index.checked_sub(1).map(|i| centers[i]), centers.get(index)) {
            (Some(a), Some(b)) => (a + b) / 2.0,
            (None, Some(b)) => b - 12.0,
            (Some(_), None) => rect.bottom() - 4.0,
            (None, None) => rect.top() + 36.0,
        };
        Some((into.clone(), index, y, *rect))
    }

    fn drop(&mut self, block: Id, into: Id, index: usize) {
        let (parent, own) = block.0.split_at(block.0.len() - 1);
        if parent == into.0 && (index == own[0] || index == own[0] + 1) {
            return; // Dropped where it already is.
        }
        match self.ws.move_block(&block, &into, index) {
            Ok(()) => {
                self.selected = None;
                self.message = "moved".into();
                self.start_check();
            }
            Err(e) => self.refused(e),
        }
    }

    fn undo(&mut self) {
        match self.ws.undo() {
            Ok(true) => {
                self.selected = None;
                self.message = "undone".into();
                self.start_check();
            }
            Ok(false) => self.message = "nothing to undo".into(),
            Err(e) => self.refused(e),
        }
    }

    fn refused(&mut self, e: MoveError) {
        self.message = match e {
            MoveError::Stale(files) => {
                // Pick up what changed so the next try works on the new text.
                let _ = self.ws.refresh();
                format!(
                    "refused: {} changed on disk; re-read, try again",
                    files.join(", ")
                )
            }
            MoveError::Invalid(why) => format!("refused: {why}"),
            MoveError::Io(e) => format!("failed: {e}"),
        };
    }

    fn open_in_nvim(&mut self, id: &Id, line: usize) {
        let path = self.ws.root().join(&self.ws.files()[id.0[0]].name);
        let spawned = std::process::Command::new("xdg-terminal-exec")
            .arg("nvim")
            .arg(format!("+{line}"))
            .arg(&path)
            .spawn();
        if let Err(e) = spawned {
            self.message = format!("couldn't open nvim: {e}");
        }
    }

    fn start_check(&mut self) {
        let root = self.ws.root().to_path_buf();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(cargo_check(&root).unwrap_or(false));
        });
        // Dropping an older receiver discards a check that is now out of date.
        self.check_rx = Some(rx);
        self.check = Check::Running;
    }
}

fn label(b: &Block) -> String {
    if b.kind == "file" {
        b.name.clone()
    } else {
        format!("{} {}", b.kind, b.name)
    }
}

fn fill(kind: &str, dark: bool) -> Color32 {
    let (r, g, b) = match kind {
        "file" => (120, 120, 130),
        "fn" => (70, 130, 200),
        "struct" | "enum" | "type" => (190, 140, 60),
        "impl" | "trait" => (140, 90, 190),
        "mod" => (70, 160, 120),
        _ => (150, 150, 150),
    };
    Color32::from_rgba_unmultiplied(r, g, b, if dark { 60 } else { 45 })
}
