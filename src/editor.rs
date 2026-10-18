use crate::{
    i18n::t,
    paint::{self, Tool},
    preferences,
};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use image::RgbaImage;
#[derive(Default)]
pub struct Editor {
    pub enabled: bool,
    sizing: Option<(Pos2, f32, f32)>,
    pub suppress_context: bool,
    pub text: Option<crate::text_layer::TextLayer>,
    pub text_requested: bool,
    pub crop_requested: bool,
    pub floating: Option<Floating>,
    pub selection_action: Option<bool>,
    pub tool: Tool,
    pub diameter: f32,
    pub hardness: f32,
    pub color: [u8; 3],
    pub tolerance: u8,
    pub filled: bool,
    pub clone_origin: Option<[f32; 2]>,
    pub clone_image: Option<std::sync::Weak<RgbaImage>>,
    pub clone_offset: Option<[f32; 2]>,
    pub constraint_anchor: Option<[f32; 2]>,
    pub constraint_axis: Option<bool>,
    pub pixels: Option<RgbaImage>,
    pub texture: Option<egui::TextureHandle>,
    pub start: Option<[f32; 2]>,
    pub last: Option<[f32; 2]>,
    pub path: Vec<[f32; 2]>,
}
impl Editor {
    pub fn new() -> Self {
        Self {
            diameter: 32.0,
            hardness: 0.8,
            color: [255, 0, 0],
            ..Self::default()
        }
    }
    pub fn adjust_brush(&mut self, ctx: &egui::Context, ui: &egui::Ui, hovered: bool, scale_x: f32, scale_y: f32) -> bool {
        let (alt, pressed, down, position, focused) = ctx.input(|i| (i.modifiers.alt,
            i.pointer.button_pressed(egui::PointerButton::Secondary),
            i.pointer.button_down(egui::PointerButton::Secondary),
            i.pointer.interact_pos(), i.viewport().focused != Some(false)));
        self.suppress_context = false;
        if !matches!(self.tool, Tool::Brush | Tool::Clone | Tool::Heal) || !focused {
            self.sizing = None; return false;
        }
        if alt && pressed && hovered && self.start.is_none() {
            if let Some(position) = position { self.sizing = Some((position, self.diameter, self.hardness)); }
        }
        let Some((anchor, diameter, hardness)) = self.sizing else { return false; };
        self.suppress_context = true;
        if let Some(position) = position {
            let delta = position - anchor;
            if alt { (self.diameter, self.hardness) = brush_adjustment(diameter, hardness, delta, scale_x, self.tool == Tool::Brush); }
            egui::Area::new(egui::Id::new("brush-adjustment"))
                .order(egui::Order::Tooltip).fixed_pos(position + Vec2::new(20.0, 24.0))
                .interactable(false).show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.label(format!("{}: {:.0} px", t("Tamanho"), self.diameter));
                        if self.tool == Tool::Brush { ui.label(format!("{}: {:.0}%", t("Dureza"), self.hardness*100.0)); }
                    });
                });
            self.cursor(ui.painter(), position, scale_x, scale_y);
        }
        ctx.set_cursor_icon(egui::CursorIcon::None);
        if !down { self.sizing = None; }
        true
    }
    pub fn pending(&self) -> bool {
        self.floating.is_some() || self.text.is_some()
    }
    pub fn panel(&mut self, ctx: &egui::Context, available: bool) {
        if !self.enabled {
            return;
        }
        egui::SidePanel::left("editing_tools")
            .show_separator_line(false)
            .resizable(false)
            .exact_width(180.0)
            .show(ctx, |ui| {
                ui.add_space(110.0);
                ui.heading(t("Painel de pintura"));
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_enabled_ui(available && self.start.is_none() && !self.pending(), |ui| {
                        for (tool, label, index) in [
                            (Tool::Hand, "Mão", 12),
                            (Tool::Select, "Selecionar e mover", 9),
                            (Tool::Crop, "Recortar", 10),
                            (Tool::Eyedropper, "Conta-gotas", 5),
                            (Tool::Heal, "Correção localizada", 2),
                            (Tool::Brush, "Pincel", 0),
                            (Tool::Clone, "Carimbo", 7),
                            (Tool::Bucket, "Balde de tinta", 1),
                            (Tool::Text, "Texto", 11),
                            (Tool::Rectangle, "Retângulo", 3),
                            (Tool::Ellipse, "Elipse", 4),
                            (Tool::Line, "Linha", 8),
                            (Tool::Arrow, "Seta", 6),
                        ] {
                            ui.horizontal(|ui| {
                                let (rect, response) =
                                    ui.allocate_exact_size(Vec2::splat(28.0), egui::Sense::click());
                                crate::icons::paint_icon(ui, rect.shrink(3.0), index);
                                if response.clicked()
                                    || ui.selectable_label(self.tool == tool, match tool_shortcut(tool) { Some(key) => format!("{} ({key})", t(label)), None => t(label) }).clicked()
                                {
                                    self.tool = tool;
                                    if tool == Tool::Text {
                                        self.text_requested = true;
                                    }
                                    if tool == Tool::Crop {
                                        self.crop_requested = true;
                                    }
                                }
                            });
                        }
                        ui.separator();
                        if !matches!(
                            self.tool,
                            Tool::Hand | Tool::Eyedropper | Tool::Select | Tool::Crop | Tool::Text
                        ) {
                            ui.label(t("Tamanho (pixels da imagem)"));
                            ui.add(
                                egui::Slider::new(&mut self.diameter, 1.0..=512.0)
                                    .logarithmic(true),
                            );
                        }
                        if self.tool == Tool::Brush {
                            ui.label(t("Dureza"));
                            ui.add(
                                egui::Slider::new(&mut self.hardness, 0.0..=1.0)
                                    .custom_formatter(|n, _| format!("{:.0}%", n * 100.0)),
                            );
                        }
                        if self.tool == Tool::Bucket {
                            ui.label(t("Tolerância"));
                            ui.add(egui::Slider::new(&mut self.tolerance, 0..=255));
                        }
                        if matches!(self.tool, Tool::Ellipse | Tool::Rectangle) {
                            ui.checkbox(&mut self.filled, t("Preencher forma"));
                        }
                        if !matches!(
                            self.tool,
                            Tool::Hand | Tool::Heal | Tool::Clone | Tool::Select | Tool::Crop | Tool::Text
                        ) {
                            preferences::color_button(ui, &t("Cor"), &mut self.color);
                        }
                    });
                    if self.floating.is_some() {
                        ui.separator();
                        ui.horizontal(|ui| {
                            if ui.button(t("Aplicar")).clicked() {
                                self.selection_action = Some(true);
                            }
                            if ui.button(t("Cancelar")).clicked() {
                                self.selection_action = Some(false);
                            }
                        });
                    }
                    if let Some(text) = &mut self.text {
                        text.panel(ui);
                        text.refresh(ui.ctx());
                    }
                });
            });
    }
    pub fn begin_brush(&mut self, ctx: &egui::Context, source: &RgbaImage) {
        self.pixels = Some(source.clone());
        let limit = ctx.input(|i| i.max_texture_side) as u32;
        let small = if source.width() > limit || source.height() > limit {
            Some(image::imageops::thumbnail(source, limit, limit))
        } else {
            None
        };
        let preview = small.as_ref().unwrap_or(source);
        self.texture = Some(ctx.load_texture(
            "paint-live",
            egui::ColorImage::from_rgba_unmultiplied(
                [preview.width() as usize, preview.height() as usize],
                preview.as_raw(),
            ),
            egui::TextureOptions::LINEAR,
        ));
    }
    pub fn stroke(&mut self, source: &RgbaImage, a: [f32; 2], b: [f32; 2]) {
        if let (Some(pixels), Some(texture)) = (&mut self.pixels, &mut self.texture) {
            let bounds = if self.tool == Tool::Clone {
                paint::clone_stamp(
                    pixels,
                    source,
                    a,
                    b,
                    self.clone_offset.unwrap(),
                    self.diameter,
                    1.0,
                )
            } else {
                paint::brush(pixels, a, b, self.diameter, self.hardness, self.color)
            };
            if bounds.w == 0 || bounds.h == 0 {
                return;
            }
            let size = texture.size();
            let sx = size[0] as f32 / pixels.width() as f32;
            let sy = size[1] as f32 / pixels.height() as f32;
            let x = (bounds.x as f32 * sx).floor() as u32;
            let y = (bounds.y as f32 * sy).floor() as u32;
            let right = (((bounds.x + bounds.w) as f32 * sx).ceil() as u32).min(size[0] as u32);
            let bottom = (((bounds.y + bounds.h) as f32 * sy).ceil() as u32).min(size[1] as u32);
            let width = right - x;
            let height = bottom - y;
            if width == 0 || height == 0 {
                return;
            }
            // Small dirty tiles avoid uploading a large empty rectangle for diagonal strokes.
            for ty in (0..height).step_by(128) {
                for tx in (0..width).step_by(128) {
                    let tw = (width - tx).min(128);
                    let th = (height - ty).min(128);
                    let cx = (x + tx) as f32 / sx + tw as f32 / sx * 0.5;
                    let cy = (y + ty) as f32 / sy + th as f32 / sy * 0.5;
                    let dx = b[0] - a[0];
                    let dy = b[1] - a[1];
                    let length = dx * dx + dy * dy;
                    let t = if length > 0.0 {
                        ((cx - a[0]) * dx + (cy - a[1]) * dy) / length
                    } else {
                        0.0
                    }
                    .clamp(0.0, 1.0);
                    let distance = (cx - a[0] - t * dx).hypot(cy - a[1] - t * dy);
                    if distance
                        > self.diameter * 0.5 + (tw as f32 / sx).hypot(th as f32 / sy) * 0.5 + 2.0
                    {
                        continue;
                    }
                    let mut patch = RgbaImage::new(tw, th);
                    for yy in 0..th {
                        for xx in 0..tw {
                            let px = (((x + tx + xx) as f32 + 0.5) / sx) as u32;
                            let py = (((y + ty + yy) as f32 + 0.5) / sy) as u32;
                            patch.put_pixel(
                                xx,
                                yy,
                                *pixels.get_pixel(
                                    px.min(pixels.width() - 1),
                                    py.min(pixels.height() - 1),
                                ),
                            );
                        }
                    }
                    texture.set_partial(
                        [(x + tx) as usize, (y + ty) as usize],
                        egui::ColorImage::from_rgba_unmultiplied(
                            [tw as usize, th as usize],
                            patch.as_raw(),
                        ),
                        egui::TextureOptions::LINEAR,
                    );
                }
            }
        }
    }
    pub fn reset_gesture(&mut self) {
        self.constraint_anchor = None;
        self.constraint_axis = None;
        self.start = None;
        self.last = None;
        self.path.clear();
        self.texture = None;
        self.pixels = None;
    }
    pub fn cursor(&self, painter: &egui::Painter, point: Pos2, sx: f32, sy: f32) {
        if !matches!(self.tool, Tool::Brush | Tool::Heal | Tool::Clone) {
            return;
        }
        let radius = brush_radius(self.diameter, sx, sy);
        for (width, color) in [(3.0_f32, Color32::BLACK), (1.0_f32, Color32::WHITE)] {
            painter.add(egui::epaint::EllipseShape::stroke(
                point,
                radius,
                Stroke::new(width, color),
            ));
        }
    }
    pub fn shape_preview(&self, painter: &egui::Painter, rect: Rect, scale: f32) {
        let color = Color32::from_rgb(self.color[0], self.color[1], self.color[2]);
        if self.filled {
            if self.tool == Tool::Rectangle {
                painter.rect_filled(rect, 0.0, color);
            } else {
                painter.add(egui::epaint::EllipseShape::filled(
                    rect.center(),
                    rect.size() * 0.5,
                    color,
                ));
            }
            return;
        }
        if self.tool == Tool::Rectangle {
            painter.rect_stroke(
                rect,
                0.0,
                Stroke::new(self.diameter * scale, color),
                egui::StrokeKind::Inside,
            );
        } else {
            painter.add(egui::epaint::EllipseShape::stroke(
                rect.center(),
                rect.size() * 0.5,
                Stroke::new(self.diameter * scale, color),
            ));
        }
    }
}

