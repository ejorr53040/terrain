//! terrain-blocks <crate dir>: the crate's infrastructure and structure as blocks on
//! a 2D canvas. A thin egui layer on the Workspace; checked live with the mouse and
//! with hands through terrain-desktop.

mod files_view;
mod functionality_view;
mod look;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use eframe::egui::{self, Pos2, Rect, RichText, pos2, vec2};
use terrain_blocks::{Id, ItemRef, MoveError, System, Workspace, cargo_check};

const FILE_WIDTH: f32 = 340.0;
const SYSTEM_WIDTH: f32 = 440.0;
/// Space between blocks laid out by default.
const GAP: f32 = 40.0;
/// Label size; with the frame margins a block's header row is about 36 px, enough
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
        Box::new(|cc| {
            look::install(&cc.egui_ctx);
            let systems = ws.systems();
            let view = if systems.is_empty() {
                View::Files
            } else {
                View::Functionality
            };
            let mut app = App {
                ws,
                systems,
                view,
                // The first three columns of each view's default layout.
                scenes: [
                    Rect::from_min_size(
                        pos2(-GAP, -GAP),
                        vec2(3.0 * (SYSTEM_WIDTH + GAP) + GAP, 600.0),
                    ),
                    Rect::from_min_size(
                        pos2(-GAP, -GAP),
                        vec2(3.0 * (FILE_WIDTH + GAP) + GAP, 600.0),
                    ),
                ],
                selected: None,
                dragging: None,
                file_drag: None,
                containers: Vec::new(),
                item_drag: None,
                system_drag: None,
                system_rects: HashMap::new(),
                renaming: None,
                expanded: HashSet::new(),
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

#[derive(Clone, Copy, PartialEq)]
enum View {
    Functionality = 0,
    Files = 1,
}

enum Check {
    Running,
    Pass,
    Fail,
}

struct App {
    ws: Workspace,
    /// The Functionality view's model, rebuilt after every change.
    systems: Vec<System>,
    view: View,
    /// The part of the canvas in view, per view.
    scenes: [Rect; 2],
    /// The block whose code shows in the side panel.
    selected: Option<Id>,

    // Files view.
    dragging: Option<Id>,
    /// Where the file block being dragged is, until it is let go and saved.
    file_drag: Option<Pos2>,
    /// Drop targets drawn this frame, outermost first: id, rect, centre y of each child.
    containers: Vec<(Id, Rect, Vec<f32>)>,

    // Functionality view.
    /// The item being dragged and its System.
    item_drag: Option<(ItemRef, String)>,
    /// The System being dragged and where it is, until it is let go and saved.
    system_drag: Option<(String, Pos2)>,
    /// Each System's rect last frame, for the lines between Systems.
    system_rects: HashMap<String, Rect>,
    /// The Capability being renamed, and the name so far.
    renaming: Option<(usize, String)>,
    /// Systems whose Unsorted tray is open.
    expanded: HashSet<String>,

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
                    self.reload_systems();
                    self.start_check();
                }
                Ok(_) => {}
                Err(e) => self.message = format!("refresh failed: {e}"),
            }
        }
        // Keeps the poll and the check badge ticking while idle.
        ui.ctx().request_repaint_after(Duration::from_millis(250));

        // Ctrl+Z undoes code moves; while renaming it belongs to the text field.
        if self.renaming.is_none()
            && ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::Z))
        {
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
        let scene = &mut self.scenes[self.view as usize];
        if zoom != 1.0 && self.renaming.is_none() && scene.is_positive() {
            *scene = Rect::from_center_size(scene.center(), scene.size() / zoom);
        }

        let pal = look::palette(ui);
        egui::Panel::top("status").show(ui, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                for (view, name) in [
                    (View::Functionality, "Functionality"),
                    (View::Files, "Files"),
                ] {
                    ui.selectable_value(
                        &mut self.view,
                        view,
                        RichText::new(name).font(look::bold(15.0)),
                    );
                }
                ui.separator();
                let (badge, color) = match self.check {
                    Check::Running => ("⏳ checking", pal.dim),
                    Check::Pass => ("✔ cargo check", pal.ok),
                    Check::Fail => ("✖ cargo check", pal.err),
                };
                ui.label(
                    RichText::new(badge)
                        .font(look::bold(LABEL_SIZE))
                        .color(color),
                );
                ui.separator();
                ui.label(RichText::new(self.ws.root().display().to_string()).color(pal.dim));
                ui.separator();
                ui.label(&self.message);
            });
            ui.add_space(4.0);
        });
        if let Some(text) = self.selected.as_ref().and_then(|id| self.ws.text(id)) {
            egui::Panel::right("code")
                .default_size(460.0)
                .show(ui, |ui| {
                    ui.label(
                        RichText::new("read-only · right-click a block for nvim").color(pal.dim),
                    );
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
            let view = self.view;
            let mut scene_rect = self.scenes[view as usize];
            egui::Scene::new()
                .zoom_range(0.1..=3.0)
                .show(ui, &mut scene_rect, |ui| match view {
                    View::Functionality => self.functionality_canvas(ui),
                    View::Files => self.files_canvas(ui),
                });
            self.scenes[view as usize] = scene_rect;
        });
    }
}

