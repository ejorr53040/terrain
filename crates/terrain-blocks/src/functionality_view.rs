//! The Functionality view: each crate is a System holding its Libraries, the user's
//! Capabilities and an Unsorted tray. Dragging here only regroups; code never moves.

use std::collections::HashMap;

use eframe::egui::{
    self, Align, Layout, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, pos2, vec2,
};
use terrain_blocks::{Capability, Item, ItemRef, Library, LibraryKind, MoveError, System};

use crate::{App, LABEL_SIZE, SYSTEM_WIDTH, look, masonry};

/// What was drawn this frame, for drops and lines.
#[derive(Default)]
struct Hits {
    /// Unsorted items: ref, System, rect.
    items: Vec<(ItemRef, String, Rect)>,
    /// Capabilities: index, System, rect, Libraries used.
    caps: Vec<(usize, String, Rect, Vec<String>)>,
    /// Library chips by (System, Library).
    libs: HashMap<(String, String), Rect>,
    systems: Vec<(String, Rect)>,
    /// An item drag ended this frame.
    dropped: bool,
}

enum Target {
    /// Onto an Unsorted item: the two make a new Capability.
    Item(ItemRef),
    Capability(usize),
    Unsorted,
}

impl App {
    pub(crate) fn functionality_canvas(&mut self, ui: &mut egui::Ui) {
        let pal = look::palette(ui);
        let pointer = self.pointer(ui);
        let systems = self.systems.clone();

        // System → System lines go first so the blocks cover them; last frame's rects.
        for s in &systems {
            for dep in &s.depends_on {
                if let (Some(a), Some(b)) =
                    (self.system_rects.get(&s.name), self.system_rects.get(dep))
                {
                    ui.painter().line_segment(
                        [a.center(), b.center()],
                        Stroke::new(2.5, pal.border_strong.gamma_multiply(0.6)),
                    );
                }
            }
        }

        // Only unplaced Systems fill the default grid, by last frame's measured heights
        // (estimates on the first frame).
        let unplaced: Vec<&System> = systems
            .iter()
            .filter(|s| self.ws.system_position(&s.name).is_none())
            .collect();
        let heights: Vec<f32> = unplaced
            .iter()
            .map(|s| {
                self.system_rects
                    .get(&s.name)
                    .map_or_else(|| self.system_height(s), |r| r.height())
            })
            .collect();
        let defaults: HashMap<&str, Pos2> = unplaced
            .iter()
            .map(|s| s.name.as_str())
            .zip(masonry(&heights, SYSTEM_WIDTH))
            .collect();
        let mut hits = Hits::default();
        for s in &systems {
            let pos = match (&self.system_drag, self.ws.system_position(&s.name)) {
                (Some((name, at)), _) if *name == s.name => *at,
                (_, Some([x, y])) => pos2(x, y),
                _ => defaults[s.name.as_str()],
            };
            let max = Rect::from_min_size(pos, vec2(SYSTEM_WIDTH, f32::INFINITY));
            let rect = ui
                .scope_builder(egui::UiBuilder::new().max_rect(max), |ui| {
                    self.system_block(ui, s, pos, &mut hits)
                })
                .inner;
            self.system_rects.insert(s.name.clone(), rect);
            hits.systems.push((s.name.clone(), rect));
        }

        // Capability → Library lines: faint, strong for the Capability under the pointer.
        for (_, system, rect, uses) in &hits.caps {
            let hovered = pointer.is_some_and(|p| rect.contains(p));
            let stroke = Stroke::new(
                if hovered { 3.0 } else { 2.0 },
                pal.fill.gamma_multiply(if hovered { 1.0 } else { 0.3 }),
            );
            for lib in uses {
                if let Some(chip) = hits.libs.get(&(system.clone(), lib.clone())) {
                    ui.painter()
                        .line_segment([chip.center_bottom(), rect.center_top()], stroke);
                }
            }
        }

        let (Some((dragged, system)), Some(p)) = (self.item_drag.clone(), pointer) else {
            return;
        };
        let target = hits
            .items
            .iter()
            .find(|(r, s, rect)| *s == system && *r != dragged && rect.contains(p))
            .map(|(r, _, rect)| (Target::Item(r.clone()), *rect))
            .or_else(|| {
                hits.caps
                    .iter()
                    .find(|(_, s, rect, _)| *s == system && rect.contains(p))
                    .map(|(i, _, rect, _)| (Target::Capability(*i), *rect))
            })
            .or_else(|| {
                hits.systems
                    .iter()
                    .find(|(s, rect)| *s == system && rect.contains(p))
                    .map(|(_, rect)| (Target::Unsorted, *rect))
            });
        if let Some((_, rect)) = &target {
            ui.painter().rect_stroke(
                rect.expand(3.0),
                12,
                Stroke::new(3.0, pal.fill),
                StrokeKind::Outside,
            );
        }
        ui.painter().text(
            p + vec2(14.0, 0.0),
            egui::Align2::LEFT_CENTER,
            &dragged.label,
            look::bold(LABEL_SIZE),
            pal.text,
        );
        if hits.dropped {
            self.item_drag = None;
            if let Some((target, _)) = target {
                self.regroup(dragged, target);
            }
        }
    }

