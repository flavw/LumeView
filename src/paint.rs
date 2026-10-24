//! CPU editing kernels. Work is restricted to the affected pixels; no viewer-time workers.
use image::{Rgba, RgbaImage};
#[derive(Clone, Copy, PartialEq, Default)]
pub enum Tool {
    #[default]
    Brush,
    Bucket,
    Heal,
    Remove,
    Rectangle,
    Ellipse,
    Eyedropper,
    Arrow,
    Clone,
    Line,
    Select,
    Crop,
    Text,
    Hand,
}
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}
impl Bounds {
    pub fn around(a: [f32; 2], b: [f32; 2], r: f32, im: &RgbaImage) -> Self {
        let x = (a[0].min(b[0]) - r).floor().max(0.0) as u32;
        let y = (a[1].min(b[1]) - r).floor().max(0.0) as u32;
        let right = ((a[0].max(b[0]) + r).ceil().max(0.0) as u32).min(im.width());
        let bottom = ((a[1].max(b[1]) + r).ceil().max(0.0) as u32).min(im.height());
        Self {
            x: x.min(im.width()),
            y: y.min(im.height()),
            w: right.saturating_sub(x),
            h: bottom.saturating_sub(y),
        }
    }
}
fn blend(p: &mut Rgba<u8>, color: [u8; 3], coverage: f32) {
    let a = coverage.clamp(0.0, 1.0);
    let old = p[3] as f32 / 255.0;
    let out = a + old * (1.0 - a);
    if out > 0.0 {
        for c in 0..3 {
            p[c] = ((color[c] as f32 * a + p[c] as f32 * old * (1.0 - a)) / out).round() as u8;
        }
        p[3] = (out * 255.0).round() as u8;
    }
}
/// Continuous swept disk avoids gaps even for fast pointer movement.
pub fn brush(
    im: &mut RgbaImage,
    a: [f32; 2],
    b: [f32; 2],
    diameter: f32,
    hardness: f32,
    color: [u8; 3],
) -> Bounds {
    let r = diameter.max(1.0) * 0.5;
    let bounds = Bounds::around(a, b, r + 1.0, im);
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let length = dx * dx + dy * dy;
    let inner = r * hardness.clamp(0.0, 1.0);
    for y in bounds.y..bounds.y + bounds.h {
        let py = y as f32 + 0.5;
        let (left, right) = if dy.abs() > 0.001 {
            let t0 = ((py - r - 1.0 - a[1]) / dy).clamp(0.0, 1.0);
            let t1 = ((py + r + 1.0 - a[1]) / dy).clamp(0.0, 1.0);
            let x0 = a[0] + dx * t0;
            let x1 = a[0] + dx * t1;
            (
                ((x0.min(x1) - r - 1.0).floor().max(bounds.x as f32) as u32),
                ((x0.max(x1) + r + 1.0)
                    .ceil()
                    .min((bounds.x + bounds.w) as f32) as u32),
            )
        } else {
            (bounds.x, bounds.x + bounds.w)
        };
        for x in left..right {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let t = if length > 0.0 {
                ((px - a[0]) * dx + (py - a[1]) * dy) / length
            } else {
                0.0
            }
            .clamp(0.0, 1.0);
            let d = ((px - a[0] - t * dx).powi(2) + (py - a[1] - t * dy).powi(2)).sqrt();
            let coverage = if d <= inner {
                1.0
            } else {
                ((r + 0.5 - d) / (r - inner + 0.5)).clamp(0.0, 1.0)
            };
            if coverage > 0.0 {
                blend(im.get_pixel_mut(x, y), color, coverage);
            }
        }
    }
    bounds
}
/// Scanline flood fill: memory follows spans rather than one queue item per pixel.
pub fn fill(im: &mut RgbaImage, point: [f32; 2], color: [u8; 3], tolerance: u8) {
    let x = point[0].floor() as u32;
    let y = point[1].floor() as u32;
    if x >= im.width() || y >= im.height() {
        return;
    }
    let target = *im.get_pixel(x, y);
    let replacement = Rgba([color[0], color[1], color[2], 255]);
    if target == replacement {
        return;
    }
    let mut seen = vec![false; im.width() as usize * im.height() as usize];
    let mut stack = vec![(x, y)];
    let w = im.width();
    let h = im.height();
    let matches = |p: &Rgba<u8>| (0..4).all(|c| p[c].abs_diff(target[c]) <= tolerance);
    while let Some((x, y)) = stack.pop() {
        if seen[(y * w + x) as usize] || !matches(im.get_pixel(x, y)) {
            continue;
        }
        let mut left = x;
        while left > 0 && !seen[(y * w + left - 1) as usize] && matches(im.get_pixel(left - 1, y)) {
            left -= 1;
        }
        let mut right = x;
        while right + 1 < w
            && !seen[(y * w + right + 1) as usize]
            && matches(im.get_pixel(right + 1, y))
        {
            right += 1;
        }
        for xx in left..=right {
            seen[(y * w + xx) as usize] = true;
            im.put_pixel(xx, y, replacement);
        }
        for yy in [y.checked_sub(1), if y + 1 < h { Some(y + 1) } else { None }]
            .into_iter()
            .flatten()
        {
            let mut span = false;
            for xx in left..=right {
                let eligible = !seen[(yy * w + xx) as usize] && matches(im.get_pixel(xx, yy));
                if eligible && !span {
                    stack.push((xx, yy));
                }
                span = eligible;
            }
        }
    }
}
pub fn shape(
    im: &mut RgbaImage,
    a: [f32; 2],
    b: [f32; 2],
    width: f32,
    color: [u8; 3],
    ellipse: bool,
    fill: Option<[u8; 3]>,
) {
    let min = [a[0].min(b[0]), a[1].min(b[1])];
    let max = [a[0].max(b[0]), a[1].max(b[1])];
    let center = [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5];
    let radii = [
        ((max[0] - min[0]) * 0.5).max(0.5),
        ((max[1] - min[1]) * 0.5).max(0.5),
    ];
    let bounds = Bounds::around(min, max, width * 0.5 + 1.0, im);
    for y in bounds.y..bounds.y + bounds.h {
        for x in bounds.x..bounds.x + bounds.w {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let distance = if ellipse {
                let nx = (px - center[0]) / radii[0];
                let ny = (py - center[1]) / radii[1];
                let q = (nx * nx + ny * ny).sqrt();
                (q - 1.0) * radii[0].min(radii[1])
            } else {
                let qx = (px - center[0]).abs() - radii[0];
                let qy = (py - center[1]).abs() - radii[1];
                qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0)
            };
            if let Some(fill) = fill {
                let coverage = (0.5 - distance).clamp(0.0, 1.0);
                if coverage > 0.0 { blend(im.get_pixel_mut(x, y), fill, coverage); }
            }
            let coverage = (width * 0.5 + 0.5 - distance.abs()).clamp(0.0, 1.0);
            if coverage > 0.0 {
                blend(im.get_pixel_mut(x, y), color, coverage);
            }
        }
    }
}
/// Local boundary propagation followed by diffusion. Runs off the UI thread.
/// Intended for small blemishes on smooth surfaces, not large object removal.
pub fn heal(im: &mut RgbaImage, path: &[[f32; 2]], diameter: f32) -> Result<(), String> {
    if path.is_empty() {
        return Ok(());
    }
    let mut a = path[0];
    let mut b = a;
    for p in path {
        a = [a[0].min(p[0]), a[1].min(p[1])];
        b = [b[0].max(p[0]), b[1].max(p[1])];
    }
    let bounds = Bounds::around(a, b, diameter * 0.5 + 2.0, im);
    if u64::from(bounds.w) * u64::from(bounds.h) > 2_000_000 {
        return Err("Use a correção localizada em uma área menor.".into());
    }
    let mut mask = RgbaImage::new(bounds.w, bounds.h);
    let local: Vec<_> = path
        .iter()
        .map(|p| [p[0] - bounds.x as f32, p[1] - bounds.y as f32])
        .collect();
    let mut previous = local[0];
    for p in &local {
        brush(&mut mask, previous, *p, diameter, 1.0, [255; 3]);
        previous = *p;
    }
    let w = bounds.w as usize;
    let h = bounds.h as usize;
    let mut pixels =
        image::imageops::crop_imm(im, bounds.x, bounds.y, bounds.w, bounds.h).to_image();
    let mut known: Vec<bool> = mask.pixels().map(|p| p[3] == 0).collect();
    let mut queue = std::collections::VecDeque::new();
    for y in 0..h {
        for x in 0..w {
            if known[y * w + x] {
                queue.push_back((x, y));
            }
        }
    }
    while let Some((x, y)) = queue.pop_front() {
        for (xx, yy) in [
            (x.wrapping_sub(1), y),
            (x + 1, y),
            (x, y.wrapping_sub(1)),
            (x, y + 1),
        ] {
            if xx < w && yy < h && !known[yy * w + xx] {
                let p = *pixels.get_pixel(x as u32, y as u32);
                pixels.put_pixel(xx as u32, yy as u32, p);
                known[yy * w + xx] = true;
                queue.push_back((xx, yy));
            }
        }
    }
    for _ in 0..24 {
        for y in 0..h {
            for x in 0..w {
                if mask.get_pixel(x as u32, y as u32)[3] == 0 {
                    continue;
                }
                let mut sum = [0u32; 4];
                let mut count = 0;
                for (xx, yy) in [
                    (x.wrapping_sub(1), y),
                    (x + 1, y),
                    (x, y.wrapping_sub(1)),
                    (x, y + 1),
                ] {
                    if xx < w && yy < h {
                        let p = pixels.get_pixel(xx as u32, yy as u32);
                        for c in 0..4 {
                            sum[c] += p[c] as u32;
                        }
                        count += 1;
                    }
                }
                if count > 0 {
                    pixels.put_pixel(x as u32, y as u32, Rgba(sum.map(|s| (s / count) as u8)));
                }
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            if mask.get_pixel(x as u32, y as u32)[3] > 0 {
                let alpha = u32::from(mask.get_pixel(x as u32, y as u32)[3]);
                let original = im.get_pixel(bounds.x + x as u32, bounds.y + y as u32);
                let healed = pixels.get_pixel(x as u32, y as u32);
                let blended = Rgba(std::array::from_fn(|c| ((u32::from(original[c])*(255-alpha) + u32::from(healed[c])*alpha + 127)/255) as u8));
                im.put_pixel(bounds.x + x as u32, bounds.y + y as u32, blended);
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn swept_brush_has_real_diameter_and_no_gaps() {
        let mut im = RgbaImage::new(100, 40);
        brush(&mut im, [10.0, 20.0], [90.0, 20.0], 10.0, 1.0, [255, 0, 0]);
        for x in 10..90 {
            assert_eq!(im.get_pixel(x, 20)[3], 255);
        }
        assert_eq!(im.get_pixel(50, 13)[3], 0);
        assert!(im.get_pixel(50, 15)[3] > 0);
    }
    #[test]
    fn softness_and_alpha_are_preserved() {
        let mut im = RgbaImage::new(40, 40);
        brush(&mut im, [20.0, 20.0], [20.0, 20.0], 20.0, 0.0, [255, 0, 0]);
        assert!(im.get_pixel(20, 20)[3] > im.get_pixel(27, 20)[3]);
        assert_eq!(im.get_pixel(0, 0)[3], 0);
    }
    #[test]
    fn bucket_respects_boundaries_and_same_color_tolerance() {
        let mut im = RgbaImage::from_pixel(20, 20, Rgba([0, 0, 0, 255]));
        for y in 0..20 {
            im.put_pixel(10, y, Rgba([255; 4]));
        }
        fill(&mut im, [2.0, 2.0], [100, 0, 0], 0);
        assert_eq!(im.get_pixel(9, 19)[0], 100);
        assert_eq!(im.get_pixel(11, 19)[0], 0);
        fill(&mut im, [2.0, 2.0], [100, 0, 0], 255);
    }
    #[test]
    fn healing_uses_surroundings_and_preserves_outside() {
        let mut im = RgbaImage::from_pixel(40, 40, Rgba([200, 100, 50, 255]));
        im.put_pixel(20, 20, Rgba([0, 0, 0, 255]));
        heal(&mut im, &[[20.5, 20.5]], 8.0).unwrap();
        assert_eq!(*im.get_pixel(20, 20), Rgba([200, 100, 50, 255]));
        assert_eq!(*im.get_pixel(0, 0), Rgba([200, 100, 50, 255]));
    }
    #[test]
    fn filled_shapes_keep_independent_outline_and_fill_colors() {
        for ellipse in [false, true] {
            let mut image = RgbaImage::new(80, 80);
            shape(&mut image, [20.5, 20.5], [60.5, 60.5], 4.0,
                [255, 0, 0], ellipse, Some([0, 0, 255]));
            assert_eq!(image.get_pixel(40, 40).0, [0, 0, 255, 255]);
            assert_eq!(image.get_pixel(20, 40).0, [255, 0, 0, 255]);
            assert_eq!(image.get_pixel(0, 0)[3], 0);
        }
    }
    #[test]
    fn shapes_have_hollow_and_filled_modes() {
        let mut im = RgbaImage::new(40, 40);
        shape(
            &mut im,
            [5.0, 5.0],
            [35.0, 35.0],
            2.0,
            [255; 3],
            false,
            None,
        );
        assert_eq!(im.get_pixel(20, 20)[3], 0);
        assert!(im.get_pixel(5, 20)[3] > 0);
        shape(
            &mut im,
            [10.0, 10.0],
            [30.0, 30.0],
            2.0,
            [255; 3],
            true,
            Some([255; 3]),
        );
        assert_eq!(im.get_pixel(20, 20)[3], 255);
        assert_eq!(im.get_pixel(0, 0)[3], 0);
    }
}

/// Shared by the live preview and rasterizer, so arrow tips follow the drag direction.
pub fn arrow_segments(a: [f32; 2], b: [f32; 2], width: f32) -> Vec<[[f32; 2]; 2]> {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let length = dx.hypot(dy);
    if length < 0.5 {
        return Vec::new();
    }
    let ux = dx / length;
    let uy = dy / length;
    let head = (width * 3.0).max(12.0).min(length * 0.45);
    let left = [
        b[0] - ux * head - uy * head * 0.6,
        b[1] - uy * head + ux * head * 0.6,
    ];
    let right = [
        b[0] - ux * head + uy * head * 0.6,
        b[1] - uy * head - ux * head * 0.6,
    ];
    vec![[a, b], [left, b], [right, b]]
}
pub fn arrow(im: &mut RgbaImage, a: [f32; 2], b: [f32; 2], width: f32, color: [u8; 3]) {
    for [a, b] in arrow_segments(a, b, width) {
        brush(im, a, b, width, 1.0, color);
    }
}
pub fn sample(im: &RgbaImage, p: [f32; 2]) -> [u8; 3] {
    let pixel = im.get_pixel(
        (p[0].floor().max(0.0) as u32).min(im.width() - 1),
        (p[1].floor().max(0.0) as u32).min(im.height() - 1),
    );
    [pixel[0], pixel[1], pixel[2]]
}
#[cfg(test)]
mod annotation_tests {
    use super::*;
    #[test]
    fn arrows_follow_direction_and_keep_other_pixels() {
        let mut im = RgbaImage::new(100, 100);
        arrow(&mut im, [15.0, 80.0], [80.0, 15.0], 3.0, [200, 10, 20]);
        assert!(im.get_pixel(50, 45)[3] > 0);
        assert!(im.get_pixel(79, 15)[3] > 0);
        assert_eq!(im.get_pixel(0, 0)[3], 0);
        let before = im.clone();
        arrow(&mut im, [50.0, 50.0], [50.0, 50.0], 3.0, [255; 3]);
        assert_eq!(im, before);
    }
    #[test]
    fn eyedropper_samples_source_pixels_without_changes() {
        let mut im = RgbaImage::new(4, 4);
        im.put_pixel(2, 1, Rgba([12, 34, 56, 100]));
        let before = im.clone();
        assert_eq!(sample(&im, [2.9, 1.1]), [12, 34, 56]);
        assert_eq!(im, before);
    }
}

/// Snap in display coordinates before mapping back to the source image.
pub fn constrain(a: [f32; 2], b: [f32; 2], diagonal: bool) -> [f32; 2] {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    if diagonal && dy.abs() > dx.abs() * 0.41421357 && dx.abs() > dy.abs() * 0.41421357 {
        let d = (dx.abs() + dy.abs()) * 0.5;
        return [a[0] + d * dx.signum(), a[1] + d * dy.signum()];
    }
    if dx.abs() >= dy.abs() {
        [b[0], a[1]]
    } else {
        [a[0], b[1]]
    }
}
/// Sample the immutable pre-stroke image: overlapping strokes cannot feed back into themselves.
pub fn clone_stamp(
    im: &mut RgbaImage,
    source: &RgbaImage,
    a: [f32; 2],
    b: [f32; 2],
    offset: [f32; 2],
    diameter: f32,
    hardness: f32,
) -> Bounds {
    let r = diameter.max(1.0) * 0.5;
    let bounds = Bounds::around(a, b, r + 1.0, im);
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let length = dx * dx + dy * dy;
    let inner = r * hardness.clamp(0.0, 1.0);
    for y in bounds.y..bounds.y + bounds.h {
        let py = y as f32 + 0.5;
        let (left, right) = if dy.abs() > 0.001 {
            let t0 = ((py - r - 1.0 - a[1]) / dy).clamp(0.0, 1.0);
            let t1 = ((py + r + 1.0 - a[1]) / dy).clamp(0.0, 1.0);
            let x0 = a[0] + dx * t0;
            let x1 = a[0] + dx * t1;
            (
                (x0.min(x1) - r - 1.0).floor().max(bounds.x as f32) as u32,
                (x0.max(x1) + r + 1.0)
                    .ceil()
                    .min((bounds.x + bounds.w) as f32) as u32,
            )
        } else {
            (bounds.x, bounds.x + bounds.w)
        };
        for x in left..right {
            let px = x as f32 + 0.5;
            let t = if length > 0.0 {
                ((px - a[0]) * dx + (py - a[1]) * dy) / length
            } else {
                0.0
            }
            .clamp(0.0, 1.0);
            let distance = (px - a[0] - t * dx).hypot(py - a[1] - t * dy);
            let coverage = if distance <= inner {
                1.0
            } else {
                ((r + 0.5 - distance) / (r - inner + 0.5)).clamp(0.0, 1.0)
            };
            let xx = (px + offset[0]).floor();
            let yy = (py + offset[1]).floor();
            if coverage <= 0.0
                || xx < 0.0
                || yy < 0.0
                || xx >= source.width() as f32
                || yy >= source.height() as f32
            {
                continue;
            }
            let sample = source.get_pixel(xx as u32, yy as u32);
            let dest = im.get_pixel_mut(x, y);
            // Interpolate unassociated RGBA in premultiplied space, including transparent source pixels.
            let sa = sample[3] as f32 / 255.0;
            let da = dest[3] as f32 / 255.0;
            let alpha = sa * coverage + da * (1.0 - coverage);
            for c in 0..3 {
                dest[c] = if alpha > 0.0 {
                    ((sample[c] as f32 * sa * coverage + dest[c] as f32 * da * (1.0 - coverage))
                        / alpha)
                        .round() as u8
                } else {
                    0
                };
            }
            dest[3] = (alpha * 255.0).round() as u8;
        }
    }
    bounds
}
#[cfg(test)]
mod stamp_tests {
    use super::*;
    #[test]
    fn snapping_axes_and_diagonals() {
        assert_eq!(constrain([0.0; 2], [30.0, 5.0], false), [30.0, 0.0]);
        assert_eq!(constrain([0.0; 2], [5.0, 30.0], false), [0.0, 30.0]);
        let p = constrain([0.0; 2], [-20.0, 25.0], true);
        assert_eq!(p, [-22.5, 22.5]);
    }
    #[test]
    fn clone_uses_snapshot_and_handles_alpha_and_bounds() {
        let mut source = RgbaImage::from_pixel(60, 30, Rgba([0, 0, 255, 255]));
        for y in 0..30 {
            for x in 0..20 {
                source.put_pixel(x, y, Rgba([255, 0, 0, 128]));
            }
        }
        let mut target = source.clone();
        clone_stamp(
            &mut target,
            &source,
            [35.5, 15.5],
            [40.5, 15.5],
            [-30.0, 0.0],
            8.0,
            1.0,
        );
        assert_eq!(*target.get_pixel(40, 15), Rgba([255, 0, 0, 128]));
        assert_eq!(*source.get_pixel(40, 15), Rgba([0, 0, 255, 255]));
        let before = target.clone();
        clone_stamp(
            &mut target,
            &source,
            [35.5, 15.5],
            [40.5, 15.5],
            [-100.0, 0.0],
            8.0,
            1.0,
        );
        assert_eq!(target, before);
    }
}

/// Cut/copy from a snapshot first, so overlapping destinations are safe.
pub fn transfer(
    source: &RgbaImage,
    c: crate::model::Crop,
    offset: [i32; 2],
    copy: bool,
) -> RgbaImage {
    let patch = image::imageops::crop_imm(source, c.x, c.y, c.w, c.h).to_image();
    let mut result = source.clone();
    if !copy {
        for y in c.y..c.y + c.h {
            for x in c.x..c.x + c.w {
                result.put_pixel(x, y, Rgba([0; 4]));
            }
        }
    }
    image::imageops::overlay(
        &mut result,
        &patch,
        c.x as i64 + offset[0] as i64,
        c.y as i64 + offset[1] as i64,
    );
    result
}
#[cfg(test)]
mod selection_tests {
    use super::*;
    use crate::model::Crop;
    #[test]
    fn move_overlap_and_copy_preserve_source() {
        let mut source = RgbaImage::new(6, 1);
        for x in 0..6 {
            source.put_pixel(x, 0, Rgba([x as u8 + 1, 0, 0, 255]));
        }
        let c = Crop {
            x: 0,
            y: 0,
            w: 3,
            h: 1,
        };
        let moved = transfer(&source, c, [1, 0], false);
        assert_eq!(moved.get_pixel(0, 0)[3], 0);
        assert_eq!(moved.get_pixel(1, 0)[0], 1);
        assert_eq!(moved.get_pixel(3, 0)[0], 3);
        assert_eq!(source.get_pixel(0, 0)[0], 1);
        let copy = transfer(&source, c, [3, 0], true);
        assert_eq!(copy.get_pixel(0, 0)[0], 1);
        assert_eq!(copy.get_pixel(3, 0)[0], 1);
    }
    #[test]
    fn copy_composites_alpha_and_zero_move_is_identity_for_opaque() {
        let source = RgbaImage::from_pixel(4, 4, Rgba([10, 20, 30, 255]));
        let c = Crop {
            x: 0,
            y: 0,
            w: 2,
            h: 2,
        };
        assert_eq!(transfer(&source, c, [0, 0], false), source);
        let mut im = source.clone();
        im.put_pixel(0, 0, Rgba([255, 0, 0, 0]));
        assert_eq!(
            *transfer(
                &im,
                Crop {
                    x: 0,
                    y: 0,
                    w: 1,
                    h: 1
                },
                [2, 0],
                true
            )
            .get_pixel(2, 0),
            Rgba([10, 20, 30, 255])
        );
    }
}
