use eframe::egui;
/// Windows common color dialog; other platforms retain egui's color picker.
pub fn color_button(ui: &mut egui::Ui, label: &str, color: &mut [u8; 3]) {
    color_control(ui, label, color, false);
}
/// Compact swatch with a separate label, retaining the common color dialog.
pub fn color_swatch(ui: &mut egui::Ui, label: &str, color: &mut [u8; 3]) {
    color_control(ui, label, color, true);
}
fn color_control(ui: &mut egui::Ui, label: &str, color: &mut [u8; 3], swatch: bool) {
    ui.horizontal(|ui| {
        let preview = egui::Color32::from_rgb(color[0], color[1], color[2]);
        let text = if color.iter().map(|v| *v as u16).sum::<u16>() > 384 {
            egui::Color32::BLACK
        } else {
            egui::Color32::WHITE
        };
        #[cfg(windows)]
        let response = if swatch {
            let accessible_label=if label.is_empty(){crate::i18n::t("Cor")}else{label.to_owned()};
            let response = ui.add_sized([22.0,22.0],egui::Button::new("").fill(preview)
                .stroke(egui::Stroke::new(1.0_f32,ui.visuals().widgets.noninteractive.fg_stroke.color)));
            response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button,ui.is_enabled(),&accessible_label));
            if !label.is_empty(){ui.label(label);}
            response.on_hover_text(accessible_label)
        } else {
            ui.add(egui::Button::new(egui::RichText::new(label).color(text)).fill(preview))
        };
        #[cfg(windows)]
        if response.clicked() {
            use windows_sys::Win32::UI::{
                Controls::Dialogs::{ChooseColorW, CC_FULLOPEN, CC_RGBINIT, CHOOSECOLORW},
                Input::KeyboardAndMouse::GetActiveWindow,
            };
            let mut custom = [0u32; 16];
            let mut dialog: CHOOSECOLORW = unsafe { std::mem::zeroed() };
            dialog.lStructSize = std::mem::size_of::<CHOOSECOLORW>() as u32;
            dialog.hwndOwner = unsafe { GetActiveWindow() };
            dialog.rgbResult = color[0] as u32 | (color[1] as u32) << 8 | (color[2] as u32) << 16;
            dialog.lpCustColors = custom.as_mut_ptr();
            dialog.Flags = CC_FULLOPEN | CC_RGBINIT;
            if unsafe { ChooseColorW(&mut dialog) } != 0 {
                *color = [
                    dialog.rgbResult as u8,
                    (dialog.rgbResult >> 8) as u8,
                    (dialog.rgbResult >> 16) as u8,
                ];
            }
        }
        #[cfg(not(windows))]
        {
            ui.color_edit_button_srgb(color);
            if !label.is_empty(){ui.label(label);}
        }
    });
}
