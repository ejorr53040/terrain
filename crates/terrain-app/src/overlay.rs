//! The overlay, toggled with `O`: the tool palette, the template Library and
//! the physics settings with the material presets.
//! Its controls only write the resources the keys write, so they behave
//! exactly as the keys do.

use bevy::prelude::*;
use bevy_egui::{EguiContexts, EguiGlobalSettings, EguiPlugin, EguiPrimaryContextPass, egui};
use terrain_app::{
    Arena, Library, Material, Materials, OverlayArea, PhysicsSettings, Requests, Selection, Shape,
    Tool,
};

pub struct OverlayPlugin;

impl Plugin for OverlayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EguiPlugin::default())
            .init_resource::<Overlay>()
            .add_systems(Startup, |mut settings: ResMut<EguiGlobalSettings>| {
                // Typing in the search box mustn't also press the scene's keys.
                settings.enable_absorb_bevy_input_system = true;
            })
            .add_systems(Update, toggle)
            .add_systems(EguiPrimaryContextPass, show);
    }
}

#[derive(Resource, Default)]
struct Overlay {
    open: bool,
    search: String,
}

fn toggle(keys: Res<ButtonInput<KeyCode>>, mut overlay: ResMut<Overlay>) {
    if keys.just_pressed(KeyCode::KeyO) {
        overlay.open = !overlay.open;
    }
}

fn show(
    mut contexts: EguiContexts,
    mut overlay: ResMut<Overlay>,
    mut tool: ResMut<Tool>,
    mut library: ResMut<Library>,
    mut requests: ResMut<Requests>,
    mut physics: ResMut<PhysicsSettings>,
    mut presets: ResMut<Materials>,
    selection: Res<Selection>,
    mut made_of: Query<&mut Material>,
    mut shapes: Query<&mut Shape>,
    arena: Res<Arena>,
    mut area: ResMut<OverlayArea>,
) -> Result {
    let ctx = contexts.ctx_mut()?;
    if !overlay.open {
        if area.0.is_some() {
            area.0 = None;
        }
        return Ok(());
    }
    let overlay = &mut *overlay;
    let shown = egui::Window::new("Builder  (O hides)")
        .default_pos([12.0, 90.0])
        .resizable(false)
        .vscroll(true)
        .default_height(420.0)
        .show(ctx, |ui| {
            ui.heading("Tool");
            ui.horizontal(|ui| {
                for (key, choice) in [
                    ("1", Tool::Move),
                    ("2", Tool::Round),
                    ("3", Tool::Taper),
                    ("4", Tool::Copy),
                ] {
                    if ui
                        .selectable_label(*tool == choice, format!("{key} {choice:?}"))
                        .clicked()
                    {
                        *tool = choice;
                    }
                }
            });
            ui.label("[ and ] step the selected shape's roundness (Round) or taper (Taper).");
            ui.separator();
            ui.heading("Selected shape");
            match selection.0.and_then(|e| shapes.get_mut(e).ok()) {
                // Edited on a copy, so the mesh and collider are only rebuilt on a change.
                Some(mut shape) => {
                    let mut edited = *shape;
                    let longest = arena.longest();
                    for (axis, name) in ["width", "height", "depth"].iter().enumerate() {
                        ui.add(
                            egui::Slider::new(&mut edited.size[axis], 0.03..=longest)
                                .suffix(" m")
                                .text(*name),
                        );
                    }
                    ui.add(egui::Slider::new(&mut edited.roundness, 0.0..=1.0).text("roundness (0 box, 1 cylinder)"));
                    ui.add(egui::Slider::new(&mut edited.taper, 0.0..=1.0).text("taper (1 = point)"));
                    if edited != *shape {
                        *shape = edited;
                    }
                }
                None => {
                    ui.label("Nothing selected: point at a shape, or grab one.");
                }
            }
            ui.separator();
            ui.heading("Library");
            ui.horizontal(|ui| {
                ui.label("Search");
                ui.text_edit_singleline(&mut overlay.search);
            });
            egui::ScrollArea::vertical().max_height(180.0).show(ui, |ui| {
                for i in library.search(&overlay.search) {
                    let t = &library.templates[i];
                    let label = match t.tags.as_slice() {
                        [] => t.name.clone(),
                        tags => format!("{}  ({})", t.name, tags.join(", ")),
                    };
                    if ui.selectable_label(library.current == i, label).clicked() {
                        library.current = i;
                    }
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Stamp at cursor (P)").clicked() {
                    requests.stamp = true;
                }
                if ui.button("Save selection (T)").clicked() {
                    requests.save = true;
                }
            });
            for error in &library.errors {
                ui.colored_label(egui::Color32::LIGHT_RED, format!("skipped {error}"));
            }
            ui.separator();
            ui.heading("Physics");
            // Edited on a copy, so the settings only count as changed when they are.
            let mut p = physics.clone();
            ui.horizontal(|ui| {
                ui.checkbox(&mut p.gravity_on, "Gravity");
                ui.add_enabled(
                    p.gravity_on,
                    egui::Slider::new(&mut p.gravity, 0.0..=20.0).suffix(" m/s²"),
                );
            });
            ui.horizontal(|ui| {
                ui.checkbox(&mut p.paused, "Paused");
                if ui.add_enabled(p.paused, egui::Button::new("Step")).clicked() {
                    p.step = true;
                }
            });
            ui.add(egui::Slider::new(&mut p.time_scale, 0.05..=2.0).text("time scale"));
            ui.add(egui::Slider::new(&mut p.linear_damping, 0.0..=5.0).text("linear damping"));
            ui.add(egui::Slider::new(&mut p.angular_damping, 0.0..=5.0).text("angular damping"));
            if p != *physics {
                *physics = p;
            }
            ui.separator();
            ui.heading("Materials");
            let mut edited = presets.presets.clone();
            egui::Grid::new("presets").striped(true).show(ui, |ui| {
                for title in ["", "density", "static", "dynamic", "bounce", "conducts"] {
                    ui.label(title);
                }
                ui.end_row();
                for p in &mut edited {
                    ui.label(&p.name);
                    ui.add(egui::DragValue::new(&mut p.density).range(10.0..=20000.0));
                    ui.add(egui::DragValue::new(&mut p.static_friction).speed(0.01).range(0.0..=2.0));
                    ui.add(egui::DragValue::new(&mut p.dynamic_friction).speed(0.01).range(0.0..=2.0));
                    ui.add(egui::DragValue::new(&mut p.restitution).speed(0.01).range(0.0..=1.0));
                    ui.checkbox(&mut p.conductive, "");
                    ui.end_row();
                }
            });
            if edited != presets.presets {
                presets.presets = edited;
            }
            if let Some(mut material) = selection.0.and_then(|e| made_of.get_mut(e).ok()) {
                ui.horizontal(|ui| {
                    ui.label("Selection is");
                    let names: Vec<String> = presets.presets.iter().map(|p| p.name.clone()).collect();
                    let mut picked = material.0;
                    egui::ComboBox::from_id_salt("material")
                        .selected_text(names.get(picked).cloned().unwrap_or_default())
                        .show_ui(ui, |ui| {
                            for (i, name) in names.iter().enumerate() {
                                ui.selectable_value(&mut picked, i, name);
                            }
                        });
                    if picked != material.0 {
                        material.0 = picked;
                    }
                });
            }
        });
    // egui points are the window's logical pixels.
    let drawn = shown.map(|s| {
        let r = s.response.rect;
        Rect::new(r.min.x, r.min.y, r.max.x, r.max.y)
    });
    if area.0 != drawn {
        area.0 = drawn;
    }
    Ok(())
}
