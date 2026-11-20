//! Uncompressed EXIF boxes in the JPEG XL container.
const SIGNATURE: &[u8] = b"\0\0\0\x0cJXL \x0d\x0a\x87\x0a";
const FILE_TYPE: &[u8] = b"\0\0\0\x14ftypjxl \0\0\0\0jxl ";
const CODESTREAM: &[u8] = b"\xff\x0a";

struct JxlBox<'a> {
    kind: &'a [u8],
    payload: &'a [u8],
    bytes: &'a [u8],
}

fn take_box<'a>(remaining: &mut &'a [u8]) -> Option<JxlBox<'a>> {
    let data = *remaining;
    let kind = data.get(4..8)?;
    let size = u32::from_be_bytes(data.get(..4)?.try_into().ok()?);
    let (length, header_size) = match size {
        0 => (data.len(), 8),
        1 => {
            let extended = u64::from_be_bytes(data.get(8..16)?.try_into().ok()?);
            (usize::try_from(extended).ok()?, 16)
        }
        size => (size as usize, 8),
    };
    let bytes = data.get(..length)?;
    let payload = bytes.get(header_size..)?;
    *remaining = data.get(length..)?;
    Some(JxlBox { kind, payload, bytes })
}

pub fn exif_tiff(data: &[u8]) -> Option<&[u8]> {
    let mut remaining = data.strip_prefix(SIGNATURE)?;
    while !remaining.is_empty() {
        let entry = take_box(&mut remaining)?;
        if entry.kind == b"Exif" {
            let offset = u32::from_be_bytes(entry.payload.get(..4)?.try_into().ok()?);
            return entry.payload.get(4..)?.get(offset as usize..);
        }
    }
    None
}

pub fn attach(data: &[u8], tiff: &[u8]) -> crate::backend::Result<Vec<u8>> {
    let exif_size = u32::try_from(tiff.len().checked_add(12).ok_or("EXIF is too large")?)
        .map_err(|_| "EXIF is too large")?;
    let write_exif = |output: &mut Vec<u8>| {
        output.extend_from_slice(&exif_size.to_be_bytes());
        output.extend_from_slice(b"Exif\0\0\0\0");
        output.extend_from_slice(tiff);
    };

    let mut output = SIGNATURE.to_vec();
    if data.starts_with(CODESTREAM) {
        output.extend_from_slice(FILE_TYPE);
        write_exif(&mut output);
        // Size zero means that this last box extends to EOF.
        output.extend_from_slice(b"\0\0\0\0jxlc");
        output.extend_from_slice(data);
        return Ok(output);
    }

    let mut remaining = data.strip_prefix(SIGNATURE).ok_or("Invalid JPEG XL signature")?;
    let file_type = take_box(&mut remaining).ok_or("Missing JPEG XL file type")?;
    if file_type.kind != b"ftyp" {
        return Err("Invalid JPEG XL file type".into());
    }
    output.extend_from_slice(file_type.bytes);
    let mut wrote_exif = false;
    while !remaining.is_empty() {
        let entry = take_box(&mut remaining).ok_or("Invalid JPEG XL box")?;
        if !wrote_exif && matches!(entry.kind, b"jxlc" | b"jxlp") {
            write_exif(&mut output);
            wrote_exif = true;
        }
        if entry.kind != b"Exif" && !(entry.kind == b"brob" && entry.payload.starts_with(b"Exif")) {
            output.extend_from_slice(entry.bytes);
        }
    }
    if !wrote_exif {
        return Err("Missing JPEG XL codestream".into());
    }
    Ok(output)
}
