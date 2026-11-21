//! Windows Save As with native format filters and an inline quality selector.
use crate::backend::Format;
use std::path::{Path, PathBuf};
pub struct Choice {
    pub path: PathBuf,
    pub format: Format,
    pub quality: u8,
    pub preserve_metadata: bool,
}
pub const QUALITY_PRESETS: [u8; 4] = [60, 75, 80, 90];

pub fn parse_quality(text: &str) -> Option<u8> {
    let text = text.trim();
    let number = text.strip_suffix('%').unwrap_or(text).trim().parse().ok()?;
    (1..=100).contains(&number).then_some(number)
}

fn selected_format(index: u32) -> Format {
    match index {
        1 => Format::Jpeg,
        2 => Format::Png,
        4 => Format::Jxl,
        _ => Format::Webp,
    }
}
fn format_index(format: Format) -> u32 {
    match format {
        Format::Jpeg => 1,
        Format::Png => 2,
        Format::Webp => 3,
        Format::Jxl => 4,
    }
}
#[cfg(windows)]
pub fn choose(path: Option<&Path>, format: Format, quality_for: impl Fn(Format) -> u8) -> Result<Option<Choice>, String> {
    use windows::{
        core::{Interface, HSTRING, PCWSTR},
        Win32::{
            Foundation::{ERROR_CANCELLED, HWND},
            System::Com::{
                CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
                CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
            },
            UI::Shell::{
                Common::COMDLG_FILTERSPEC, FileSaveDialog, IFileDialogCustomize, IFileSaveDialog,
                IShellItem, SHCreateItemFromParsingName, FOS_FORCEFILESYSTEM, FOS_OVERWRITEPROMPT,
                FOS_PATHMUSTEXIST, FOS_STRICTFILETYPES, SIGDN_FILESYSPATH,
            },
        },
    };
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    let source_present = path.is_some();
    let run = || -> windows::core::Result<Option<Choice>> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE).ok()?;
            let _apartment = Apartment;
            let dialog: IFileSaveDialog =
                CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?;
            dialog.SetOptions(
                FOS_FORCEFILESYSTEM | FOS_OVERWRITEPROMPT | FOS_PATHMUSTEXIST | FOS_STRICTFILETYPES,
            )?;
            let names = [
                HSTRING::from("JPEG (*.jpg; *.jpeg)"),
                HSTRING::from("PNG (*.png)"),
                HSTRING::from("WebP (*.webp)"),
                HSTRING::from("JPEG XL (*.jxl)"),
            ];
            let patterns = [
                HSTRING::from("*.jpg;*.jpeg"),
                HSTRING::from("*.png"),
                HSTRING::from("*.webp"),
                HSTRING::from("*.jxl"),
            ];
            let filters: Vec<_> = names
                .iter()
                .zip(&patterns)
                .map(|(name, pattern)| COMDLG_FILTERSPEC {
                    pszName: PCWSTR(name.as_ptr()),
                    pszSpec: PCWSTR(pattern.as_ptr()),
                })
                .collect();
            dialog.SetFileTypes(&filters)?;
            dialog.SetFileTypeIndex(format_index(format))?;
            dialog.SetDefaultExtension(&HSTRING::from(format.ext()))?;
            dialog.SetTitle(&HSTRING::from(crate::i18n::t("Salvar imagem")))?;
            let stem = path
                .and_then(Path::file_stem)
                .filter(|s| !s.is_empty())
                .unwrap_or(std::ffi::OsStr::new("LumeView"));
            dialog.SetFileName(&HSTRING::from(format!(
                "{}.{}",
                stem.to_string_lossy(),
                format.ext()
            )))?;
            if let Some(parent) = path.and_then(Path::parent).filter(|p| p.is_dir()) {
                let folder: IShellItem =
                    SHCreateItemFromParsingName(&HSTRING::from(parent.as_os_str()), None)?;
                dialog.SetFolder(&folder)?;
            }
            const CUSTOM: u32 = 1011;
            const LOSSLESS: u32 = 1012;
            let custom: IFileDialogCustomize = dialog.cast()?;
            custom.StartVisualGroup(100, &HSTRING::from(crate::i18n::t("Qualidade")))?;
            custom.AddComboBox(101)?;
            for value in QUALITY_PRESETS.into_iter().rev() {
                custom.AddControlItem(101, u32::from(value), &HSTRING::from(format!("{value}%")))?;
            }
            custom.AddControlItem(101, LOSSLESS, &HSTRING::from(crate::i18n::t("Sem perdas (WebP / JPEG XL)")))?;
            custom.AddControlItem(101, CUSTOM, &HSTRING::from(crate::i18n::t("Personalizada (digite abaixo)")))?;
            let initial_format = if format == Format::Png { Format::Jpeg } else { format };
            let initial_quality = quality_for(initial_format).clamp(1, 100);
            let initial_choice = if QUALITY_PRESETS.contains(&initial_quality) {
                u32::from(initial_quality)
            } else if initial_quality == 100 && matches!(initial_format, Format::Webp | Format::Jxl) {
                LOSSLESS
            } else {
                CUSTOM
            };
            custom.SetSelectedControlItem(101, initial_choice)?;
            custom.AddEditBox(102, &HSTRING::from(initial_quality.to_string()))?;
            custom.AddText(103, &HSTRING::from(crate::i18n::t("PNG preserva a imagem sem perdas; não usa qualidade percentual.")))?;
            custom.EndVisualGroup()?;
            const PRESERVE_METADATA: u32 = 104;
            if path.is_some() {
                custom.AddCheckButton(
                    PRESERVE_METADATA,
                    &HSTRING::from(crate::i18n::t("Preservar metadados")),
                    false,
                )?;
            }
            let owner = HWND(windows_sys::Win32::UI::Input::KeyboardAndMouse::GetActiveWindow());
            if let Err(error) = dialog.Show(owner) {
                if error.code() == windows::core::HRESULT::from_win32(ERROR_CANCELLED.0) {
                    return Ok(None);
                }
                return Err(error);
            }
            let item = dialog.GetResult()?;
            let name = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let value = name.to_string();
            CoTaskMemFree(Some(name.0.cast()));
            let path = PathBuf::from(value?);
            let selected = selected_format(dialog.GetFileTypeIndex()?);
            let actual = match path
                .extension()
                .and_then(|s| s.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref()
            {
                Some("jpg" | "jpeg") => Format::Jpeg,
                Some("png") => Format::Png,
                Some("webp") => Format::Webp,
                Some("jxl") => Format::Jxl,
                _ => selected,
            };
            let invalid = |message: &str| windows::core::Error::new(
                windows::Win32::Foundation::E_INVALIDARG,
                crate::i18n::t(message),
            );
            let quality = if actual == Format::Png {
                100
            } else {
                match custom.GetSelectedControlItem(101)? {
                    CUSTOM => {
                        let raw = custom.GetEditBoxText(102)?;
                        let text = windows::core::PWSTR(raw).to_string();
                        CoTaskMemFree(Some(raw.cast()));
                        parse_quality(&text?).ok_or_else(|| invalid("Digite uma qualidade de 1 a 100."))?
                    }
                    LOSSLESS if actual == Format::Jpeg => return Err(invalid("A opção sem perdas está disponível para WebP e JPEG XL.")),
                    LOSSLESS => 100,
                    value => value as u8,
                }
            };
            let preserve_metadata = source_present
                && custom.GetCheckButtonState(PRESERVE_METADATA)?.as_bool();
            Ok(Some(Choice { path, format: actual, quality, preserve_metadata }))
        }
    };
    run().map_err(|error| error.to_string())
}
#[cfg(not(windows))]
pub fn choose(
    _path: Option<&Path>,
    _format: Format,
    _quality_for: impl Fn(Format) -> u8,
) -> Result<Option<Choice>, String> {
    Ok(None)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_filter_indices_round_trip() {
        for format in [Format::Jpeg, Format::Png, Format::Webp, Format::Jxl] {
            assert_eq!(selected_format(format_index(format)), format);
        }
    }
}
