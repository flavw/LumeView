//! Carry compatible source metadata into an image encoded from edited pixels.
use crate::{backend::{self, Format}, generation};
use exif::{Field, In, Tag, Value};
use image::RgbaImage;
use std::{io::{Cursor, Write}, path::Path};

pub fn save(
    pixels: &RgbaImage,
    source: &Path,
    destination: &Path,
    format: Format,
    quality: u8,
) -> backend::Result<()> {
    let original = std::fs::read(source).map_err(|e| e.to_string())?;
    let text = generation::png_text(&original);
    let exif = build_exif(&original, &text, pixels.dimensions(), format)?;
    if exif.is_empty() && (format != Format::Png || text.is_empty()) {
        return backend::export(pixels, destination, format, quality);
    }

    let parent = destination.parent().unwrap_or(Path::new("."));
    let encoded = tempfile::NamedTempFile::new_in(parent)
        .map_err(|e| e.to_string())?.into_temp_path();
    backend::export(pixels, &encoded, format, quality)?;
    let bytes = std::fs::read(&encoded).map_err(|e| e.to_string())?;
    let output = match format {
        Format::Png => attach_png(&bytes, &text, &exif)?,
        Format::Jpeg => attach_jpeg(&bytes, &exif)?,
        Format::Jxl => crate::jxl_metadata::attach(&bytes, &exif)?,
        Format::Webp => crate::ai_webp::attach_tiff(
            &bytes, &exif, pixels.width(), pixels.height(), pixels.pixels().any(|p| p[3] != 255),
        )?,
    };
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temporary.write_all(&output).map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    temporary.persist(destination).map_err(|e| e.error.to_string())?;
    Ok(())
}

fn build_exif(
    original: &[u8], text: &[(String, String)], (width, height): (u32, u32), format: Format,
) -> backend::Result<Vec<u8>> {
    let mut fields = Vec::new();
    let mut seen = std::collections::HashSet::new();
    if let Some(exif) = generation::exif_from_image(original) {
        for field in exif.fields() {
            // The old thumbnail and opaque manufacturer offsets refer to the original image.
            if field.ifd_num != In::PRIMARY || field.tag == Tag::MakerNote ||
                matches!(field.value, Value::Unknown(..)) || !seen.insert(field.tag) {
                continue;
            }
            let mut field = field.clone();
            field.value = match field.tag {
                Tag::Orientation => Value::Short(vec![1]),
                Tag::ImageWidth | Tag::PixelXDimension => Value::Long(vec![width]),
                Tag::ImageLength | Tag::PixelYDimension => Value::Long(vec![height]),
                _ => field.value,
            };
            fields.push(field);
        }
    }

    if format != Format::Png {
        let mut generation_rows: Vec<_> = text.iter()
            .filter(|(key, _)| matches!(key.as_str(), "parameters" | "prompt" | "workflow"))
            .map(|(key, value)| (format!("Geração por IA · {key}"), value.clone()))
            .collect();
        if !generation_rows.is_empty() {
            if let Some(Field {value: Value::Undefined(bytes, _) | Value::Byte(bytes), ..}) =
                fields.iter().find(|field| field.tag == Tag::UserComment)
            {
                generation_rows.push(("EXIF · UserComment".into(), generation::user_comment(bytes)));
            }
            if let Some(prompt) = crate::ai_webp::prompt(&generation_rows) {
                fields.retain(|field| field.tag != Tag::UserComment);
                fields.push(Field {
                    tag: Tag::UserComment,
                    ifd_num: In::PRIMARY,
                    value: Value::Undefined(crate::ai_webp::unicode_comment(&prompt), 0),
                });
            }
        }
    }

    if fields.is_empty() {
        return Ok(Vec::new());
    }
    let mut writer = exif::experimental::Writer::new();
    for field in &fields {
        writer.push_field(field);
    }
    let mut output = Cursor::new(Vec::new());
    writer.write(&mut output, true).map_err(|e| e.to_string())?;
    Ok(output.into_inner())
}

fn attach_jpeg(data: &[u8], exif: &[u8]) -> backend::Result<Vec<u8>> {
    let rest = data.strip_prefix(b"\xff\xd8").ok_or("Invalid JPEG output")?;
    let size = u16::try_from(exif.len().checked_add(8).ok_or("EXIF is too large")?)
        .map_err(|_| "Os metadados excedem o limite EXIF do JPEG. Escolha outro formato.")?;
    let mut out = b"\xff\xd8\xff\xe1".to_vec();
    out.extend_from_slice(&size.to_be_bytes());
    out.extend_from_slice(b"Exif\0\0");
    out.extend_from_slice(exif);
    out.extend_from_slice(rest);
    Ok(out)
}