impl App {
    /// The pointer in the canvas's own coordinates.
    fn pointer(&self, ui: &egui::Ui) -> Option<Pos2> {
        let to_local = ui
            .ctx()
            .layer_transform_from_global(ui.layer_id())
            .unwrap_or_default();
        ui.ctx().pointer_latest_pos().map(|p| to_local * p)
    }

    fn reload_systems(&mut self) {
        self.systems = self.ws.systems();
    }

    /// After a code move or undo: the files changed under every view.
    fn code_changed(&mut self, message: &str) {
        self.selected = None;
        self.message = message.into();
        self.reload_systems();
        self.start_check();
    }

    fn undo(&mut self) {
        match self.ws.undo() {
            Ok(true) => self.code_changed("undone"),
            Ok(false) => self.message = "nothing to undo".into(),
            Err(e) => self.refused(e),
        }
    }

    fn refused(&mut self, e: MoveError) {
        self.message = match e {
            MoveError::Stale(files) => {
                // Pick up what changed so the next try works on the new text.
                let _ = self.ws.refresh();
                self.reload_systems();
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

/// Where blocks with no place in the sidecar go: each into the shortest column of
/// a grid about as wide as it is tall, in order.
// ponytail: Files view heights are estimates (wrapped labels run taller); measure
// last frame's rects as the Functionality view does if file columns overlap.
fn masonry(heights: &[f32], width: f32) -> Vec<Pos2> {
    let columns = (heights.len() as f32).sqrt().ceil().max(1.0) as usize;
    let mut bottoms = vec![0.0_f32; columns];
    heights
        .iter()
        .map(|h| {
            let c = (0..columns)
                .min_by(|&a, &b| bottoms[a].total_cmp(&bottoms[b]))
                .unwrap();
            let at = pos2(c as f32 * (width + GAP), bottoms[c]);
            bottoms[c] += h + GAP;
            at
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unplaced_blocks_fill_the_shortest_column_of_a_square_ish_grid() {
        // Two short blocks, then a tall one and a short one: 4 blocks, 2 columns.
        let at = masonry(&[40.0, 40.0, 120.0, 40.0], 100.0);

        let col = 100.0 + GAP;
        assert_eq!((at[0].x, at[0].y, at[1].x, at[1].y), (0.0, 0.0, col, 0.0));
        assert_eq!(
            (at[2].x, at[2].y),
            (0.0, 40.0 + GAP),
            "a tie goes to the first column"
        );
        assert_eq!(
            (at[3].x, at[3].y),
            (col, 40.0 + GAP),
            "the second column is now the shorter one"
        );
    }
}
