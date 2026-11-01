//! WebP export: PNG generation text -> Unicode EXIF UserComment.
use std::{io::Write, path::Path};
use crate::{backend::{self, Format}};

pub fn prompt(rows: &[(String,String)]) -> Option<String> {
    let fields: Vec<_> = rows.iter().filter(|(key,value)|
        (key.starts_with("Geração por IA · ") || key=="EXIF · UserComment") && !value.trim().is_empty()
    ).collect();
    match fields.as_slice() {
        [] => None,
        [(_,value)] => Some(value.clone()),
        _ => Some(fields.iter().map(|(key,value)|format!("{}:\n{value}",key.strip_prefix("Geração por IA · ").unwrap_or(key))).collect::<Vec<_>>().join("\n\n")),
    }
}

/// Read only generation metadata; no pixel decode or external process.
pub fn read_prompt(path: &Path) -> backend::Result<Option<String>> {
    let bytes=std::fs::read(path).map_err(|e|e.to_string())?;
    let mut rows: Vec<_>=crate::generation::png_text(&bytes).into_iter()
        .filter(|(key,_)|matches!(key.as_str(),"parameters"|"prompt"|"workflow"))
        .map(|(key,value)|(format!("Geração por IA · {key}"),value)).collect();
    if let Some(exif)=crate::generation::exif_from_image(&bytes) {
        if let Some(field)=exif.get_field(exif::Tag::UserComment,exif::In::PRIMARY) {
            if let exif::Value::Undefined(bytes,_) | exif::Value::Byte(bytes)=&field.value {
                rows.push(("EXIF · UserComment".into(),crate::generation::user_comment(bytes)));
            }
        }
    }
    Ok(prompt(&rows))
}

fn exif(text: &str) -> Vec<u8> {
    let mut comment=b"UNICODE\0\xfe\xff".to_vec();
    for unit in text.encode_utf16().chain(std::iter::once(0)) {comment.extend(unit.to_be_bytes());}
    let mut data=b"II\x2a\0\x08\0\0\0".to_vec();
    data.extend(1u16.to_le_bytes());
    data.extend(0x8769u16.to_le_bytes());data.extend(4u16.to_le_bytes());
    data.extend(1u32.to_le_bytes());data.extend(26u32.to_le_bytes());data.extend(0u32.to_le_bytes());
    data.extend(1u16.to_le_bytes());
    data.extend(0x9286u16.to_le_bytes());data.extend(7u16.to_le_bytes());
    data.extend((comment.len() as u32).to_le_bytes());data.extend(44u32.to_le_bytes());data.extend(0u32.to_le_bytes());
    data.extend(comment);data
}

fn chunk(out: &mut Vec<u8>,kind: &[u8;4],data: &[u8]) {
    out.extend(kind);out.extend((data.len() as u32).to_le_bytes());out.extend(data);
    if data.len()%2==1 {out.push(0);}
}

fn attach(data: &[u8],text: &str,width: u32,height: u32,alpha: bool) -> backend::Result<Vec<u8>> {
    if data.get(..4)!=Some(b"RIFF") || data.get(8..12)!=Some(b"WEBP") || width==0 || height==0 || width>0x1000000 || height>0x1000000 {
        return Err("Invalid WebP container or dimensions".into());
    }
    if text.len()>16*1024*1024 {return Err("AI metadata exceeds the 16 MiB limit".into());}
    let mut chunks=Vec::new();let mut at=12;
    while at<data.len() {
        let header=data.get(at..at+8).ok_or("Truncated WebP chunk")?;
        let size=u32::from_le_bytes(header[4..8].try_into().unwrap()) as usize;
        let end=at.checked_add(8+size).ok_or("Invalid WebP size")?;
        let body=data.get(at+8..end).ok_or("Truncated WebP data")?;
        chunks.push((&header[..4],body));at=end+size%2;
    }
    let mut out=b"RIFF\0\0\0\0WEBP".to_vec();
    if !chunks.iter().any(|(kind,_)|*kind==b"VP8X") {
        let mut extended=vec![8 | if alpha {16}else{0},0,0,0];
        extended.extend(&(width-1).to_le_bytes()[..3]);extended.extend(&(height-1).to_le_bytes()[..3]);
        chunk(&mut out,b"VP8X",&extended);
    }
    for (kind,body) in chunks {
        if kind==b"EXIF" {continue;}
        if kind==b"VP8X" {
            if body.len()!=10 {return Err("Invalid VP8X header".into());}
            let mut extended=body.to_vec();extended[0]|=8;chunk(&mut out,b"VP8X",&extended);
        } else {chunk(&mut out,kind.try_into().unwrap(),body);}
    }
    chunk(&mut out,b"EXIF",&exif(text));
    let size=u32::try_from(out.len()-8).map_err(|_|"WebP too large")?;out[4..8].copy_from_slice(&size.to_le_bytes());
    Ok(out)
}