    /// Draws a System; returns its rect.
    fn system_block(&mut self, ui: &mut egui::Ui, s: &System, pos: Pos2, hits: &mut Hits) -> Rect {
        let pal = look::palette(ui);
        let inner = look::block(pal.surface, pal.border_strong, 18).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                let head = ui
                    .add(
                        egui::Label::new(RichText::new(&s.name).font(look::bold(22.0)))
                            .sense(Sense::click_and_drag())
                            .selectable(false),
                    )
                    .on_hover_cursor(egui::CursorIcon::Grab);
                if head.drag_started() {
                    self.system_drag = Some((s.name.clone(), pos));
                }
                if let (true, Some((_, at))) = (head.dragged(), self.system_drag.as_mut()) {
                    *at += head.drag_delta();
                }
                if head.drag_stopped()
                    && let Some((name, at)) = self.system_drag.take()
                    && let Err(e) = self.ws.place_system(&name, [at.x, at.y])
                {
                    self.message = format!("couldn't save the layout: {e}");
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .button(RichText::new("+ Capability").font(look::bold(14.0)))
                        .clicked()
                    {
                        match self.ws.new_capability(&s.name, "New capability") {
                            Ok(index) => {
                                self.renaming = Some((index, "New capability".into()));
                                self.reload_systems();
                            }
                            Err(e) => self.refused(e),
                        }
                    }
                });
            });

            // Rows by measured width: framed chips don't wrap in a wrapped layout.
            let room = ui.available_width();
            let mut rows: Vec<Vec<&Library>> = vec![Vec::new()];
            let mut used = 0.0;
            for lib in &s.libraries {
                let w = chip_width(ui, lib) + ui.spacing().item_spacing.x;
                if used + w > room && !rows.last().unwrap().is_empty() {
                    rows.push(Vec::new());
                    used = 0.0;
                }
                rows.last_mut().unwrap().push(lib);
                used += w;
            }
            // The plumbing button ends the last row, or takes one of its own.
            if !s.plumbing.is_empty() {
                let font = egui::TextStyle::Body.resolve(ui.style());
                let text = ui
                    .painter()
                    .layout_no_wrap("00 plumbing".into(), font, egui::Color32::WHITE)
                    .size()
                    .x;
                if used + text + 2.0 * ui.spacing().button_padding.x > room {
                    rows.push(Vec::new());
                }
            }
            let last = rows.len() - 1;
            for (i, row) in rows.into_iter().enumerate() {
                ui.horizontal(|ui| {
                    for lib in row {
                        hits.libs
                            .insert((s.name.clone(), lib.name.clone()), library_chip(ui, lib));
                    }
                    if i == last {
                        self.plumbing_menu(ui, s);
                    }
                });
            }
            ui.add_space(6.0);
            for cap in &s.capabilities {
                self.capability_block(ui, s, cap, hits);
                ui.add_space(6.0);
            }
            self.unsorted_tray(ui, s, hits);
        });
        inner.response.rect
    }

    fn plumbing_menu(&mut self, ui: &mut egui::Ui, s: &System) {
        if s.plumbing.is_empty() {
            return;
        }
        let hidden = s.plumbing.iter().filter(|(_, shown)| !shown).count();
        let pal = look::palette(ui);
        let button = ui.button(RichText::new(format!("{hidden} plumbing")).color(pal.dim));
        egui::Popup::menu(&button).show(|ui| {
            for (name, shown) in &s.plumbing {
                let mut on = *shown;
                if ui.checkbox(&mut on, name).changed() {
                    match self.ws.pin(name, on) {
                        Ok(()) => self.reload_systems(),
                        Err(e) => self.message = format!("couldn't save the sidecar: {e}"),
                    }
                }
            }
        });
    }

    fn capability_block(
        &mut self,
        ui: &mut egui::Ui,
        s: &System,
        cap: &Capability,
        hits: &mut Hits,
    ) {
        let pal = look::palette(ui);
        let inner = look::block(pal.wash, pal.fill, 14).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                match self.renaming.as_mut() {
                    Some((index, name)) if *index == cap.index => {
                        let edit = ui.add(
                            egui::TextEdit::singleline(name)
                                .font(look::bold(17.0))
                                .desired_width(220.0),
                        );
                        if edit.lost_focus() {
                            let (index, name) = self.renaming.take().unwrap();
                            if !name.trim().is_empty() {
                                self.capability_op(|ws| ws.rename(index, name.trim()));
                            }
                        } else {
                            edit.request_focus();
                        }
                    }
                    _ => {
                        let title = egui::Label::new(
                            RichText::new(&cap.name)
                                .font(look::bold(17.0))
                                .color(pal.link),
                        )
                        .sense(Sense::click())
                        .selectable(false);
                        if ui
                            .add(title)
                            .on_hover_text("double-click to rename")
                            .double_clicked()
                        {
                            self.renaming = Some((cap.index, cap.name.clone()));
                        }
                    }
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if ui
                        .small_button("✖")
                        .on_hover_text("remove; items go back to Unsorted")
                        .clicked()
                    {
                        self.capability_op(|ws| ws.remove_capability(cap.index));
                    }
                    if cap.missing > 0
                        && ui
                            .button(
                                RichText::new(format!("{} missing", cap.missing)).color(pal.err),
                            )
                            .on_hover_text("renamed or deleted outside the app; click to dismiss")
                            .clicked()
                    {
                        self.capability_op(|ws| ws.dismiss_missing(cap.index));
                    }
                });
            });
            if cap.items.is_empty() {
                ui.label(RichText::new("drop items here").color(pal.dim));
            }
            for item in &cap.items {
                self.item_chip(ui, s, item, false, hits);
            }
        });
        hits.caps.push((
            cap.index,
            s.name.clone(),
            inner.response.rect,
            cap.uses.clone(),
        ));
    }

    fn unsorted_tray(&mut self, ui: &mut egui::Ui, s: &System, hits: &mut Hits) {
        let pal = look::palette(ui);
        let open = self.expanded.contains(&s.name);
        look::block(pal.inset, pal.border, 14).show(ui, |ui| {
            ui.set_width(ui.available_width());
            let arrow = if open { "▾" } else { "▸" };
            let title = egui::Label::new(
                RichText::new(format!("{arrow} Unsorted ({})", s.unsorted.len()))
                    .font(look::bold(16.0))
                    .color(pal.dim),
            )
            .sense(Sense::click())
            .selectable(false);
            if ui.add(title).clicked() && !self.expanded.remove(&s.name) {
                self.expanded.insert(s.name.clone());
            }
            if open {
                for item in &s.unsorted {
                    self.item_chip(ui, s, item, true, hits);
                }
            }
        });
    }

    /// An item as a whole-chip drag handle, big enough for a hand pointer.
    fn item_chip(
        &mut self,
        ui: &mut egui::Ui,
        s: &System,
        item: &Item,
        unsorted: bool,
        hits: &mut Hits,
    ) {
        let pal = look::palette(ui);
        let kind = item.label.split(' ').next().unwrap_or_default();
        let (fill, edge) = look::kind_colors(kind, &pal);
        let chip = look::block(fill, edge, 10).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(
                egui::Label::new(RichText::new(&item.label).font(look::bold(LABEL_SIZE)))
                    .selectable(false),
            );
        });
        let r = chip
            .response
            .interact(Sense::click_and_drag())
            .on_hover_cursor(egui::CursorIcon::Grab);
        let reference = ItemRef {
            file: item.file.clone(),
            label: item.label.clone(),
        };
        if r.drag_started() {
            self.item_drag = Some((reference.clone(), s.name.clone()));
        }
        if r.drag_stopped() {
            hits.dropped = true;
        }
        if r.clicked() {
            self.selected = Some(item.id.clone());
        }
        if r.secondary_clicked() {
            self.open_in_nvim(&item.id, item.line);
        }
        if unsorted {
            hits.items.push((reference, s.name.clone(), r.rect));
        }
    }

    fn regroup(&mut self, dragged: ItemRef, target: Target) {
        self.capability_op(|ws| match target {
            Target::Item(onto) => ws.group(&onto, &dragged).map(|_| ()),
            Target::Capability(c) => ws.assign(&dragged, Some(c)),
            Target::Unsorted => ws.assign(&dragged, None),
        });
    }

    /// Run a sidecar-only change and show the result; code is never touched.
    fn capability_op(
        &mut self,
        op: impl FnOnce(&mut terrain_blocks::Workspace) -> Result<(), MoveError>,
    ) {
        match op(&mut self.ws) {
            Ok(()) => self.reload_systems(),
            Err(e) => self.refused(e),
        }
    }

    /// Rough height before the first frame measures it, for the default layout.
    fn system_height(&self, s: &System) -> f32 {
        let row = 46.0;
        let caps: f32 = s
            .capabilities
            .iter()
            .map(|c| 56.0 + c.items.len().max(1) as f32 * row)
            .sum();
        let tray = 48.0
            + if self.expanded.contains(&s.name) {
                s.unsorted.len() as f32 * row
            } else {
                0.0
            };
        let libs = 52.0 * (s.libraries.len() as f32 / 3.0).ceil().max(1.0);
        70.0 + libs + caps + tray
    }
}

