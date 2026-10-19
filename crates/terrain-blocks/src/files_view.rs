//! The Files view: file blocks with their stacks. The only view in which a drag
//! moves code.

use eframe::egui::{self, Pos2, Rect, RichText, Sense, pos2, vec2};
use terrain_blocks::{Block, Id};

use crate::{App, FILE_WIDTH, LABEL_SIZE, look, masonry};

impl App {
    pub(crate) fn files_canvas(&mut self, ui: &mut egui::Ui) {
        self.containers.clear();
        let pointer = self.pointer(ui);
        let files = self.ws.files().to_vec();
        let mut dropped = None;
        let heights: Vec<f32> = files.iter().map(estimated_height).collect();
        let defaults = masonry(&heights, FILE_WIDTH);
        for (i, file) in files.iter().enumerate() {
            let id = Id(vec![i]);
            let pos = match (self.dragging.as_ref() == Some(&id), self.file_drag) {
                (true, Some(p)) => p,
                _ => self
                    .ws
                    .position(&id)
                    .map_or(defaults[i], |[x, y]| pos2(x, y)),
            };
            let max = Rect::from_min_size(pos, vec2(FILE_WIDTH, f32::INFINITY));
            ui.scope_builder(egui::UiBuilder::new().max_rect(max), |ui| {
                self.block(ui, file, id, &mut dropped);
            });
        }

        let (Some(dragging), Some(p)) = (self.dragging.clone(), pointer) else {
            return;
        };
        let pal = look::palette(ui);
        if dragging.0.len() > 1 {
            if let Some((into, index, y, rect)) = self.drop_target(&dragging, p) {
                let x = rect.x_range().shrink(6.0);
                ui.painter().hline(x, y, egui::Stroke::new(4.0, pal.fill));
                if dropped.as_ref() == Some(&dragging) {
                    self.drop(dragging.clone(), into, index);
                }
            }
            let label = self.ws.block(&dragging).map(label).unwrap_or_default();
            ui.painter().text(
                p + vec2(14.0, 0.0),
                egui::Align2::LEFT_CENTER,
                label,
                look::bold(LABEL_SIZE),
                pal.text,
            );
        }
        if dropped.is_some() {
            if let (1, Some(at)) = (dragging.0.len(), self.file_drag)
                && let Err(e) = self.ws.place(&dragging, [at.x, at.y])
            {
                self.message = format!("couldn't save the layout: {e}");
            }
            self.dragging = None;
            self.file_drag = None;
        }
    }

    /// Draws `b` and its stack; returns its rect.
    fn block(&mut self, ui: &mut egui::Ui, b: &Block, id: Id, dropped: &mut Option<Id>) -> Rect {
        let pal = look::palette(ui);
        let (fill, edge) = look::kind_colors(b.kind, &pal);
        let edge = if b.error { pal.err } else { edge };
        let radius = if b.kind == "file" { 14 } else { 10 };
        let inner = look::block(fill, edge, radius).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let text = RichText::new(label(b)).font(look::bold(LABEL_SIZE));
            let head = ui
                .add(
                    egui::Label::new(text)
                        .sense(Sense::click_and_drag())
                        .selectable(false),
                )
                .on_hover_cursor(egui::CursorIcon::Grab);
            if b.error {
                ui.colored_label(pal.err, "can't parse: left untouched");
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
            Ok(()) => self.code_changed("moved"),
            Err(e) => self.refused(e),
        }
    }
}

/// A header row plus the frame's margins, and the stack inside.
fn estimated_height(b: &Block) -> f32 {
    44.0 + b
        .children
        .iter()
        .map(|c| estimated_height(c) + 6.0)
        .sum::<f32>()
}

fn label(b: &Block) -> String {
    if b.kind == "file" {
        b.name.clone()
    } else {
        format!("{} {}", b.kind, b.name)
    }
}