fn attach_png(data: &[u8], text: &[(String, String)], exif: &[u8]) -> backend::Result<Vec<u8>> {
    if !data.starts_with(b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR") || data.len() < 33 {
        return Err("Invalid PNG output".into());
    }
    let mut out = data[..33].to_vec();
    if !exif.is_empty() {
        png_chunk(&mut out, b"eXIf", exif)?;
    }
    for (key, value) in text {
        let mut entry = Vec::new();
        for ch in key.chars() {
            entry.push(u8::try_from(ch).map_err(|_| "Invalid PNG metadata keyword")?);
        }
        entry.extend_from_slice(&[0; 5]);
        entry.extend_from_slice(value.as_bytes());
        png_chunk(&mut out, b"iTXt", &entry)?;
    }
    out.extend_from_slice(&data[33..]);
    Ok(out)
}

fn png_chunk(out: &mut Vec<u8>, kind: &[u8; 4], value: &[u8]) -> backend::Result<()> {
    let size = u32::try_from(value.len()).map_err(|_| "PNG metadata is too large")?;
    let mut crc = flate2::Crc::new();
    crc.update(kind);
    crc.update(value);
    out.extend_from_slice(&size.to_be_bytes());
    out.extend_from_slice(kind);
    out.extend_from_slice(value);
    out.extend_from_slice(&crc.sum().to_be_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saving_edited_pixels_preserves_compatible_metadata_and_normalizes_orientation() {
        let directory = tempfile::tempdir().unwrap();
        let source = directory.path().join("source.png");
        let original = RgbaImage::from_pixel(24, 16, image::Rgba([4, 8, 12, 255]));
        original.save(&source).unwrap();
        let fields = [
            Field {tag: Tag::Make, ifd_num: In::PRIMARY, value: Value::Ascii(vec![b"Camera".to_vec()])},
            Field {tag: Tag::Orientation, ifd_num: In::PRIMARY, value: Value::Short(vec![6])},
            Field {tag: Tag::PixelXDimension, ifd_num: In::PRIMARY, value: Value::Long(vec![24])},
        ];
        let mut writer = exif::experimental::Writer::new();
        for field in &fields {
            writer.push_field(field);
        }
        let mut exif = Cursor::new(Vec::new());
        writer.write(&mut exif, true).unwrap();
        let text = vec![("parameters".into(), "Céu azul 🌌".into()), ("Author".into(), "Test author".into())];
        let bytes = attach_png(&std::fs::read(&source).unwrap(), &text, exif.get_ref()).unwrap();
        std::fs::write(&source, &bytes).unwrap();
        let mut edited = RgbaImage::from_pixel(16, 24, image::Rgba([4, 8, 12, 255]));
        for x in 0..16 {
            edited.put_pixel(x, 10, image::Rgba([215, 20, 60, 255]));
        }

        for format in [Format::Png, Format::Jpeg, Format::Webp, Format::Jxl] {
            let path = directory.path().join(format!("edited.{}", format.ext()));
            save(&edited, &source, &path, format, 100).unwrap();
            let decoded = backend::load(&path).unwrap();
            assert_eq!(decoded.pixels.dimensions(), (16, 24));
            if format != Format::Jpeg {
                assert_eq!(&*decoded.pixels, &edited, "{format:?}");
            }
            let data = std::fs::read(&path).unwrap();
            let exif = generation::exif_from_image(&data).unwrap();
            let Value::Ascii(names) = &exif.get_field(Tag::Make, In::PRIMARY).unwrap().value else {
                panic!("Expected camera make as ASCII");
            };
            assert_eq!(names, &[b"Camera".to_vec()]);
            assert_eq!(exif.get_field(Tag::Orientation, In::PRIMARY).unwrap().value.get_uint(0), Some(1));
            assert_eq!(exif.get_field(Tag::PixelXDimension, In::PRIMARY).unwrap().value.get_uint(0), Some(16));
            assert_eq!(crate::ai_webp::read_prompt(&path).unwrap().as_deref(), Some("Céu azul 🌌"));
            if format == Format::Png {
                assert_eq!(generation::png_text(&data), text);
            }
        }
        assert_eq!(std::fs::read(source).unwrap(), bytes);
    }
}
