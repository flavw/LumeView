# LumeView

A minimalist image viewer and basic image editor written in Rust, inspired by IrfanView. Browse folders quickly, crop wallpapers, make simple edits, read CBZ comics and inspect AI generation metadata. Supports JPEG, PNG, WebP, AVIF, GIF and more.

![LumeView with its default interface](docs/screenshot.png)

## Download

LumeView is available for **64-bit Windows**. Choose one of these installation options:

- **Microsoft Store:** [Install LumeView from the Microsoft Store](https://apps.microsoft.com/detail/9n0wr4v1nxkf).
- **EXE installer:** Open the [latest GitHub release](https://github.com/flavw/LumeView/releases/latest), download **LumeView-Setup.exe**, and run it. The installer creates a Start menu shortcut and registers LumeView in **Open with**; your default apps are not changed.
- **Manual ZIP:** Open the [latest GitHub release](https://github.com/flavw/LumeView/releases/latest), download **LumeView-0.38.0-Windows-x64.zip**, extract it, and run **LumeView/lumeview.exe**. Keep the `tools` folder beside the executable. No automatic installer or registry changes are performed; file associations are configured manually.

See the [manual installation instructions](docs/manual-installation.md).

## Features

- Fast folder navigation, animated GIF playback and support for common image formats, SVG and CBZ comics.
- Fullscreen viewing, slideshows, seven zoom modes and cursor-centered zoom.
- Wallpaper cropping with preset or custom aspect ratios, plus lossless JPEG cropping and rotation.
- Basic editing in Paint dialog: brush, fill, clone stamp, spot healing, Remove, shapes, arrows and floating text, with undo/redo.
- Image effects and adjustments, including black and white, sepia, brightness, contrast and saturation.
- Batch resizing, conversion and renaming; horizontal or vertical panorama creation.
- AI generation metadata inspection, clipboard images and JPEG/PNG/WebP export.
- Customizable shortcuts, dark/light/neon themes, and English, Portuguese, Spanish and French interfaces.

## Quick controls

| Key | Action |
| --- | --- |
| Left / Right | Previous / next image |
| Ctrl+O | Open an image |
| Enter / F / left double-click | Toggle fullscreen |
| 1–7 | Choose a zoom mode; retained while navigating |
| Ctrl or Alt + mouse wheel | Zoom |
| I | Image information and AI metadata |
| P | Preferences |
| C | Toggle centered / top-left zoom alignment |
| F2 | Rename |
| R | Rotate the view 90° clockwise |
| F5 | Reload image and folder |
| Ctrl+S | Save dialog |
| Ctrl+P | Windows Print Pictures |
| E / W / Q | Export JPEG / WebP / PNG |
| Page Down / Page Up | Scroll down / up; press again at the boundary to change image |
| Ctrl+Y | Apply the crop |
| Delete / Shift+Delete | Recycle / permanently delete, with confirmation |
| F1 | All shortcuts |
| F12 | Show / hide Paint dialog |

### Paint dialog controls

Tool shortcuts take priority over viewer shortcuts while Paint dialog is active. Typing in text fields keeps normal text input.

| Key | Tool or action |
| --- | --- |
| B | Brush |
| H | Hand |
| M / V | Select and move |
| C | Crop, with preset or custom aspect ratio and a swap button |
| I | Eyedropper; click its HEX code to copy it |
| J | Last-used spot healing / Remove tool |
| Shift+J | Cycle spot healing / Remove |
| S | Clone stamp; Alt+click sets the source |
| G | Paint bucket |
| T | Floating text |
| U / Shift+U | Last-used shape / cycle rectangle, ellipse, line and arrow |
| Alt+right-drag | Adjust brush, clone stamp, spot healing or Remove size; vertical dragging changes hardness for Brush only |
| Ctrl+Z / Ctrl+J | Undo / redo |
| Ctrl+Enter / Esc | Apply / cancel floating text |

Hold Shift for horizontal/vertical brush strokes, 45° steps with line/arrow tools, and square/circular shapes. Mouse-wheel image navigation is disabled while Paint dialog is active.

Without modifiers, the mouse wheel navigates images or scrolls vertically in the width-fit reading mode. The default zoom fits large images to the window without enlarging small ones. Selected zoom modes persist between images; width-fit pages start at the top. Left-click and drag creates a freeform selection in zoom modes 2 and 3 outside fullscreen. Automatic restart at the end of a folder is enabled by default; disabling it shows a restart dialog.

## Build from source

Install Rust and your platform's native build tools, then run:

```sh
cargo build --release --bin lumeview
```

The Windows build also needs a resource compiler (`windres` for the GNU toolchain). The shipped Windows release is tested; Linux support is present in the source but has not yet been validated as a release.

JPEG/PNG/WebP viewing uses native libraries. Additional formats such as AVIF and batch conversion use ImageMagick on demand; lossless JPEG operations use jpegtran. These tools are bundled with the Windows installer. For a source build, install them on PATH, place them under `tools/`, or set `LUMEVIEW_MAGICK` and `LUMEVIEW_JPEGTRAN` to their executable paths. They are not required for native-format navigation.

```sh
cargo test --bin lumeview
cargo clippy --bin lumeview -- -D warnings
```

## Build the Windows EXE installer

After building the release executable, run `powershell -File installer/build.ps1`. Place the ImageMagick and jpegtran distributions under `tools/` first. The script creates `LumeView-Setup.exe`; `/S` enables silent installation. See [installer exit codes](installer/STORE-INSTALLER.md).

## License and AI assistance

LumeView's own source code and assets are dedicated to the public domain under **CC0 1.0 Universal**. See [LICENSE](LICENSE). Third-party libraries and bundled tools retain their respective licenses; see [THIRD_PARTY_LICENSES.txt](THIRD_PARTY_LICENSES.txt) and their accompanying notices.

This project was programmed with AI assistance. AI tools were also used to create the cat icon. LumeView is an independent project and is not affiliated with IrfanView.