pub fn export(pixels: &image::RgbaImage,dest: &Path,text: &str,quality: u8,overwrite: bool) -> backend::Result<()> {
    // Both encoding and metadata assembly finish before replacing the destination.
    let parent=dest.parent().unwrap_or(Path::new("."));
    let encoded=tempfile::Builder::new().suffix(".webp").tempfile_in(parent).map_err(|e|e.to_string())?.into_temp_path();
    backend::export(pixels,&encoded,Format::Webp,quality)?;
    let bytes=std::fs::read(&encoded).map_err(|e|e.to_string())?;
    let bytes=attach(&bytes,text,pixels.width(),pixels.height(),pixels.pixels().any(|p|p[3]!=255))?;
    let mut output=tempfile::NamedTempFile::new_in(parent).map_err(|e|e.to_string())?;
    output.write_all(&bytes).map_err(|e|e.to_string())?;output.as_file().sync_all().map_err(|e|e.to_string())?;
    if overwrite {output.persist(dest).map_err(|e|e.to_string())?;}
    else {output.persist_noclobber(dest).map_err(|e|e.to_string())?;}
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn webp_round_trip_preserves_unicode_long_text_pixels_and_alpha() {
        for alpha in [255,100] {
            let pixels=image::RgbaImage::from_pixel(17,11,image::Rgba([30,100,220,alpha]));
            let mut encoded=std::io::Cursor::new(Vec::new());
            pixels.write_to(&mut encoded,image::ImageFormat::WebP).unwrap();
            let text=format!("ação, café, 猫 🐱\n{}", "workflow text ".repeat(10000));
            let result=attach(encoded.get_ref(),&text,17,11,alpha!=255).unwrap();
            let metadata=crate::generation::exif_from_image(&result).unwrap();
            let field=metadata.get_field(exif::Tag::UserComment,exif::In::PRIMARY).unwrap();
            let exif::Value::Undefined(comment,_)=&field.value else {panic!("Expected UserComment")};
            assert_eq!(crate::generation::user_comment(comment),text);
            assert_eq!(image::load_from_memory(&result).unwrap().to_rgba8(),pixels);
        }
    }
    #[test]
    fn gathers_all_generation_fields_and_excludes_other_metadata() {
        let rows=vec![("Arquivo".into(),"test.png".into()),("Geração por IA · prompt".into(),"positive".into()),("Geração por IA · workflow".into(),"{workflow}".into())];
        assert_eq!(prompt(&rows).unwrap(),"prompt:\npositive\n\nworkflow:\n{workflow}");
        assert!(prompt(&rows[..1]).is_none());
        assert_eq!(prompt(&[("EXIF · UserComment".into(),"raw prompt".into())]).unwrap(),"raw prompt");
    }
    #[test]
    fn export_encodes_webp_and_keeps_existing_destination_on_failure() {
        let dir=tempfile::tempdir().unwrap();let path=dir.path().join("test.webp");
        let pixels=image::RgbaImage::from_pixel(32,32,image::Rgba([50,120,200,255]));
        export(&pixels,&path,"Prompt: ação 🐱",80,false).unwrap();
        let bytes=std::fs::read(&path).unwrap();assert!(bytes.windows(4).any(|w|w==b"VP8 "));
        assert_eq!(image::load_from_memory(&bytes).unwrap().width(),32);
        assert!(crate::generation::exif_from_image(&bytes).is_some());
        assert!(export(&pixels,&path,&"x".repeat(16*1024*1024+1),80,true).is_err());
        assert_eq!(std::fs::read(&path).unwrap(),bytes);
        assert!(export(&pixels,&path,"replacement",80,false).is_err());
        assert_eq!(std::fs::read(&path).unwrap(),bytes);
        assert_eq!(read_prompt(&path).unwrap().unwrap(),"Prompt: ação 🐱");
    }
}