fn brush_radius(diameter: f32, sx: f32, sy: f32) -> Vec2 {
    Vec2::new(diameter * sx * 0.5, diameter * sy * 0.5)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cursor_tracks_source_diameter_at_zoom_and_stretch() {
        assert_eq!(brush_radius(32.0, 1.0, 1.0), Vec2::splat(16.0));
        assert_eq!(brush_radius(32.0, 0.5, 0.5), Vec2::splat(8.0));
        assert_eq!(brush_radius(32.0, 2.0, 0.5), Vec2::new(32.0, 8.0));
    }
}

pub struct Floating {
    pub source: crate::model::Crop,
    pub offset: [i32; 2],
    pub copy: bool,
    pub texture: egui::TextureHandle,
    pub image: std::sync::Weak<RgbaImage>,
    pub grab: Option<([f32; 2], [i32; 2])>,
}
impl Editor {
    pub fn selection_input(
        &mut self,
        ctx: &egui::Context,
        ui: &egui::Ui,
        response: &egui::Response,
        im: &std::sync::Arc<RgbaImage>,
        shown: Rect,
        source_rect: Rect,
        scale: Vec2,
        rotation: crate::view_rotation::Rotation,
        checker: &egui::TextureHandle,
        background: Color32,
        show_checker: bool,
    ) {
        if self.floating.as_ref().is_some_and(|f| {
            !f.image
                .upgrade()
                .is_some_and(|source| std::sync::Arc::ptr_eq(&source, im))
        }) {
            self.floating = None;
        }
        let source_point = |p: Pos2| {
            let p = rotation.inverse().point(p, shown.center());
            [
                ((p.x - source_rect.left()) / scale.x).clamp(0.0, im.width() as f32),
                ((p.y - source_rect.top()) / scale.y).clamp(0.0, im.height() as f32),
            ]
        };
        let screen_rect = |c: crate::model::Crop, offset: [i32; 2]| {
            let min = source_rect.min
                + Vec2::new(
                    (c.x as i32 + offset[0]) as f32 * scale.x,
                    (c.y as i32 + offset[1]) as f32 * scale.y,
                );
            let rect =
                Rect::from_min_size(min, Vec2::new(c.w as f32 * scale.x, c.h as f32 * scale.y));
            rotation.rect(rect, shown.center())
        };
        let pointer = ctx.input(|i| i.pointer.interact_pos());
        let pressed = ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary));
        let released = ctx.input(|i| {
            i.pointer.button_released(egui::PointerButton::Primary)
                || i.viewport().focused == Some(false)
        });
        if released && pointer.is_none() {
            self.start = None;
            self.last = None;
            if let Some(f) = &mut self.floating {
                f.grab = None;
            }
        }
        if response.hovered() {
            ctx.set_cursor_icon(if self.floating.is_some() {
                egui::CursorIcon::Grab
            } else {
                egui::CursorIcon::Crosshair
            });
        }
        if pressed && response.hovered() {
            let origin = ctx
                .input(|i| {
                    i.events
                        .iter()
                        .find_map(|e| match e {
                            egui::Event::PointerButton {
                                pos,
                                button: egui::PointerButton::Primary,
                                pressed: true,
                                ..
                            } => Some(*pos),
                            _ => None,
                        })
                        .or_else(|| i.pointer.press_origin())
                })
                .or(pointer);
            if let Some(origin) = origin.filter(|p| shown.contains(*p)) {
                let p = source_point(origin);
                if let Some(f) = &mut self.floating {
                    if screen_rect(f.source, f.offset).contains(origin) {
                        f.grab = Some((p, f.offset));
                        f.copy = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
                    }
                } else {
                    self.start = Some(p);
                    self.last = Some(p);
                }
            }
        }
        if let Some(f) = &mut self.floating {
            if let (Some((origin, offset)), Some(p)) = (f.grab, pointer) {
                let p = source_point(p);
                f.offset = [
                    (offset[0] + (p[0] - origin[0]).round() as i32).clamp(
                        -(f.source.x as i32),
                        (im.width() - f.source.x - f.source.w) as i32,
                    ),
                    (offset[1] + (p[1] - origin[1]).round() as i32).clamp(
                        -(f.source.y as i32),
                        (im.height() - f.source.y - f.source.h) as i32,
                    ),
                ];
                if released {
                    f.grab = None;
                }
                ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
            }
            if !f.copy && f.offset != [0, 0] {
                let hole = screen_rect(f.source, [0, 0]);
                ui.painter().rect_filled(hole, 0.0, background);
                if show_checker {
                    ui.painter().image(
                        checker.id(),
                        hole,
                        Rect::from_min_max(
                            Pos2::ZERO,
                            Pos2::new(hole.width() / 32.0, hole.height() / 32.0),
                        ),
                        Color32::WHITE,
                    );
                }
            }
            if f.offset != [0, 0] {
                let min = source_rect.min
                    + Vec2::new(
                        (f.source.x as i32 + f.offset[0]) as f32 * scale.x,
                        (f.source.y as i32 + f.offset[1]) as f32 * scale.y,
                    );
                rotation.paint(
                    ui.painter(),
                    f.texture.id(),
                    Rect::from_min_size(
                        min,
                        Vec2::new(f.source.w as f32 * scale.x, f.source.h as f32 * scale.y),
                    ),
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    shown.center(),
                );
            }
            selection_border(ui.painter(), screen_rect(f.source, f.offset));
        } else if let (Some(start), Some(p)) = (self.start, pointer) {
            let p = source_point(p);
            self.last = Some(p);
            let x = start[0].min(p[0]).floor() as u32;
            let y = start[1].min(p[1]).floor() as u32;
            let w = (start[0].max(p[0]).ceil() as u32).saturating_sub(x);
            let h = (start[1].max(p[1]).ceil() as u32).saturating_sub(y);
            if w > 0 && h > 0 {
                let c = crate::model::Crop { x, y, w, h };
                selection_border(ui.painter(), screen_rect(c, [0, 0]));
                if released {
                    let patch = image::imageops::crop_imm(&**im, x, y, w, h).to_image();
                    let limit = ctx.input(|i| i.max_texture_side) as u32;
                    let patch = if w > limit || h > limit {
                        image::imageops::thumbnail(&patch, limit, limit)
                    } else {
                        patch
                    };
                    let texture = ctx.load_texture(
                        "floating-selection",
                        egui::ColorImage::from_rgba_unmultiplied(
                            [patch.width() as usize, patch.height() as usize],
                            patch.as_raw(),
                        ),
                        egui::TextureOptions::LINEAR,
                    );
                    self.floating = Some(Floating {
                        source: c,
                        offset: [0, 0],
                        copy: false,
                        texture,
                        image: std::sync::Arc::downgrade(im),
                        grab: None,
                    });
                }
            }
            if released {
                self.start = None;
                self.last = None;
            }
        }
    }
}
fn selection_border(p: &egui::Painter, rect: Rect) {
    p.rect_stroke(
        rect,
        0.0,
        Stroke::new(3.0_f32, Color32::BLACK),
        egui::StrokeKind::Inside,
    );
    p.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0_f32, Color32::WHITE),
        egui::StrokeKind::Inside,
    );
}

