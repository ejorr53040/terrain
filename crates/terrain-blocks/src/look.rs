//! The Glacier palette, JetBrains Mono and pixel icons from Ethan's brand, on rounded,
//! chunky Scratch-like blocks with a hard shade (rounded by request, not stepped).

use eframe::egui::{self, Color32, CornerRadius, Margin, Pos2, Rect, Shadow, Stroke, Vec2};

/// Glacier tokens for one theme (brand `assets/tokens.css`).
#[derive(Clone, Copy)]
pub struct Palette {
    pub bg: Color32,
    pub surface: Color32,
    pub inset: Color32,
    pub border: Color32,
    pub border_strong: Color32,
    pub text: Color32,
    pub dim: Color32,
    /// Accent for grounds, borders and bars; never text.
    pub fill: Color32,
    /// Accent for text.
    pub link: Color32,
    pub wash: Color32,
    pub ok: Color32,
    pub err: Color32,
    b2: Color32,
    b4: Color32,
    pub b5: Color32,
    b6: Color32,
    o3: Color32,
    o4: Color32,
    o6: Color32,
    pub g4: Color32,
}

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

const LIGHT: Palette = Palette {
    bg: hex(0xEEF5FB),
    surface: hex(0xFFFFFF),
    inset: hex(0xDEEAF4),
    border: hex(0xC3D8E8),
    border_strong: hex(0x9CB8CE),
    text: hex(0x0F1A22),
    dim: hex(0x52687A),
    fill: hex(0xD0753C),
    link: hex(0xA8551F),
    wash: hex(0xF9EBE0),
    ok: hex(0x3F7A63),
    err: hex(0xB8443E),
    b2: hex(0xDEEAF4),
    b4: hex(0x9CB8CE),
    b5: hex(0x6E90AC),
    b6: hex(0x45647F),
    o3: hex(0xE3A575),
    o4: hex(0xD2763D),
    o6: hex(0x8E491F),
    g4: hex(0x65A489),
};

const DARK: Palette = Palette {
    bg: hex(0x0C141C),
    surface: hex(0x121C26),
    inset: hex(0x17232F),
    border: hex(0x253647),
    border_strong: hex(0x3A5266),
    text: hex(0xDFEAF3),
    dim: hex(0x8499AA),
    fill: hex(0xE38B4E),
    link: hex(0xF2A873),
    wash: hex(0x2B1E15),
    ok: hex(0x5FAA8C),
    err: hex(0xE2726B),
    b2: hex(0x17232F),
    b4: hex(0x3A5266),
    b5: hex(0x6E90AC),
    b6: hex(0x9CB8CE),
    o3: hex(0x8E491F),
    o4: hex(0xD2763D),
    o6: hex(0xEFC9A9),
    g4: hex(0x448069),
};

pub fn palette(ui: &egui::Ui) -> Palette {
    if ui.visuals().dark_mode { DARK } else { LIGHT }
}

/// Fonts and visuals for both themes; egui follows the system theme.
pub fn install(ctx: &egui::Context) {
    // ponytail: read from the system font dir; falls back to egui's fonts if missing.
    let dir = std::path::Path::new("/usr/share/fonts/TTF");
    let mut fonts = egui::FontDefinitions::default();
    for (name, file) in [
        ("mono", "JetBrainsMonoNerdFont-Regular.ttf"),
        ("bold", "JetBrainsMonoNerdFont-Bold.ttf"),
    ] {
        if let Ok(bytes) = std::fs::read(dir.join(file)) {
            fonts
                .font_data
                .insert(name.into(), egui::FontData::from_owned(bytes).into());
        }
    }
    if fonts.font_data.contains_key("mono") {
        for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
            fonts
                .families
                .entry(family)
                .or_default()
                .insert(0, "mono".into());
        }
    }
    let mut bold = fonts.families[&egui::FontFamily::Proportional].clone();
    if fonts.font_data.contains_key("bold") {
        bold.insert(0, "bold".into());
    }
    fonts
        .families
        .insert(egui::FontFamily::Name("bold".into()), bold);
    ctx.set_fonts(fonts);

    for (theme, p) in [(egui::Theme::Light, LIGHT), (egui::Theme::Dark, DARK)] {
        let mut v = theme.default_visuals();
        v.panel_fill = p.bg;
        v.window_fill = p.surface;
        v.extreme_bg_color = p.inset;
        v.faint_bg_color = p.inset;
        v.override_text_color = Some(p.text);
        v.hyperlink_color = p.link;
        v.selection.bg_fill = p.wash;
        v.selection.stroke = Stroke::new(1.0, p.link);
        v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, p.border);
        v.widgets.inactive.weak_bg_fill = p.inset;
        v.widgets.inactive.bg_fill = p.inset;
        v.widgets.hovered.weak_bg_fill = p.wash;
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, p.fill);
        v.widgets.active.weak_bg_fill = p.wash;
        ctx.set_visuals_of(theme, v);
    }
}

pub fn bold(size: f32) -> egui::FontId {
    egui::FontId::new(size, egui::FontFamily::Name("bold".into()))
}

