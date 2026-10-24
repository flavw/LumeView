use eframe::egui::{self, Pos2, Sense, Stroke, Vec2};

const ARROWS: [(char, &str); 4] = [
    ('←', include_str!("../assets/icons/arrow-left.svg")),
    ('→', include_str!("../assets/icons/arrow-right.svg")),
    ('↑', include_str!("../assets/icons/arrow-up.svg")),
    ('↓', include_str!("../assets/icons/arrow-down.svg")),
];

// These four embedded SVGs only contain a polyline. Render its vector points
// directly with egui so the viewer needs no SVG rasterizer or symbol font.
fn points(svg: &str) -> Vec<Pos2> {
    svg.split("points=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .split_whitespace()
        .map(|pair| {
            let (x, y) = pair.split_once(',').unwrap();
            Pos2::new(x.parse().unwrap(), y.parse().unwrap())
        })
        .collect()
}

pub fn shortcut(ui: &mut egui::Ui, text: &str) {
    let mut segment = String::new();
    for ch in text.chars() {
        if let Some((_, svg)) = ARROWS.iter().find(|(arrow, _)| *arrow == ch) {
            if !segment.is_empty() {
                ui.strong(&segment);
                segment.clear();
            }
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(18.0), Sense::hover());
            let points = points(svg)
                .into_iter()
                .map(|p| rect.min + p.to_vec2() * (18.0 / 24.0))
                .collect();
            ui.painter().add(egui::Shape::line(
                points,
                Stroke::new(1.7_f32, ui.visuals().text_color()),
            ));
        } else {
            segment.push(ch);
        }
    }
    if !segment.is_empty() {
        ui.strong(segment);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn svg_arrows_have_valid_vector_points() {
        for (_, svg) in ARROWS {
            let p = points(svg);
            assert_eq!(p.len(), 3);
            assert!(p
                .iter()
                .all(|p| p.x >= 0.0 && p.y >= 0.0 && p.x <= 24.0 && p.y <= 24.0));
        }
    }
}

/// Small vector buttons avoid platform-dependent icon fonts and bitmap assets.
pub fn tool_button(ui: &mut egui::Ui, label: &str, enabled: bool) -> bool {
    let mut clicked = false;
    ui.add_enabled_ui(enabled, |ui| {
        let (r, response) =
            ui.allocate_exact_size(Vec2::splat(ui.spacing().interact_size.y), Sense::click());
        let v = ui.style().interact(&response);
        let fade =
            ui.ctx()
                .animate_bool_with_time(response.id.with("hover"), response.hovered(), 0.12);
        ui.painter()
            .rect_filled(r, 4.0, v.bg_fill.gamma_multiply(0.35 + fade * 0.65));
        if response.has_focus() {
            ui.painter()
                .rect_stroke(r, 4.0, v.bg_stroke, egui::StrokeKind::Inside);
        }
        let scale = r.width() / 30.0;
        let stroke = Stroke::new((1.7_f32 * scale).max(1.0), v.fg_stroke.color);
        let point = |x: f32, y: f32| r.min + Vec2::new(5.0 + x, 5.0 + y) * scale;
        let line = |points: &[(f32, f32)]| {
            ui.painter().add(egui::Shape::line(
                points.iter().map(|&(x, y)| point(x, y)).collect(),
                stroke,
            ))
        };
        let rect = |x: f32, y: f32, w: f32, h: f32| {
            ui.painter().rect_stroke(
                egui::Rect::from_min_size(point(x, y), Vec2::new(w, h) * scale),
                1.0,
                stroke,
                egui::StrokeKind::Inside,
            )
        };
        match label {
            "Mover para cima" | "Mover para baixo" => {
                let y = |n: f32| if label == "Mover para baixo" { 20. - n } else { n };
                line(&[(10., y(17.)), (10., y(3.))]);
                line(&[(4., y(9.)), (10., y(3.)), (16., y(9.))]);
            }
            "Remover da lista" => {
                line(&[(5., 5.), (15., 15.)]);
                line(&[(15., 5.), (5., 15.)]);
            }
            "Abrir" => {
                line(&[
                    (1., 16.),
                    (1., 4.),
                    (8., 4.),
                    (10., 7.),
                    (19., 7.),
                    (16., 16.),
                    (1., 16.),
                ]);
                line(&[(1., 9.), (17., 9.)]);
            }
            "Salvar" => {
                rect(2., 2., 16., 16.);
                rect(6., 2., 8., 6.);
                rect(6., 11., 8., 7.);
            }
            "Enviar à lixeira" => {
                rect(5., 6., 10., 12.);
                line(&[(3., 4.), (17., 4.)]);
                line(&[(7., 2.), (13., 2.)]);
                line(&[(8., 8.), (8., 15.)]);
                line(&[(12., 8.), (12., 15.)]);
            }
            "Copiar" => {
                rect(3., 2., 10., 13.);
                rect(7., 6., 10., 13.);
            }
            "Colar" => {
                rect(4., 4., 12., 15.);
                rect(7., 1., 6., 5.);
                line(&[(7., 10.), (13., 10.)]);
                line(&[(7., 14.), (13., 14.)]);
            }
            "Anterior" | "Próxima" => {
                let flip = label == "Próxima";
                let x = |n: f32| if flip { 20. - n } else { n };
                line(&[(x(17.), 10.), (x(3.), 10.)]);
                line(&[(x(9.), 4.), (x(3.), 10.), (x(9.), 16.)]);
            }
            "+" | "−" => {
                ui.painter()
                    .circle_stroke(point(8., 8.), 6. * scale, stroke);
                line(&[(12., 12.), (18., 18.)]);
                line(&[(5., 8.), (11., 8.)]);
                if label == "+" {
                    line(&[(8., 5.), (8., 11.)]);
                }
            }
            "Desfazer" => {
                line(&[(7., 3.), (2., 8.), (7., 12.)]);
                line(&[(2., 8.), (12., 8.), (16., 11.), (16., 16.), (12., 18.)]);
            }
            "Ajustar" => {
                rect(1., 3., 18., 14.);
                line(&[(5., 10.), (15., 10.)]);
                line(&[(8., 7.), (5., 10.), (8., 13.)]);
                line(&[(12., 7.), (15., 10.), (12., 13.)]);
            }
            "1:1" => {
                ui.painter().text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    "1:1",
                    egui::FontId::proportional(12.0 * scale),
                    v.fg_stroke.color,
                );
            }
            "Informações da imagem" => {
                ui.painter()
                    .circle_stroke(point(10., 10.), 8. * scale, stroke);
                line(&[(10., 9.), (10., 15.)]);
                ui.painter()
                    .circle_filled(point(10., 5.), scale, v.fg_stroke.color);
            }
            "Preferências" => {
                for (x, y) in [(4., 6.), (10., 14.), (16., 9.)] {
                    line(&[(x, 2.), (x, 18.)]);
                    ui.painter()
                        .circle_filled(point(x, y), 2.5 * scale, v.bg_fill);
                    ui.painter().circle_stroke(point(x, y), 2.5 * scale, stroke);
                }
            }
            "Tela cheia / janela" => {
                line(&[(2., 7.), (2., 2.), (7., 2.)]);
                line(&[(13., 2.), (18., 2.), (18., 7.)]);
                line(&[(18., 13.), (18., 18.), (13., 18.)]);
                line(&[(7., 18.), (2., 18.), (2., 13.)]);
            }
            "Parar apresentação" => {
                rect(4., 4., 12., 12.);
            }
            _ => {
                line(&[(5., 3.), (17., 10.), (5., 17.), (5., 3.)]);
            }
        }
        clicked = response.clicked();
        response.on_hover_text(crate::i18n::t(label));
    });
    clicked
}

