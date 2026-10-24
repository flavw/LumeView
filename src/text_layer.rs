//! Floating text uses one supersampled raster for both preview and final compositing.
use crate::{i18n::t, preferences};
use ab_glyph::{point, Font, FontArc, ScaleFont};
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use image::{GrayImage, Luma, RgbaImage};
use std::{
    path::PathBuf,
    sync::{Arc, Weak},
};
pub fn fonts() -> Vec<(String, PathBuf)> {
    #[cfg(windows)]
    let candidates = {
        let root = PathBuf::from(std::env::var_os("WINDIR").unwrap_or_else(|| "C:/Windows".into()))
            .join("Fonts");
        vec![
            ("Arial", root.join("arial.ttf")),
            ("Times New Roman", root.join("times.ttf")),
            ("Consolas", root.join("consola.ttf")),
        ]
    };
    #[cfg(not(windows))]
    let candidates = vec![
        (
            "DejaVu Sans",
            PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
        ),
        (
            "DejaVu Serif",
            PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSerif.ttf"),
        ),
        (
            "DejaVu Sans Mono",
            PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf"),
        ),
    ];
    candidates
        .into_iter()
        .filter(|(_, p)| p.is_file())
        .map(|(name, path)| (name.to_owned(), path))
        .collect()
}
pub fn raster(
    font: &FontArc,
    text: &str,
    size: f32,
    color: [u8; 3],
) -> Result<Option<RgbaImage>, String> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    let scale = font.as_scaled(size.clamp(6.0, 512.0) * 2.0);
    let line_height = scale.height() + scale.line_gap();
    let mut x = 0.0f32;
    let mut baseline = scale.ascent();
    let mut previous = None;
    let mut outlines = Vec::new();
    let mut min = point(0.0, 0.0);
    let mut max = point(0.0, line_height);
    for ch in text.chars() {
        if ch == '\n' {
            x = 0.0;
            baseline += line_height;
            max.y = max.y.max(baseline - scale.descent());
            previous = None;
            continue;
        }
        if ch == '\r' {
            continue;
        }
        let id = scale.glyph_id(ch);
        if let Some(last) = previous {
            x += scale.kern(last, id);
        }
        let glyph = id.with_scale_and_position(scale.scale(), point(x, baseline));
        if let Some(outline) = font.outline_glyph(glyph) {
            let bounds = outline.px_bounds();
            min.x = min.x.min(bounds.min.x);
            min.y = min.y.min(bounds.min.y);
            max.x = max.x.max(bounds.max.x);
            max.y = max.y.max(bounds.max.y);
            outlines.push(outline);
        }
        x += scale.h_advance(id);
        max.x = max.x.max(x);
        previous = Some(id);
        if max.x - min.x > 8190.0 || max.y - min.y > 8190.0 {
            return Err(t(
                "O texto é grande demais. Reduza o tamanho ou divida em linhas.",
            ));
        }
    }
    let width = ((max.x - min.x).ceil() as u32 + 8).div_ceil(2) * 2;
    let height = ((max.y - min.y).ceil() as u32 + 8).div_ceil(2) * 2;
    if u64::from(width) * u64::from(height) > 16_000_000 {
        return Err(t(
            "O texto é grande demais. Reduza o tamanho ou divida em linhas.",
        ));
    }
    let mut alpha = GrayImage::new(width, height);
    for outline in outlines {
        let bounds = outline.px_bounds();
        let ox = (bounds.min.x - min.x).round() as u32 + 4;
        let oy = (bounds.min.y - min.y).round() as u32 + 4;
        outline.draw(|x, y, coverage| {
            let x = x + ox;
            let y = y + oy;
            if x < width && y < height {
                let old = alpha.get_pixel(x, y)[0] as f32 / 255.0;
                alpha.put_pixel(
                    x,
                    y,
                    Luma([((coverage + old * (1.0 - coverage)) * 255.0).round() as u8]),
                );
            }
        });
    }
    let alpha = image::imageops::resize(
        &alpha,
        width / 2,
        height / 2,
        image::imageops::FilterType::Lanczos3,
    );
    let mut pixels = RgbaImage::new(width / 2, height / 2);
    for (x, y, a) in alpha.enumerate_pixels() {
        pixels.put_pixel(
            x,
            y,
            image::Rgba(if a[0] == 0 {
                [0; 4]
            } else {
                [color[0], color[1], color[2], a[0]]
            }),
        );
    }
    Ok(Some(pixels))
}
fn soft_shadow(text: &RgbaImage, size: f32) -> (RgbaImage, i32) {
    let sigma = (size * 0.035).clamp(1.0, 8.0);
    let offset = (size * 0.04).clamp(1.0, 10.0).round() as u32;
    let padding = (sigma * 3.0).ceil() as u32 + offset + 1;
    let mut mask = GrayImage::new(text.width() + padding * 2, text.height() + padding * 2);
    for (x,y,p) in text.enumerate_pixels() {
        mask.put_pixel(x + padding + offset,y + padding + offset,Luma([p[3]]));
    }
    let mask = image::imageops::blur(&mask,sigma);
    let mut pixels = RgbaImage::from_fn(mask.width(),mask.height(),|x,y| {
        image::Rgba([0,0,0,(f32::from(mask.get_pixel(x,y)[0])*0.45).round() as u8])
    });
    image::imageops::overlay(&mut pixels,text,i64::from(padding),i64::from(padding));
    (pixels,padding as i32)
}
pub struct TextLayer {
    pub text: String,
    pub size: f32,
    pub color: [u8; 3],
    pub shadow: bool,
    pub inset: i32,
    pub position: [i32; 2],
    pub image: Weak<RgbaImage>,
    pub pixels: Option<Arc<RgbaImage>>,
    pub texture: Option<egui::TextureHandle>,
    pub action: Option<bool>,
    fonts: Vec<(String, PathBuf)>,
    selected: usize,
    font: Option<FontArc>,
    font_initialized: bool,
    dirty: bool,
    focus: bool,
    pub error: Option<String>,
    grab: Option<([f32; 2], [i32; 2])>,
}
impl TextLayer {
    pub fn new(im: &Arc<RgbaImage>, color: [u8; 3]) -> Self {
        let fonts = fonts();
        Self {
            text: String::new(),
            size: 48.0,
            color,
            shadow: false,
            inset: 0,
            position: [(im.width() / 12) as i32, (im.height() / 12) as i32],
            image: Arc::downgrade(im),
            pixels: None,
            texture: None,
            action: None,
            fonts,
            selected: 0,
            font: None,
            font_initialized: false,
            dirty: true,
            focus: true,
            error: None,
            grab: None,
        }
    }
    fn load_font(&mut self) {
        self.font_initialized = true;
        self.font = None;
        self.error = None;
        if let Some((_, path)) = self.fonts.get(self.selected) {
            match std::fs::read(path)
                .ok()
                .and_then(|bytes| FontArc::try_from_vec(bytes).ok())
            {
                Some(font) => self.font = Some(font),
                None => self.error = Some(t("Não foi possível carregar a fonte.")),
            }
        } else {
            self.error = Some(t("Escolha um arquivo de fonte."));
        }
        self.dirty = true;
    }
    pub fn panel(&mut self, ui: &mut egui::Ui) {
        let response = text_input(ui,&mut self.text);
        if self.focus {
            response.request_focus();
            self.focus = false;
        }
        self.dirty |= response.changed();
        let before = self.selected;
        egui::ComboBox::from_id_salt("paint_font")
            .width(145.0)
            .selected_text(
                self.fonts
                    .get(self.selected)
                    .map(|(name, _)| name.as_str())
                    .unwrap_or("—"),
            )
            .show_ui(ui, |ui| {
                for (index, (name, _)) in self.fonts.iter().enumerate() {
                    ui.selectable_value(&mut self.selected, index, name);
                }
            });
        if self.selected != before || !self.font_initialized {
            self.load_font();
        }
        if ui.button(t("Escolher fonte")).clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("TrueType / OpenType", &["ttf", "otf"])
                .pick_file()
            {
                let name = path
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned();
                self.fonts.push((name, path));
                self.selected = self.fonts.len() - 1;
                self.load_font();
            }
        }
        ui.horizontal(|ui| {
            ui.label(t("Tamanho"));
            self.dirty |= ui
                .add(
                    egui::DragValue::new(&mut self.size)
                        .range(6.0..=512.0)
                        .suffix(" px"),
                )
                .changed();
        });
        let previous = self.color;
        preferences::color_swatch(ui, &t("Cor"), &mut self.color);
        self.dirty |= previous != self.color;
        self.dirty |= ui.checkbox(&mut self.shadow,t("Sombra suave")).changed();
        if let Some(error) = &self.error {
            ui.colored_label(Color32::LIGHT_RED, error);
        }
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    self.pixels.is_some() && !self.dirty,
                    egui::Button::new(t("Aplicar")),
                )
                .clicked()
            {
                self.action = Some(true);
            }
            if ui.button(t("Cancelar")).clicked() {
                self.action = Some(false);
            }
        });
    }
    pub fn refresh(&mut self, ctx: &egui::Context) {
        if !self.dirty {
            return;
        }
        self.dirty = false;
        self.pixels = None;
        self.texture = None;
        self.inset = 0;
        if let Some(font) = &self.font {
            match raster(font, &self.text, self.size, self.color) {
                Ok(Some(pixels)) => {
                    let pixels = if self.shadow {
                        let (pixels,inset) = soft_shadow(&pixels,self.size);
                        self.inset = inset;
                        pixels
                    } else { pixels };
                    let pixels = Arc::new(pixels);
                    self.texture = Some(ctx.load_texture(
                        "floating-text",
                        egui::ColorImage::from_rgba_unmultiplied(
                            [pixels.width() as usize, pixels.height() as usize],
                            pixels.as_raw(),
                        ),
                        egui::TextureOptions::LINEAR,
                    ));
                    self.pixels = Some(pixels);
                    self.error = None;
                }
                Ok(None) => self.error = None,
                Err(error) => self.error = Some(error),
            }
        }
    }
    pub fn canvas(
        &mut self,
        ctx: &egui::Context,
        ui: &egui::Ui,
        response: &egui::Response,
        source_rect: Rect,
        shown: Rect,
        scale: Vec2,
        rotation: crate::view_rotation::Rotation,
        dimensions: (u32, u32),
    ) {
        let source_point = |p: Pos2| {
            let p = rotation.inverse().point(p, shown.center());
            [
                (p.x - source_rect.left()) / scale.x,
                (p.y - source_rect.top()) / scale.y,
            ]
        };
        let size = self
            .pixels
            .as_ref()
            .map(|p| Vec2::new(p.width() as f32 * scale.x, p.height() as f32 * scale.y))
            .unwrap_or(Vec2::new(120.0, 48.0));
        let rect = Rect::from_min_size(
            source_rect.min
                + Vec2::new(
                    (self.position[0] - self.inset) as f32 * scale.x,
                    (self.position[1] - self.inset) as f32 * scale.y,
                ),
            size,
        );
        let bounds = rotation.rect(rect, shown.center());
        if response.hovered() {
            ctx.set_cursor_icon(
                if ctx
                    .input(|i| i.pointer.hover_pos())
                    .is_some_and(|p| bounds.contains(p))
                {
                    egui::CursorIcon::Grab
                } else {
                    egui::CursorIcon::Text
                },
            );
        }
        if response.hovered()
            && ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary))
        {
            if let Some(origin) = ctx.input(|i| {
                i.events
                    .iter()
                    .find_map(|event| match event {
                        egui::Event::PointerButton {
                            pos,
                            button: egui::PointerButton::Primary,
                            pressed: true,
                            ..
                        } => Some(*pos),
                        _ => None,
                    })
                    .or_else(|| i.pointer.press_origin())
            }) {
                let p = source_point(origin);
                if bounds.contains(origin) {
                    self.grab = Some((p, self.position));
                } else if shown.contains(origin) {
                    self.position = [p[0].round() as i32, p[1].round() as i32];
                    self.grab = Some((p, self.position));
                }
            }
        }
        if let (Some((origin, position)), Some(pointer)) =
            (self.grab, ctx.input(|i| i.pointer.interact_pos()))
        {
            let p = source_point(pointer);
            self.position = [
                (position[0] + (p[0] - origin[0]).round() as i32)
                    .clamp(0, dimensions.0.saturating_sub(1) as i32),
                (position[1] + (p[1] - origin[1]).round() as i32)
                    .clamp(0, dimensions.1.saturating_sub(1) as i32),
            ];
            ctx.set_cursor_icon(egui::CursorIcon::Grabbing);
        }
        if ctx.input(|i| {
            i.pointer.button_released(egui::PointerButton::Primary)
                || i.viewport().focused == Some(false)
        }) {
            self.grab = None;
        }
        let rect = Rect::from_min_size(
            source_rect.min
                + Vec2::new(
                    (self.position[0] - self.inset) as f32 * scale.x,
                    (self.position[1] - self.inset) as f32 * scale.y,
                ),
            size,
        );
        if let Some(texture) = &self.texture {
            rotation.paint(
                ui.painter(),
                texture.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                shown.center(),
            );
        }
        let bounds = rotation.rect(rect, shown.center());
        ui.painter().rect_stroke(
            bounds,
            0.0,
            Stroke::new(1.0_f32, Color32::LIGHT_BLUE),
            egui::StrokeKind::Inside,
        );
    }
}
fn text_input(ui: &mut egui::Ui, text: &mut String) -> egui::Response {
    // Match the visible editor's height rather than centering a taller, empty container.
    let height = ui.text_style_height(&egui::TextStyle::Body) + 4.0;
    ui.allocate_ui_with_layout(egui::vec2(220.0,height),egui::Layout::top_down(egui::Align::Min),|ui| {
        egui::ScrollArea::vertical().id_salt("paint_text_input")
            .min_scrolled_height(height).max_height(height).show(ui,|ui| {
                ui.add(egui::TextEdit::multiline(text)
                    .desired_rows(1).desired_width(210.0).char_limit(2000)
                    .hint_text(t("Digite o texto")))
            }).inner
    }).inner
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn typing_field_and_font_selector_have_the_same_vertical_center() {
        let ctx = egui::Context::default();
        for content in ["","Lume"] {
            let mut text = content.to_owned();
            let mut centers = (0.0_f32,0.0_f32);
            let _ = ctx.run(egui::RawInput::default(),|ctx| {
                egui::CentralPanel::default().show(ctx,|ui| {
                    ui.allocate_ui_with_layout(Vec2::new(900.0,32.0),egui::Layout::left_to_right(egui::Align::Center),|ui| {
                        centers.0 = text_input(ui,&mut text).rect.center().y;
                        centers.1 = egui::ComboBox::from_id_salt("test_font").selected_text("Arial")
                            .show_ui(ui,|ui| {ui.label("Arial");}).response.rect.center().y;
                    });
                });
            });
            assert!((centers.0-centers.1).abs()<=0.5,"field and font selector centers: {centers:?}");
        }
    }
    #[test]
    fn soft_shadow_has_a_blurred_black_halo_and_preserves_opaque_text() {
        let text = RgbaImage::from_fn(12,12,|x,y|image::Rgba(if (3..9).contains(&x) && (3..9).contains(&y) {[255;4]} else {[0;4]}));
        let (shadow,padding) = soft_shadow(&text,48.0);
        assert_eq!(shadow.dimensions(),(12+padding as u32*2,12+padding as u32*2));
        assert_eq!(*shadow.get_pixel(padding as u32+5,padding as u32+5),image::Rgba([255;4]));
        assert!(shadow.pixels().any(|p|p[0]==0 && p[1]==0 && p[2]==0 && p[3]>0 && p[3]<115));
        assert_eq!(*shadow.get_pixel(0,0),image::Rgba([0;4]));
    }
    #[test]
    fn text_is_antialiased_multiline_and_has_clean_color() {
        let Some((_, path)) = fonts().into_iter().next() else {
            return;
        };
        let font = FontArc::try_from_vec(std::fs::read(path).unwrap()).unwrap();
        let one = raster(&font, "Olá", 32.0, [240, 30, 10]).unwrap().unwrap();
        let two = raster(&font, "Olá\nLume", 32.0, [240, 30, 10])
            .unwrap()
            .unwrap();
        assert!(two.height() > one.height());
        assert!(one.pixels().any(|p| p[3] > 0 && p[3] < 255));
        assert!(one
            .pixels()
            .filter(|p| p[3] > 0)
            .all(|p| p[0] == 240 && p[1] == 30 && p[2] == 10));
        assert!(raster(&font, "", 32.0, [0; 3]).unwrap().is_none());
    }
    #[test]
    fn oversized_text_is_rejected_before_allocating() {
        let Some((_, path)) = fonts().into_iter().next() else {
            return;
        };
        let font = FontArc::try_from_vec(std::fs::read(path).unwrap()).unwrap();
        assert!(raster(&font, &"W".repeat(2000), 512.0, [0; 3]).is_err());
    }
}
