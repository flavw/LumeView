use image::RgbaImage;
use eframe::egui;

#[derive(Clone, Copy)]
pub enum Effect {
    Grayscale, Sepia, Negative,
    BrightnessContrast { brightness: f32, contrast: f32 },
    Saturation(f32), Temperature(f32), Posterize(u8),
}

pub struct Adjustment {
    pub effect: Effect,
    source: RgbaImage,
    preview: Option<egui::TextureHandle>,
}
impl Adjustment {
    pub fn new(source: &RgbaImage, effect: Effect) -> Self {
        Self { effect, source: image::imageops::thumbnail(source, 360, 240), preview: None }
    }
    pub fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        use crate::i18n::t;
        let mut changed = false;
        match &mut self.effect {
            Effect::BrightnessContrast { brightness, contrast } => {
                changed |= ui.add(egui::Slider::new(brightness, -100.0..=100.0).text(t("Brilho"))).changed();
                changed |= ui.add(egui::Slider::new(contrast, -100.0..=100.0).text(t("Contraste"))).changed();
            }
            Effect::Saturation(value) => { changed |= ui.add(egui::Slider::new(value, 0.0..=200.0).suffix("%").text(t("Saturação"))).changed(); }
            Effect::Temperature(value) => {
                changed |= ui.add(egui::Slider::new(value, -100.0..=100.0).text(t("Temperatura"))).changed();
                ui.small(t("Negativo: frio · Positivo: quente"));
            }
            Effect::Posterize(levels) => { changed |= ui.add(egui::Slider::new(levels, 2..=32).text(t("Níveis por canal"))).changed(); }
            _ => {}
        }
        if changed || self.preview.is_none() {
            let pixels = apply(&self.source, self.effect);
            let color = egui::ColorImage::from_rgba_unmultiplied([pixels.width() as usize, pixels.height() as usize], pixels.as_raw());
            if let Some(texture) = &mut self.preview { texture.set(color, egui::TextureOptions::LINEAR); }
            else { self.preview = Some(ui.ctx().load_texture("effect-preview", color, egui::TextureOptions::LINEAR)); }
        }
        if let Some(texture) = &self.preview { ui.image((texture.id(), texture.size_vec2())); }
        ui.button(t("Aplicar")).clicked()
    }
}

pub fn apply(source: &RgbaImage, effect: Effect) -> RgbaImage {
    let mut result = source.clone();
    for pixel in result.pixels_mut() {
        let [r, g, b, _] = pixel.0.map(f32::from);
        let channels = match effect {
            Effect::Grayscale => {
                let luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
                [luminance; 3]
            }
            Effect::Negative => [255.0-r, 255.0-g, 255.0-b],
            Effect::BrightnessContrast { brightness, contrast } => {
                let c = contrast.clamp(-100.0, 100.0) * 2.55;
                let factor = 259.0 * (c + 255.0) / (255.0 * (259.0 - c));
                [r,g,b].map(|v| factor * (v - 128.0) + 128.0 + brightness * 2.55)
            }
            Effect::Saturation(amount) => {
                let gray = 0.2126*r + 0.7152*g + 0.0722*b;
                [r,g,b].map(|v| gray + (v-gray)*amount/100.0)
            }
            Effect::Temperature(amount) => [r + amount*0.6, g + amount*0.1, b - amount*0.6],
            Effect::Posterize(levels) => {
                let steps = f32::from(levels.clamp(2, 32)-1);
                [r,g,b].map(|v| (v*steps/255.0).round()*255.0/steps)
            }
            Effect::Sepia => [
                0.393 * r + 0.769 * g + 0.189 * b,
                0.349 * r + 0.686 * g + 0.168 * b,
                0.272 * r + 0.534 * g + 0.131 * b,
            ],
        };
        for (target, value) in pixel.0[..3].iter_mut().zip(channels) {
            *target = value.round().clamp(0.0, 255.0) as u8;
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adjustments_preserve_alpha_and_neutral_values() {
        let source = RgbaImage::from_pixel(1, 1, image::Rgba([40, 120, 210, 71]));
        for effect in [Effect::BrightnessContrast { brightness: 0.0, contrast: 0.0 }, Effect::Saturation(100.0), Effect::Temperature(0.0)] {
            assert_eq!(apply(&source, effect), source);
        }
        assert_eq!(apply(&apply(&source, Effect::Negative), Effect::Negative), source);
        let gray = apply(&source, Effect::Saturation(0.0));
        assert_eq!(gray[ (0,0) ][0], gray[(0,0)][2]);
        assert_eq!(apply(&source, Effect::Posterize(2))[(0,0)].0, [0,0,255,71]);
        for effect in [Effect::Temperature(100.0), Effect::Saturation(200.0), Effect::BrightnessContrast { brightness: 100.0, contrast: 100.0 }] {
            assert_eq!(apply(&source, effect)[(0,0)][3], 71);
        }
    }
    #[test]
    fn colors_and_transparency() {
        let source = RgbaImage::from_pixel(1, 1, image::Rgba([255, 255, 255, 37]));
        assert_eq!(apply(&source, Effect::Sepia).get_pixel(0, 0).0, [255, 255, 239, 37]);
        let red = RgbaImage::from_pixel(1, 1, image::Rgba([255, 0, 0, 128]));
        assert_eq!(apply(&red, Effect::Grayscale).get_pixel(0, 0).0, [54, 54, 54, 128]);
        assert_eq!(red.get_pixel(0, 0).0, [255, 0, 0, 128]);
    }
}