/// Two opposite arrows, drawn directly to avoid missing symbol-font glyphs.
pub fn invert_ratio_button(ui: &mut egui::Ui) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(34.0, 28.0), Sense::click());
    let visuals = ui.style().interact(&response);
    ui.painter().rect(
        rect,
        4.0,
        visuals.bg_fill,
        visuals.bg_stroke,
        egui::StrokeKind::Inside,
    );
    let stroke = Stroke::new(1.8_f32, visuals.fg_stroke.color);
    let center = rect.center();
    let at = |x, y| center + Vec2::new(x, y);
    for (y, direction) in [(-4.0_f32, 1.0_f32), (4.0_f32, -1.0_f32)] {
        ui.painter().line_segment([at(-9.0, y), at(9.0, y)], stroke);
        let tip = 9.0 * direction;
        ui.painter()
            .line_segment([at(tip - 4.0 * direction, y - 3.0), at(tip, y)], stroke);
        ui.painter()
            .line_segment([at(tip - 4.0 * direction, y + 3.0), at(tip, y)], stroke);
    }
    response.on_hover_text(crate::i18n::t("Inverter proporção"))
}

const PAINT_SVGS:[&str;14]=[
 include_str!("../assets/icons/paint-brush.svg"),include_str!("../assets/icons/paint-bucket.svg"),
 include_str!("../assets/icons/paint-heal.svg"),include_str!("../assets/icons/paint-rectangle.svg"),
 include_str!("../assets/icons/paint-ellipse.svg"),include_str!("../assets/icons/paint-eyedropper.svg"),
 include_str!("../assets/icons/paint-arrow.svg"),include_str!("../assets/icons/paint-clone.svg"),include_str!("../assets/icons/paint-line.svg"),include_str!("../assets/icons/paint-select.svg"),include_str!("../assets/icons/paint-crop.svg"),include_str!("../assets/icons/paint-text.svg"),include_str!("../assets/icons/paint-hand.svg"),include_str!("../assets/icons/paint-remove.svg")];
