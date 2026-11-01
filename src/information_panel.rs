//! Image details with full-width, selectable fields for long metadata.
use eframe::egui;
use crate::{i18n::t,icons};

fn value_field(ui: &mut egui::Ui,value: &str) -> egui::text_edit::TextEditOutput {
    let mut read_only=value;
    egui::TextEdit::multiline(&mut read_only)
        .font(egui::TextStyle::Body)
        .desired_width((ui.available_width()-12.0).max(1.0))
        .desired_rows(3)
        .margin(egui::vec2(6.0,6.0))
        .show(ui)
}

pub fn show(ui: &mut egui::Ui,rows: &[(String,String)]) -> Option<String> {
    let mut copied=None;
    for (key,value) in rows {
        let copyable=key.starts_with("Geração por IA") || key=="EXIF · UserComment";
        if copyable || value.contains('\n') || value.len()>160 {
            ui.separator();
            ui.horizontal(|ui| {
                let label=if key=="Geração por IA · parameters" {t("Parâmetros de IA")}else{t(key)};
                ui.strong(format!("{label}:"));
                if copyable && ui.scope(|ui| {
                    ui.spacing_mut().interact_size.y=30.0;
                    icons::tool_button(ui,"Copiar",true)
                }).inner {copied=Some(value.clone());}
            });
            ui.push_id(key,|ui|{value_field(ui,value);});
        } else {
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(egui::vec2(130.0,0.0),egui::Layout::top_down(egui::Align::Min),|ui| {
                    ui.label(t(key));
                });
                ui.add(egui::Label::new(value).wrap().selectable(true));
            });
        }
    }
    copied
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_prompt_wraps_inside_its_field_and_does_not_overlap_the_next_row() {
        let text=format!("{}\n{}","Prompt with many words and image details. ".repeat(80),"unbroken_workflow_token".repeat(30));
        for width in [280.0,760.0] {
            let ctx=egui::Context::default();
            let _=ctx.run(egui::RawInput{screen_rect:Some(egui::Rect::from_min_size(egui::Pos2::ZERO,egui::vec2(width+32.0,900.0))),..Default::default()},|ctx| {
                egui::CentralPanel::default().show(ctx,|ui| {
                    ui.set_width(width);
                    let heading=ui.strong("AI Parameters:").rect;
                    let available=ui.available_width();let output=value_field(ui,&text);
                    assert!(output.response.rect.width()<=available+1.0);
                    assert!(output.galley.rect.width()<=output.response.rect.width());
                    assert!(output.response.rect.top()>=heading.bottom());
                    assert!(output.galley_pos.y>=output.response.rect.top());
                    assert!(output.galley_pos.y+output.galley.rect.height()<=output.response.rect.bottom()+1.0);
                    let next=ui.label("Next metadata field").rect;
                    assert!(next.top()>=output.response.rect.bottom());
                });
            });
        }
    }
}