/// The width `library_chip` will take, before drawing it.
fn chip_width(ui: &egui::Ui, lib: &Library) -> f32 {
    let text = ui
        .painter()
        .layout_no_wrap(lib.name.clone(), look::bold(14.0), egui::Color32::WHITE)
        .size()
        .x;
    let spacing = ui.spacing().item_spacing.x;
    let tile = if lib.kind == LibraryKind::Crate {
        22.0 + spacing
    } else {
        0.0
    };
    // Frame margins and stroke, the icon, the text.
    28.0 + tile + 24.0 + spacing + text
}

/// A Library as a chip: Crate dependencies get a monogram tile and the cube icon,
/// Outside systems their own icon on the accent wash.
fn library_chip(ui: &mut egui::Ui, lib: &Library) -> Rect {
    let pal = look::palette(ui);
    let (fill, edge, icon) = match &lib.kind {
        LibraryKind::Crate => (pal.inset, pal.border_strong, look::library_icon(None)),
        LibraryKind::Outside { icon } => (pal.wash, pal.fill, look::library_icon(Some(icon))),
    };
    look::block(fill, edge, 12)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                if lib.kind == LibraryKind::Crate {
                    let (tile, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                    ui.painter().rect_filled(tile, 6, pal.b5);
                    let letter = lib.name.chars().next().unwrap_or('?').to_ascii_uppercase();
                    ui.painter().text(
                        tile.center(),
                        egui::Align2::CENTER_CENTER,
                        letter,
                        look::bold(14.0),
                        pal.surface,
                    );
                }
                look::icon(ui, icon, 2.0);
                ui.label(RichText::new(&lib.name).font(look::bold(14.0)));
            });
        })
        .response
        .rect
}