struct PaintPath { points:Vec<Pos2>,filled:bool,color:Option<egui::Color32>,width:f32 }
fn paint_paths(svg:&str)->Vec<PaintPath>{
 svg.split('<').filter(|tag|tag.starts_with("polygon ")||tag.starts_with("polyline ")).map(|tag| {
  let color=tag.split("=\"#").nth(1).and_then(|value|value.split('"').next())
   .filter(|value|value.len()==6).and_then(|value|u32::from_str_radix(value,16).ok())
   .map(|rgb|egui::Color32::from_rgb((rgb>>16) as u8,(rgb>>8) as u8,rgb as u8));
  let width=tag.split("stroke-width=\"").nth(1).and_then(|value|value.split('"').next())
   .and_then(|value|value.parse::<f32>().ok()).filter(|width|width.is_finite() && *width>0.0).unwrap_or(1.7);
  PaintPath{points:points(tag),filled:tag.starts_with("polygon "),color,width}
 }).collect()
}
/// Render the embedded SVG polygon/polyline subset directly; no bitmap rasterizer.
pub fn paint_icon(ui:&egui::Ui,rect:egui::Rect,index:usize){
 static PATHS:std::sync::OnceLock<Vec<Vec<PaintPath>>>=std::sync::OnceLock::new();
 let icons=PATHS.get_or_init(||PAINT_SVGS.iter().map(|svg|paint_paths(svg)).collect());
 let scale=rect.width()/24.0;
 for path in &icons[index]{
  let points=path.points.iter().map(|p|rect.min+p.to_vec2()*scale).collect();
  let color=path.color.unwrap_or_else(||ui.visuals().text_color());
  if path.filled{ui.painter().add(egui::Shape::convex_polygon(points,color,Stroke::NONE));}
  else{ui.painter().add(egui::Shape::line(points,Stroke::new(path.width*scale,color)));}
 }
}
#[cfg(test)] mod paint_tests {
 use super::*;
 #[test] fn bold_tool_icons_respect_svg_stroke_width(){
  assert!(paint_paths(PAINT_SVGS[11]).iter().all(|path|path.width==3.2));
  for index in [6,8]{assert!(paint_paths(PAINT_SVGS[index]).iter().all(|path|path.width==2.8));}
 }
 #[test] fn all_paint_svg_paths_are_valid(){for svg in PAINT_SVGS {let paths=paint_paths(svg);assert!(!paths.is_empty());for path in paths {assert!(path.points.len()>=2);assert!(path.points.iter().all(|p|p.x>=0.0&&p.x<=24.0&&p.y>=0.0&&p.y<=24.0));}}}
 #[test] fn bandages_have_identical_geometry_and_remove_only_adds_sparkles(){
  let heal=paint_paths(PAINT_SVGS[2]);let remove=paint_paths(PAINT_SVGS[13]);
  assert!(remove.len()>heal.len());
  for (a,b) in heal.iter().zip(&remove){assert_eq!(a.points,b.points);assert_eq!(a.filled,b.filled);assert_eq!(a.color,b.color);}
  assert_eq!(heal[1].color,Some(egui::Color32::from_gray(136)));
  assert_eq!(heal[2].color,Some(egui::Color32::from_gray(51)));
 }
}
