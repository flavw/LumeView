//! Windows Save As with native format filters and an inline quality selector.
use crate::backend::Format;
use std::path::{Path, PathBuf};
pub struct Choice {
    pub path: PathBuf,
    pub format: Format,
    pub quality: u8,
}
fn selected_format(index: u32) -> Format {
    match index {
        1 => Format::Jpeg,
        2 => Format::Png,
        _ => Format::Webp,
    }
}
fn format_index(format: Format) -> u32 {
    match format {
        Format::Jpeg => 1,
        Format::Png => 2,
        Format::Webp => 3,
    }
}
#[cfg(windows)]
pub fn choose(path: Option<&Path>, format: Format, quality: u8) -> Result<Option<Choice>, String> {
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
            ];
            let patterns = [
                HSTRING::from("*.jpg;*.jpeg"),
                HSTRING::from("*.png"),
                HSTRING::from("*.webp"),
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
            let custom: IFileDialogCustomize = dialog.cast()?;
            custom.AddText(100, &HSTRING::from(crate::i18n::t("Qualidade JPEG / WebP")))?;
            custom.AddComboBox(101)?;
            for value in (1..=100u32).rev() {
                custom.AddControlItem(101, value, &HSTRING::from(format!("{value}%")))?;
            }
            custom.SetSelectedControlItem(101, u32::from(quality.clamp(1, 100)))?;
            custom.AddText(
                102,
                &HSTRING::from(crate::i18n::t(
                    "PNG preserva a imagem sem perdas; não usa qualidade percentual.",
                )),
            )?;
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
                _ => selected,
            };
            Ok(Some(Choice {
                path,
                format: actual,
                quality: custom.GetSelectedControlItem(101)? as u8,
            }))
        }
    };
    run().map_err(|error| error.to_string())
}
#[cfg(not(windows))]
pub fn choose(
    _path: Option<&Path>,
    _format: Format,
    _quality: u8,
) -> Result<Option<Choice>, String> {
    Ok(None)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_filter_indices_round_trip() {
        for format in [Format::Jpeg, Format::Png, Format::Webp] {
            assert_eq!(selected_format(format_index(format)), format);
        }
    }
}
