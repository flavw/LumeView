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
    pub crop_ratio_requested: Option<Option<f32>>,
    crop_ratio: Option<f32>,
    crop_dimensions: (u32,u32),
    crop_width: String,
    crop_height: String,
    pub floating: Option<Floating>,
    pub selection_action: Option<bool>,
    pub tool: Tool,
    shape_tool: Option<Tool>,
    retouch_tool: Option<Tool>,
    pub diameter: f32,
    pub hardness: f32,
    pub color: [u8; 3],
    pub fill_color: [u8; 3],
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
    pub heal_preview: crate::heal_preview::Preview,
}
impl Editor {
    pub fn new() -> Self {
        Self {
            diameter: 32.0,
            hardness: 0.8,
            color: [255, 0, 0],
            fill_color: [255; 3],
            ..Self::default()
        }
    }
    pub fn adjust_brush(&mut self, ctx: &egui::Context, ui: &egui::Ui, hovered: bool, scale_x: f32, scale_y: f32) -> bool {
        let (alt, pressed, down, position, focused) = ctx.input(|i| (i.modifiers.alt,
            i.pointer.button_pressed(egui::PointerButton::Secondary),
            i.pointer.button_down(egui::PointerButton::Secondary),
            i.pointer.interact_pos(), i.viewport().focused != Some(false)));
        self.suppress_context = false;
        if !matches!(self.tool, Tool::Brush | Tool::Clone | Tool::Heal | Tool::Remove) || !focused {
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
    pub fn select_tool(&mut self, tool: Tool) {
        self.tool = tool;
        if is_shape(tool) { self.shape_tool = Some(tool); }
        if matches!(tool,Tool::Heal|Tool::Remove) { self.retouch_tool = Some(tool); }
        if tool == Tool::Text { self.text_requested = true; }
        if tool == Tool::Crop { self.crop_requested = true; }
    }
    pub fn shortcut_tool(&self, key: egui::Key, modifiers: egui::Modifiers) -> Option<Tool> {
        if modifiers.ctrl || modifiers.command || modifiers.alt { return None; }
        if key == egui::Key::J {
            let current=self.retouch_tool.unwrap_or(Tool::Heal);
            return Some(if modifiers.shift {if current==Tool::Heal {Tool::Remove}else{Tool::Heal}}else{current});
        }
        if key == egui::Key::U {
            let current = if is_shape(self.tool) { self.tool } else { self.shape_tool.unwrap_or(Tool::Rectangle) };
            return Some(if modifiers.shift {
                match current {
                    Tool::Rectangle => Tool::Ellipse, Tool::Ellipse => Tool::Line,
                    Tool::Line => Tool::Arrow, _ => Tool::Rectangle,
                }
            } else { current });
        }
        if modifiers.shift { return None; }
        match key {
            egui::Key::B => Some(Tool::Brush), egui::Key::H => Some(Tool::Hand),
            egui::Key::S => Some(Tool::Clone),
            egui::Key::I => Some(Tool::Eyedropper), egui::Key::G => Some(Tool::Bucket),
            egui::Key::M | egui::Key::V => Some(Tool::Select),
            egui::Key::C => Some(Tool::Crop), egui::Key::T => Some(Tool::Text),
            _ => None,
        }
    }
    pub fn panel(&mut self, ctx: &egui::Context, available: bool, chrome_height: f32) {
        if !self.enabled { return; }
        // Reserve the same height for every tool, including tools without options.
        egui::TopBottomPanel::top("paint_controls")
            .resizable(false).show_separator_line(false)
            .exact_height(chrome_height + 48.0)
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(egui::Margin { left:36,right:8,top:4,bottom:4 }))
            .show(ctx, |ui| {
                ui.add_space(chrome_height);
                egui::ScrollArea::horizontal().id_salt("paint_controls_scroll")
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.spacing_mut().slider_width = 96.0;
                        ui.allocate_ui_with_layout(Vec2::new(ui.available_width(),32.0),
                            egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                self.tool_options(ui, available);
                            });
                    });
            });
        egui::SidePanel::left("editing_tools")
            .show_separator_line(false)
            .resizable(false)
            .exact_width(36.0)
            .frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(egui::Margin::symmetric(4, 4)))
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                ui.style_mut().interaction.tooltip_delay = 2.0;
                ui.style_mut().interaction.tooltip_grace_time = 0.0;
                egui::ScrollArea::vertical()
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                    ui.add_enabled_ui(available && self.start.is_none() && !self.pending(), |ui| {
                        for (tool, label, index) in [
                            (Tool::Hand, "Mão", 12),
                            (Tool::Select, "Selecionar e mover", 9),
                            (Tool::Crop, "Recortar", 10),
                            (Tool::Eyedropper, "Conta-gotas", 5),
                            (Tool::Heal, "Correção localizada", 2),
                            (Tool::Remove, "Remover", 13),
                            (Tool::Brush, "Pincel", 0),
                            (Tool::Clone, "Carimbo", 7),
                            (Tool::Bucket, "Balde de tinta", 1),
                            (Tool::Text, "Texto", 11),
                            (Tool::Rectangle, "Retângulo", 3),
                            (Tool::Ellipse, "Elipse", 4),
                            (Tool::Line, "Linha", 8),
                            (Tool::Arrow, "Seta", 6),
                        ] {
                            let hint = match tool_shortcut(tool) {
                                Some(key) => format!("{} ({key})", t(label)),
                                None => t(label),
                            };
                            let (rect, response) = ui.allocate_exact_size(Vec2::splat(28.0), egui::Sense::click());
                            response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &hint));
                            let visuals = ui.style().interact_selectable(&response, self.tool == tool);
                            ui.painter().rect(rect, 4.0, visuals.weak_bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);
                            crate::icons::paint_icon(ui, rect.shrink(3.0), index);
                            let response = response.on_hover_text(hint);
                            if response.clicked() || response.secondary_clicked() {
                                self.select_tool(tool);
                            }
                        }
                    });
                });
            });
        if let Some(text) = &mut self.text { text.refresh(ctx); }
    }
    fn tool_options(&mut self, ui: &mut egui::Ui, available: bool) {
        if self.tool == Tool::Text {
            if let Some(text) = &mut self.text { text.panel(ui); }
            return;
        }
        ui.add_enabled_ui(available && self.start.is_none() && !self.pending(), |ui| {
            if self.tool == Tool::Crop { self.crop_options(ui); }
            if !matches!(self.tool, Tool::Hand | Tool::Eyedropper | Tool::Select | Tool::Crop | Tool::Text) {
                ui.label(t(if matches!(self.tool, Tool::Rectangle | Tool::Ellipse) { "Espessura" } else { "Tamanho" }));
                ui.add(egui::Slider::new(&mut self.diameter, 1.0..=512.0).logarithmic(true).suffix(" px"));
            }
            if self.tool == Tool::Brush {
                ui.label(t("Dureza"));
                ui.add(egui::Slider::new(&mut self.hardness, 0.0..=1.0)
                    .custom_formatter(|n, _| format!("{:.0}%", n * 100.0)));
            }
            if self.tool == Tool::Bucket {
                ui.label(t("Tolerância"));
                ui.add(egui::Slider::new(&mut self.tolerance, 0..=255));
            }
            if matches!(self.tool, Tool::Ellipse | Tool::Rectangle) {
                preferences::color_swatch(ui, &t("Contorno"), &mut self.color);
                ui.checkbox(&mut self.filled, t("Preencher forma"));
                ui.add_enabled_ui(self.filled, |ui| {
                    preferences::color_swatch(ui, &t("Preenchimento"), &mut self.fill_color);
                });
            } else if !matches!(self.tool, Tool::Hand | Tool::Heal | Tool::Remove | Tool::Clone | Tool::Select | Tool::Crop | Tool::Text) {
                let label=if self.tool == Tool::Eyedropper {String::new()}else{t("Cor")};
                preferences::color_swatch(ui, &label, &mut self.color);
            }
            if self.tool == Tool::Eyedropper {
                ui.label("HEX");
                hex_color_button(ui,self.color);
            }
        });
        if self.floating.is_some() {
            if ui.button(t("Aplicar")).clicked() { self.selection_action = Some(true); }
            if ui.button(t("Cancelar")).clicked() { self.selection_action = Some(false); }
        }
    }
    pub fn sync_crop(&mut self, ratio: Option<f32>, dimensions: (u32,u32)) {
        self.crop_dimensions = dimensions;
        if self.crop_ratio != ratio {
            self.crop_ratio = ratio;
            if let Some(ratio) = ratio {
                if parsed_ratio(&self.crop_width,&self.crop_height).is_none_or(|r|(r-ratio).abs()>0.0001) {
                    if let Some((label,_)) = crate::RATIOS.iter().find(|(_,r)|r.is_some_and(|r|(r-ratio).abs()<0.0001)) {
                        if let Some((w,h)) = label.split_once(':') {
                            self.crop_width = w.into(); self.crop_height = h.into();
                            return;
                        }
                    }
                    self.crop_width = format!("{ratio:.4}"); self.crop_height = "1".into();
                }
            } else { self.crop_width.clear(); self.crop_height.clear(); }
        }
    }
    fn crop_options(&mut self, ui: &mut egui::Ui) {
        ui.label(t("Proporção"));
        let label = crate::RATIOS.iter().find(|(_,r)| *r == self.crop_ratio)
            .map(|(label,_)| *label).unwrap_or("Proporção manual");
        egui::ComboBox::from_id_salt("paint_crop_ratio").width(125.0).selected_text(t(label))
            .show_ui(ui,|ui| {
                for (label,ratio) in crate::RATIOS {
                    if ui.selectable_label(self.crop_ratio == ratio,t(label)).clicked() {
                        self.crop_ratio_requested = Some(ratio);
                        if let Some((w,h)) = label.split_once(':') {
                            self.crop_width = w.into(); self.crop_height = h.into();
                        } else if ratio.is_some() {
                            self.crop_width = self.crop_dimensions.0.to_string();
                            self.crop_height = self.crop_dimensions.1.to_string();
                        } else { self.crop_width.clear(); self.crop_height.clear(); }
                    }
                }
                if ui.selectable_label(false,t("Proporção manual")).clicked() {
                    self.crop_width = "2".into(); self.crop_height = "1".into();
                    self.crop_ratio_requested = Some(Some(2.0));
                }
            });
        let width = ui.add(egui::TextEdit::singleline(&mut self.crop_width).desired_width(56.0).hint_text(t("Largura")))
            .on_hover_text(t("Largura"));
        let invert = crate::icons::invert_ratio_button(ui).clicked();
        let height = ui.add(egui::TextEdit::singleline(&mut self.crop_height).desired_width(56.0).hint_text(t("Altura")))
            .on_hover_text(t("Altura"));
        if invert { std::mem::swap(&mut self.crop_width,&mut self.crop_height); }
        if width.changed() || height.changed() || invert {
            if self.crop_width.trim().is_empty() && self.crop_height.trim().is_empty() {
                self.crop_ratio_requested = Some(None);
            } else if let Some(ratio) = parsed_ratio(&self.crop_width,&self.crop_height) {
                self.crop_ratio_requested = Some(Some(ratio));
            }
        }
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
        self.heal_preview.clear();
        self.constraint_anchor = None;
        self.constraint_axis = None;
        self.start = None;
        self.last = None;
        self.path.clear();
        self.texture = None;
        self.pixels = None;
    }
    pub fn cursor(&self, painter: &egui::Painter, point: Pos2, sx: f32, sy: f32) {
        if !matches!(self.tool, Tool::Brush | Tool::Heal | Tool::Remove | Tool::Clone) {
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
        let fill = Color32::from_rgb(self.fill_color[0], self.fill_color[1], self.fill_color[2]);
        if self.filled {
            if self.tool == Tool::Rectangle { painter.rect_filled(rect, 0.0, fill); }
            else { painter.add(egui::epaint::EllipseShape::filled(rect.center(), rect.size()*0.5, fill)); }
        }
        if self.tool == Tool::Rectangle {
            painter.rect_stroke(rect, 0.0, Stroke::new(self.diameter*scale, color), egui::StrokeKind::Middle);
        } else {
            painter.add(egui::epaint::EllipseShape::stroke(rect.center(), rect.size()*0.5, Stroke::new(self.diameter*scale, color)));
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

fn hex_color_button(ui: &mut egui::Ui,color: [u8;3]) -> egui::Response {
    let hex=format!("#{:02X}{:02X}{:02X}",color[0],color[1],color[2]);
    let response=ui.button(egui::RichText::new(&hex).monospace())
        .on_hover_text(t("Copiar código hexadecimal"))
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if response.clicked() {ui.ctx().copy_text(hex);}
    response
}
fn parsed_ratio(width: &str,height: &str) -> Option<f32> {
    let w: f32 = width.trim().replace(',',".").parse().ok()?;
    let h: f32 = height.trim().replace(',',".").parse().ok()?;
    let ratio = w/h;
    (w > 0.0 && h > 0.0 && ratio.is_finite() && ratio > 0.0).then_some(ratio)
}
fn is_shape(tool: Tool) -> bool {
    matches!(tool, Tool::Rectangle | Tool::Ellipse | Tool::Line | Tool::Arrow)
}
fn tool_shortcut(tool: Tool) -> Option<&'static str> {
    match tool {
        Tool::Brush => Some("B"), Tool::Hand => Some("H"),
        Tool::Heal | Tool::Remove => Some("J / Shift+J"), Tool::Clone => Some("S"), Tool::Eyedropper => Some("I"),
        Tool::Bucket => Some("G"), Tool::Select => Some("M / V"),
        Tool::Text => Some("T"), Tool::Crop => Some("C"),
        Tool::Rectangle | Tool::Ellipse | Tool::Line | Tool::Arrow => Some("U / Shift+U"),
    }
}
#[cfg(test)]
mod shortcut_tests {
    use super::*;
    #[test]
    fn tool_keys_leave_modified_shortcuts_intact() {
        let editor = Editor::new();
        for (key, tool) in [(egui::Key::B,Tool::Brush),(egui::Key::H,Tool::Hand),(egui::Key::J,Tool::Heal),(egui::Key::U,Tool::Rectangle),
            (egui::Key::S,Tool::Clone),(egui::Key::I,Tool::Eyedropper),(egui::Key::G,Tool::Bucket),
            (egui::Key::M,Tool::Select),(egui::Key::V,Tool::Select),(egui::Key::C,Tool::Crop),(egui::Key::T,Tool::Text)] {
            assert!(editor.shortcut_tool(key,egui::Modifiers::NONE) == Some(tool));
            for modifiers in [egui::Modifiers::CTRL,egui::Modifiers::ALT,egui::Modifiers::COMMAND] {
                assert!(editor.shortcut_tool(key,modifiers).is_none());
            }
            if !matches!(key,egui::Key::U|egui::Key::J) { assert!(editor.shortcut_tool(key,egui::Modifiers::SHIFT).is_none()); }
        }
    }
    #[test]
    fn retouch_shortcut_cycles_and_remembers_the_remove_tool() {
        let mut editor=Editor::new();
        assert!(editor.shortcut_tool(egui::Key::J,egui::Modifiers::SHIFT)==Some(Tool::Remove));
        editor.select_tool(Tool::Remove);editor.select_tool(Tool::Brush);
        assert!(editor.shortcut_tool(egui::Key::J,egui::Modifiers::NONE)==Some(Tool::Remove));
        assert!(editor.shortcut_tool(egui::Key::J,egui::Modifiers::SHIFT)==Some(Tool::Heal));
    }
    #[test]
    fn shape_cycle_remembers_the_last_shape_and_requests_match_buttons() {
        let mut editor = Editor::new();
        editor.select_tool(Tool::Rectangle);
        for expected in [Tool::Ellipse,Tool::Line,Tool::Arrow,Tool::Rectangle] {
            let tool = editor.shortcut_tool(egui::Key::U,egui::Modifiers::SHIFT).unwrap();
            assert!(tool == expected);
            editor.select_tool(tool);
        }
        editor.select_tool(Tool::Arrow);
        editor.select_tool(Tool::Brush);
        assert!(editor.shortcut_tool(egui::Key::U,egui::Modifiers::NONE) == Some(Tool::Arrow));
        editor.select_tool(Tool::Crop);
        assert!(editor.crop_requested);
        editor.select_tool(Tool::Text);
        assert!(editor.text_requested);
    }
}

#[cfg(test)]
mod controls_layout_tests {
    use super::*;
    #[test]
    fn clicking_hex_code_copies_the_current_rgb_value() {
        let ctx=egui::Context::default();
        let mut rect=Rect::NOTHING;
        for phase in 0..3 {
            let events=if phase==0 {vec![]} else {vec![
                egui::Event::PointerMoved(rect.center()),
                egui::Event::PointerButton {pos:rect.center(),button:egui::PointerButton::Primary,
                    pressed:phase==1,modifiers:egui::Modifiers::NONE},
            ]};
            let output=ctx.run(egui::RawInput {events,screen_rect:Some(Rect::from_min_size(Pos2::ZERO,Vec2::new(500.0,100.0))),..Default::default()},|ctx| {
                egui::CentralPanel::default().show(ctx,|ui|{rect=hex_color_button(ui,[0,171,255]).rect;});
            });
            if phase==2 {assert!(output.platform_output.commands.iter().any(|command|
                matches!(command,egui::OutputCommand::CopyText(text) if text=="#00ABFF")));}
        }
    }
    #[test]
    fn switching_tools_keeps_the_canvas_and_narrow_toolbar_in_place() {
        let mut reference = None;
        for tool in [Tool::Hand, Tool::Brush, Tool::Clone, Tool::Rectangle, Tool::Ellipse, Tool::Text] {
            let ctx = egui::Context::default();
            let mut editor = Editor::new();
            editor.enabled = true;
            editor.tool = tool;
            let image = std::sync::Arc::new(RgbaImage::new(100,100));
            if tool == Tool::Text { editor.text = Some(crate::text_layer::TextLayer::new(&image,[0;3])); }
            let mut canvas = Rect::NOTHING;
            let _ = ctx.run(egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1100.0, 800.0))),
                ..Default::default()
            }, |ctx| {
                editor.panel(ctx, true, 60.0);
                egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx, |ui| { canvas = ui.max_rect(); });
            });
            assert!(canvas.left() <= 48.0, "toolbar expanded");
            assert!(canvas.top() <= 108.0, "controls overflowed the reserved row: {canvas:?}");
            if let Some(previous) = reference { assert_eq!(canvas, previous); }
            else { reference = Some(canvas); }
        }
    }
    #[test]
    fn crop_ratio_fields_validate_and_preserve_custom_values() {
        assert_eq!(parsed_ratio("16","9"),Some(16.0/9.0));
        assert_eq!(parsed_ratio("1,5","1"),Some(1.5));
        for (w,h) in [("0","1"),("1","0"),("NaN","1"),("inf","1"),("-2","1"),("","9")] {
            assert!(parsed_ratio(w,h).is_none());
        }
        let mut editor = Editor::new();
        editor.sync_crop(Some(16.0/9.0),(1920,1080));
        assert_eq!((&*editor.crop_width,&*editor.crop_height),("16","9"));
        editor.crop_width="30".into(); editor.crop_height="20".into();
        editor.sync_crop(Some(1.5),(1920,1080));
        assert_eq!((&*editor.crop_width,&*editor.crop_height),("30","20"));
        editor.sync_crop(None,(1920,1080));
        assert!(editor.crop_width.is_empty() && editor.crop_height.is_empty());
    }
}