/// A chunky rounded block with a hard shade, Scratch-like.
pub fn block(fill: Color32, edge: Color32, radius: u8) -> egui::Frame {
    egui::Frame::new()
        .fill(fill)
        .stroke(Stroke::new(2.0, edge))
        .corner_radius(CornerRadius::same(radius))
        .inner_margin(Margin::symmetric(12, 8))
        .shadow(Shadow {
            offset: [0, 4],
            blur: 0,
            spread: 0,
            color: edge.gamma_multiply(0.55),
        })
}

/// Fill and edge for an item or file block by kind: blue for code that runs, green
/// for data, deep blue for impls and traits, grey for the rest.
pub fn kind_colors(kind: &str, p: &Palette) -> (Color32, Color32) {
    let edge = match kind {
        "fn" => p.b5,
        "struct" | "enum" | "type" => p.g4,
        "impl" | "trait" => p.b6,
        "file" => p.border_strong,
        _ => p.dim,
    };
    (p.surface, edge)
}

/// The brand icon for a Library: Crate dependencies are cubes, Outside systems use
/// their declared icon.
pub fn library_icon(outside_icon: Option<&str>) -> &'static [&'static str; 12] {
    match outside_icon {
        None => &CUBE,
        Some("vm") => &SERVER,
        Some("network") => &GLOBE,
        Some("http") => &CLOUD,
        Some("tls") => &LOCK,
        Some("storage") => &STACK,
        Some("database") => &DATABASE,
        Some(_) => &TERMINAL, // "process", and anything unknown
    }
}

/// Allocate and paint a 12×12 brand icon at `px` points per cell.
pub fn icon(ui: &mut egui::Ui, grid: &[&str; 12], px: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0 * px), egui::Sense::hover());
    paint_icon(ui.painter(), rect.min, grid, px, &palette(ui));
}

fn paint_icon(painter: &egui::Painter, at: Pos2, grid: &[&str; 12], px: f32, p: &Palette) {
    for (y, row) in grid.iter().enumerate() {
        for (x, c) in row.chars().enumerate() {
            // Grid characters map to tokens, as in the brand's pixel.js.
            let color = match c {
                '1' => p.b2,
                '2' => p.b4,
                '3' => p.b6,
                '4' => p.text,
                '5' => p.o3,
                '6' => p.o4,
                '7' => p.o6,
                '8' => p.surface,
                '9' => p.ok,
                _ => continue,
            };
            let min = at + Vec2::new(x as f32 * px, y as f32 * px);
            painter.rect_filled(Rect::from_min_size(min, Vec2::splat(px)), 0.0, color);
        }
    }
}

// Grids copied from the brand's assets/pixel.js.
const CUBE: [&str; 12] = [
    "....4444....",
    "..44111144..",
    "441111111144",
    "431111111124",
    "433311112224",
    "433333222224",
    "433333222224",
    "433333222224",
    "433333222224",
    "443333222244",
    "..44332244..",
    "....4444....",
];
const SERVER: [&str; 12] = [
    "............",
    ".4444444444.",
    ".4911111114.",
    ".4111111114.",
    ".4444444444.",
    ".4911111114.",
    ".4111111114.",
    ".4444444444.",
    ".4911111114.",
    ".4111111114.",
    ".4444444444.",
    "............",
];
const GLOBE: [&str; 12] = [
    "............",
    "....4444....",
    "..44122144..",
    "..41122114..",
    ".4111221114.",
    ".4222222224.",
    ".4222222224.",
    ".4111221114.",
    "..41122114..",
    "..44122144..",
    "....4444....",
    "............",
];
const CLOUD: [&str; 12] = [
    "............",
    "............",
    "....4444....",
    "...411114...",
    "..41111114..",
    ".4111111114.",
    ".4111111114.",
    ".4444444444.",
    "............",
    "............",
    "............",
    "............",
];
const LOCK: [&str; 12] = [
    "............",
    "....4444....",
    "...44..44...",
    "...44..44...",
    "..44444444..",
    "..41111114..",
    "..41166114..",
    "..41166114..",
    "..41111114..",
    "..44444444..",
    "............",
    "............",
];
const STACK: [&str; 12] = [
    "..44444444..",
    "..41111114..",
    "..44444444..",
    "............",
    "..44444444..",
    "..41111114..",
    "..44444444..",
    "............",
    "..44444444..",
    "..41111114..",
    "..44444444..",
    "............",
];
const DATABASE: [&str; 12] = [
    "............",
    "...444444...",
    "..41111114..",
    "..44444444..",
    "..41111114..",
    "..41111114..",
    "..44444444..",
    "..41111114..",
    "..41111114..",
    "..44444444..",
    "...444444...",
    "............",
];
const TERMINAL: [&str; 12] = [
    "............",
    ".4444444444.",
    ".4111111114.",
    ".4661111114.",
    ".4166111114.",
    ".4661111114.",
    ".4111111114.",
    ".4116666114.",
    ".4111111114.",
    ".4444444444.",
    "............",
    "............",
];