fn brush_adjustment(diameter: f32, hardness: f32, delta: Vec2, scale: f32, adjust_hardness: bool) -> (f32, f32) {
    ((diameter + delta.x / scale.max(0.001)).clamp(1.0, 512.0),
     if adjust_hardness { (hardness - delta.y / 200.0).clamp(0.0, 1.0) } else { hardness })
}
#[cfg(test)]
mod adjustment_tests {
    use super::*;
    #[test]
    fn zoom_scaling_and_limits() {
        assert_eq!(brush_adjustment(32.0, 0.8, Vec2::new(20.0, 100.0), 2.0, false), (42.0, 0.8));
        assert_eq!(brush_adjustment(32.0, 0.8, Vec2::new(20.0, 60.0), 2.0, true), (42.0, 0.5));
        assert_eq!(brush_adjustment(32.0, 0.8, Vec2::new(-1000.0, -1000.0), 1.0, true), (1.0, 1.0));
        assert_eq!(brush_adjustment(32.0, 0.8, Vec2::new(1000.0, 1000.0), 1.0, true), (512.0, 0.0));
    }
}

pub fn shortcut_tool(key: egui::Key, modifiers: egui::Modifiers) -> Option<Tool> {
    if modifiers.ctrl || modifiers.command || modifiers.alt || modifiers.shift { return None; }
    match key {
        egui::Key::B => Some(Tool::Brush),
        egui::Key::H => Some(Tool::Hand),
        egui::Key::J => Some(Tool::Heal),
        egui::Key::U => Some(Tool::Rectangle),
        _ => None,
    }
}
fn tool_shortcut(tool: Tool) -> Option<&'static str> {
    match tool {
        Tool::Brush => Some("B"), Tool::Hand => Some("H"),
        Tool::Heal => Some("J"), Tool::Rectangle => Some("U"), _ => None,
    }
}
#[cfg(test)]
mod shortcut_tests {
    use super::*;
    #[test]
    fn tool_keys_leave_modified_shortcuts_intact() {
        assert!(shortcut_tool(egui::Key::S, egui::Modifiers::NONE).is_none());
        for (key, tool) in [(egui::Key::B,Tool::Brush),(egui::Key::H,Tool::Hand),(egui::Key::J,Tool::Heal),(egui::Key::U,Tool::Rectangle)] {
            assert!(shortcut_tool(key,egui::Modifiers::NONE) == Some(tool));
            for modifiers in [egui::Modifiers::CTRL,egui::Modifiers::ALT,egui::Modifiers::SHIFT,egui::Modifiers::COMMAND] {
                assert!(shortcut_tool(key,modifiers).is_none());
            }
        }
    }
}
