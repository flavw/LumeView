//! Incremental swept-disk mask; opacity is applied once when drawing the preview.
use eframe::egui::{self, Color32, Pos2, Rect, Vec2};
use image::RgbaImage;
use std::collections::{HashMap, HashSet};

const TILE: u32 = 128;
const OPACITY: u8 = 112;
#[derive(Default)]
pub struct Preview {
    tiles: HashMap<(u32,u32),Tile>,
    processed: usize,
}
struct Tile {
    pixels: RgbaImage,
    texture: Option<egui::TextureHandle>,
}
impl Preview {
    pub fn clear(&mut self) { self.tiles.clear(); self.processed = 0; }
    fn update_mask(&mut self, path: &[[f32;2]], diameter: f32, dimensions: (u32,u32)) -> HashSet<(u32,u32)> {
        let mut dirty = HashSet::new();
        if path.len() < self.processed { self.clear(); }
        let (w,h) = dimensions;
        if w == 0 || h == 0 { return dirty; }
        for index in self.processed..path.len() {
            let a = path[index.saturating_sub(1)];
            let b = path[index];
            let r = diameter.max(1.0)*0.5 + 1.0;
            let top = (a[1].min(b[1])-r).floor().max(0.0) as u32;
            let bottom = ((a[1].max(b[1])+r).ceil().max(0.0) as u32).min(h);
            if top >= bottom { continue; }
            let dx = b[0]-a[0]; let dy = b[1]-a[1];
            for ty in top/TILE..=(bottom-1)/TILE {
                // Only visit tiles intersecting the swept disk's corridor.
                let (left,right) = if dy.abs() > 0.001 {
                    let t0 = ((ty as f32*TILE as f32-r-a[1])/dy).clamp(0.0,1.0);
                    let t1 = ((((ty+1)*TILE) as f32+r-a[1])/dy).clamp(0.0,1.0);
                    let x0 = a[0]+dx*t0; let x1 = a[0]+dx*t1;
                    (x0.min(x1)-r,x0.max(x1)+r)
                } else { (a[0].min(b[0])-r,a[0].max(b[0])+r) };
                let left = left.floor().max(0.0) as u32;
                let right = (right.ceil().max(0.0) as u32).min(w);
                if left >= right { continue; }
                for tx in left/TILE..=(right-1)/TILE {
                    let origin = [(tx*TILE) as f32,(ty*TILE) as f32];
                    let tile = self.tiles.entry((tx,ty)).or_insert_with(||Tile {
                        pixels: RgbaImage::new(TILE.min(w-tx*TILE),TILE.min(h-ty*TILE)),texture:None,
                    });
                    crate::paint::brush(&mut tile.pixels,[a[0]-origin[0],a[1]-origin[1]],
                        [b[0]-origin[0],b[1]-origin[1]],diameter,1.0,[0;3]);
                    dirty.insert((tx,ty));
                }
            }
        }
        self.processed = path.len();
        dirty
    }
    pub fn paint(&mut self, ctx: &egui::Context, painter: &egui::Painter, path: &[[f32;2]],
        diameter: f32, dimensions: (u32,u32), source: Rect, shown: Rect, scale: Vec2,
        rotation: crate::view_rotation::Rotation) {
        for key in self.update_mask(path,diameter,dimensions) {
            let tile = self.tiles.get_mut(&key).unwrap();
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [tile.pixels.width() as usize,tile.pixels.height() as usize],tile.pixels.as_raw());
            if let Some(texture) = &mut tile.texture { texture.set(image,egui::TextureOptions::LINEAR); }
            else { tile.texture = Some(ctx.load_texture("heal-preview",image,egui::TextureOptions::LINEAR)); }
        }
        let painter = painter.with_clip_rect(painter.clip_rect().intersect(shown));
        for (&(tx,ty),tile) in &self.tiles {
            let Some(texture) = &tile.texture else { continue; };
            let dest = Rect::from_min_size(source.min+Vec2::new((tx*TILE) as f32*scale.x,(ty*TILE) as f32*scale.y),
                Vec2::new(tile.pixels.width() as f32*scale.x,tile.pixels.height() as f32*scale.y));
            if !rotation.rect(dest,shown.center()).intersects(painter.clip_rect()) { continue; }
            let mut mesh = rotation.mesh(texture.id(),dest,Rect::from_min_max(Pos2::ZERO,Pos2::new(1.0,1.0)),shown.center());
            for vertex in &mut mesh.vertices { vertex.color = Color32::from_white_alpha(OPACITY); }
            painter.add(egui::Shape::mesh(mesh));
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rounded_preview_matches_the_repair_mask_across_tile_boundaries() {
        let path = [[110.5,120.5],[145.5,125.5],[156.5,170.5],[140.5,160.5]];
        let mut preview = Preview::default();
        preview.update_mask(&path[..1],20.0,(300,240));
        preview.update_mask(&path,20.0,(300,240));
        let mut reference = RgbaImage::new(300,240);
        let mut previous = path[0];
        for point in path {
            crate::paint::brush(&mut reference,previous,point,20.0,1.0,[0;3]);previous=point;
        }
        for (x,y,pixel) in reference.enumerate_pixels() {
            let actual = preview.tiles.get(&(x/TILE,y/TILE)).map(|tile|tile.pixels.get_pixel(x%TILE,y%TILE)[3]).unwrap_or(0);
            assert_eq!(actual,pixel[3],"mask at {x},{y}");
        }
        assert!(preview.update_mask(&path,20.0,(300,240)).is_empty());
        assert!(preview.tiles.len()<6);
        preview.clear(); assert!(preview.tiles.is_empty()); assert_eq!(preview.processed,0);
    }
    #[test]
    fn click_is_a_round_spot_and_overlaps_keep_one_opacity() {
        let mut preview = Preview::default();
        preview.update_mask(&[[30.5,30.5];3],20.0,(100,100));
        let tile = &preview.tiles[&(0,0)].pixels;
        assert_eq!(tile.get_pixel(30,30)[3],255);
        assert_eq!(tile.get_pixel(39,39)[3],0);
        assert!(tile.get_pixel(40,30)[3]>0);
        assert!(OPACITY>0 && OPACITY<255);
    }
}
