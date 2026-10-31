//! Exterior-only removal: copy matching detail, interpolate smooth backgrounds.
use image::RgbaImage;
use crate::paint::{brush, Bounds};

fn sample(pixel: &image::Rgba<u8>) -> [f32;4] {
    let alpha = f32::from(pixel[3]);
    [f32::from(pixel[0])*alpha/255.0,f32::from(pixel[1])*alpha/255.0,
        f32::from(pixel[2])*alpha/255.0,alpha]
}
fn from_sample(p: [f32;4]) -> image::Rgba<u8> {
    let alpha=p[3].clamp(0.0,255.0);
    image::Rgba(if alpha<0.5 {[0;4]} else {
        [(p[0]*255.0/alpha).clamp(0.0,255.0).round() as u8,
         (p[1]*255.0/alpha).clamp(0.0,255.0).round() as u8,
         (p[2]*255.0/alpha).clamp(0.0,255.0).round() as u8,alpha.round() as u8]
    })
}
fn solve(mut matrix: [[f64;3];3],mut values: [[f64;4];3]) -> Option<[[f64;4];3]> {
    for column in 0..3 {
        let pivot = (column..3).max_by(|&a,&b|matrix[a][column].abs().total_cmp(&matrix[b][column].abs())).unwrap();
        if matrix[pivot][column].abs()<1e-10 { return None; }
        matrix.swap(column,pivot);values.swap(column,pivot);
        let divisor = matrix[column][column];
        for j in column..3 { matrix[column][j]/=divisor; }
        for c in 0..4 { values[column][c]/=divisor; }
        for row in 0..3 {
            if row==column {continue;}
            let factor=matrix[row][column];
            for j in column..3 {matrix[row][j]-=factor*matrix[column][j];}
            for c in 0..4 {values[row][c]-=factor*values[column][c];}
        }
    }
    Some(values)
}
/// Match a nearby intact rectangle against the known pixels bordering the mask.
/// Bounded coarse-to-fine search; candidates cannot overlap any removed pixels.
fn matching_offset(image: &RgbaImage,bounds: Bounds,marked: &[bool],border: &[usize]) -> Option<[i64;2]> {
    let w=bounds.w as usize;
    let mut min=[bounds.w as i64,bounds.h as i64];let mut max=[0_i64;2];
    for (index,&removed) in marked.iter().enumerate() {
        if removed {
            let p=[(index%w) as i64,(index/w) as i64];
            for axis in 0..2 {min[axis]=min[axis].min(p[axis]);max[axis]=max[axis].max(p[axis]);}
        }
    }
    if min[0]>max[0] {return None;}
    let origin=[bounds.x as i64,bounds.y as i64];
    let dimensions=[image.width() as i64,image.height() as i64];
    let radius=((max[0]-min[0]+1).max(max[1]-min[1]+1)*2).clamp(48,256);
    let samples:Vec<_>=border.iter().step_by(border.len().div_ceil(96).max(1)).map(|&index| {
        let p=[origin[0]+(index%w) as i64,origin[1]+(index/w) as i64];
        (p,sample(image.get_pixel(p[0] as u32,p[1] as u32)))
    }).collect();
    if samples.is_empty(){return None;}
    let score=|offset:[i64;2]| -> Option<f64> {
        // Reject bounding-box overlap, including intact holes in a complex stroke.
        if offset[0].abs()<=max[0]-min[0] && offset[1].abs()<=max[1]-min[1] {return None;}
        for axis in 0..2 {
            if origin[axis]+min[axis]+offset[axis]<0 || origin[axis]+max[axis]+offset[axis]>=dimensions[axis] {return None;}
        }
        let mut error=0.0;
        for &(p,color) in &samples {
            let q=[p[0]+offset[0],p[1]+offset[1]];
            if q[0]<0 || q[1]<0 || q[0]>=dimensions[0] || q[1]>=dimensions[1] {return None;}
            // The context must also come entirely from the original unmarked image.
            let local=[q[0]-origin[0],q[1]-origin[1]];
            if local[0]>=0 && local[1]>=0 && local[0]<bounds.w as i64 && local[1]<bounds.h as i64
                && marked[local[1] as usize*w+local[0] as usize] {return None;}
            let source=sample(image.get_pixel(q[0] as u32,q[1] as u32));
            for c in 0..4 {error+=f64::from(color[c]-source[c]).powi(2);}
        }
        Some(error/samples.len() as f64)
    };
    let mut candidates:Vec<(f64,[i64;2])>=Vec::new();
    let step=(radius/32).max(1);
    {
    let mut consider=|offset| {
        if candidates.iter().any(|(_,p)|*p==offset){return;}
        if let Some(error)=score(offset) {
            candidates.push((error,offset));
            candidates.sort_by(|a,b|a.0.total_cmp(&b.0).then_with(|| {
                let distance=|p:[i64;2]|p[0]*p[0]+p[1]*p[1];
                distance(a.1).cmp(&distance(b.1))
            }));
            candidates.truncate(8);
        }
    };
    for y in (-radius..=radius).step_by(step as usize) {
        for x in (-radius..=radius).step_by(step as usize) {consider([x,y]);}
    }
    }
    // Narrow search around several good matches to avoid favoring a single grid phase.
    let mut scale=step;
    while scale>0 {
        let centers=candidates.clone();
        for (_,center) in centers {
            for y in -1..=1 {for x in -1..=1 {
                let offset=[center[0]+x*scale,center[1]+y*scale];
                if offset[0].abs()>radius || offset[1].abs()>radius || candidates.iter().any(|(_,p)|*p==offset){continue;}
                if let Some(error)=score(offset){candidates.push((error,offset));}
            }}
        }
        candidates.sort_by(|a,b|a.0.total_cmp(&b.0));candidates.truncate(8);
        if scale==1 {break;}scale=(scale/2).max(1);
    }
    candidates.first().map(|(_,offset)|*offset)
}
/// Fade only the donor's boundary color mismatch into a two-pixel inner band.
/// This corrects seams without blurring donor texture or reading the removed object.
fn soften_patch_edge(image: &RgbaImage,bounds: Bounds,marked: &[bool],border: &[usize],unknown: &[usize],offset: [i64;2],copied: &mut [image::Rgba<u8>]) {
    const BAND:u8=2;
    let w=bounds.w as usize;let h=bounds.h as usize;
    let mut depth=vec![u8::MAX;marked.len()];
    let mut queue=std::collections::VecDeque::new();
    for &index in border {depth[index]=0;queue.push_back(index);}
    while let Some(index)=queue.pop_front() {
        if depth[index]>=BAND {continue;}
        let x=index%w;let y=index/w;
        for next in [if x>0 {Some(index-1)}else{None},if x+1<w {Some(index+1)}else{None},
            if y>0 {Some(index-w)}else{None},if y+1<h {Some(index+w)}else{None}].into_iter().flatten() {
            if marked[next] && depth[next]==u8::MAX {
                depth[next]=depth[index]+1;queue.push_back(next);
            }
        }
    }
    for (&index,pixel) in unknown.iter().zip(copied) {
        if depth[index]>BAND {continue;}
        let x=(index%w) as i64;let y=(index/w) as i64;
        let mut correction=[0.0_f32;4];let mut total=0.0_f32;
        for dy in -3_i64..=3 {for dx in -3_i64..=3 {
            let q=[x+dx,y+dy];
            if q[0]<0 || q[1]<0 || q[0]>=w as i64 || q[1]>=h as i64 {continue;}
            if marked[q[1] as usize*w+q[0] as usize] {continue;}
            let donor=[bounds.x as i64+q[0]+offset[0],bounds.y as i64+q[1]+offset[1]];
            if donor[0]<0 || donor[1]<0 || donor[0]>=image.width() as i64 || donor[1]>=image.height() as i64 {continue;}
            let local=[donor[0]-bounds.x as i64,donor[1]-bounds.y as i64];
            if local[0]>=0 && local[1]>=0 && local[0]<w as i64 && local[1]<h as i64
                && marked[local[1] as usize*w+local[0] as usize] {continue;}
            let target=sample(image.get_pixel(bounds.x+q[0] as u32,bounds.y+q[1] as u32));
            let source=sample(image.get_pixel(donor[0] as u32,donor[1] as u32));
            let weight=1.0/(dx*dx+dy*dy) as f32;
            for c in 0..4 {correction[c]+=(target[c]-source[c])*weight;}
            total+=weight;
        }}
        if total==0.0 || correction.iter().all(|v|v.abs()<0.0001) {continue;}
        let amount=f32::from(BAND+1-depth[index])/f32::from(BAND+1);
        let mut color=sample(pixel);
        for c in 0..4 {color[c]+=correction[c]/total*amount;}
        *pixel=from_sample(color);
    }
}
fn fill_detail(image: &mut RgbaImage,bounds: Bounds,marked: &[bool],border: &[usize],unknown: &[usize]) {
    let w=bounds.w as usize;
    if let Some(offset)=matching_offset(image,bounds,marked,border) {
        // Read all donor pixels before writing, so one repair never samples itself.
        let mut copied:Vec<_>=unknown.iter().map(|&index|*image.get_pixel(
            (bounds.x as i64+(index%w) as i64+offset[0]) as u32,
            (bounds.y as i64+(index/w) as i64+offset[1]) as u32)).collect();
        soften_patch_edge(image,bounds,marked,border,unknown,offset,&mut copied);
        for (&index,pixel) in unknown.iter().zip(copied) {
            image.put_pixel(bounds.x+(index%w) as u32,bounds.y+(index/w) as u32,pixel);
        }
        return;
    }
    // Near image edges an intact donor rectangle may not fit. Extend the closest
    // exterior samples without averaging contrasting colors into a blurred seam.
    let h=bounds.h as usize;
    let mut known:Vec<_>=marked.iter().map(|v|!*v).collect();
    let mut queue:std::collections::VecDeque<_>=border.iter().copied().collect();
    while let Some(index)=queue.pop_front() {
        let x=index%w;let y=index/w;
        for next in [if x>0 {Some(index-1)}else{None},if x+1<w {Some(index+1)}else{None},
            if y>0 {Some(index-w)}else{None},if y+1<h {Some(index+w)}else{None}].into_iter().flatten() {
            if known[next]{continue;}
            let pixel=*image.get_pixel(bounds.x+x as u32,bounds.y+y as u32);
            image.put_pixel(bounds.x+(next%w) as u32,bounds.y+(next/w) as u32,pixel);
            known[next]=true;queue.push_back(next);
        }
    }
}
pub fn apply(image: &mut RgbaImage,path: &[[f32;2]],diameter: f32) -> Result<(),String> {
    let Some(&first)=path.first() else {return Ok(());};
    let (mut min,mut max)=(first,first);
    for p in path {for axis in 0..2 {min[axis]=min[axis].min(p[axis]);max[axis]=max[axis].max(p[axis]);}}
    let bounds=Bounds::around(min,max,diameter.max(1.0)*0.5+3.0,image);
    if u64::from(bounds.w)*u64::from(bounds.h)>2_000_000 {
        return Err("Use a ferramenta Remover em uma área menor.".into());
    }
    if bounds.w==0 || bounds.h==0 {return Ok(());}
    let mut mask=RgbaImage::new(bounds.w,bounds.h);
    let mut previous=[first[0]-bounds.x as f32,first[1]-bounds.y as f32];
    for p in path {
        let point=[p[0]-bounds.x as f32,p[1]-bounds.y as f32];
        brush(&mut mask,previous,point,diameter,1.0,[255;3]);previous=point;
    }
    // Include the entire antialiased fringe in the removal, never mix its old RGB back in.
    let marked:Vec<_>=mask.pixels().map(|p|p[3]>0).collect();
    let w=bounds.w as usize;let h=bounds.h as usize;
    let neighbors=|index:usize| {
        let x=index%w;let y=index/w;
        [if x>0 {Some(index-1)}else{None},if x+1<w {Some(index+1)}else{None},
         if y>0 {Some(index-w)}else{None},if y+1<h {Some(index+w)}else{None}]
    };
    let position=|index:usize| [1.0,(index%w) as f64/w as f64,(index/w) as f64/h as f64];
    let mut pixels=vec![[0.0_f32;4];w*h];
    let mut matrix=[[0.0;3];3];let mut values=[[0.0;4];3];
    let mut count=0usize;let mut mean=[0.0_f64;4];let mut border=Vec::new();
    for index in 0..pixels.len() {
        if marked[index] {continue;}
        pixels[index]=sample(image.get_pixel(bounds.x+(index%w) as u32,bounds.y+(index/w) as u32));
        if !neighbors(index).into_iter().flatten().any(|n|marked[n]) {continue;}
        border.push(index);
        count+=1;
        let p=position(index);
        for c in 0..4 {mean[c]+=f64::from(pixels[index][c]);}
        for row in 0..3 {
            for column in 0..3 {matrix[row][column]+=p[row]*p[column];}
            for c in 0..4 {values[row][c]+=p[row]*f64::from(pixels[index][c]);}
        }
    }
    if count==0 {return Err("Marque uma área menor e deixe pixels do fundo ao redor.".into());}
    for c in &mut mean {*c/=count as f64;}
    let plane=solve(matrix,values).unwrap_or([mean,[0.0;4],[0.0;4]]);
    let unknown:Vec<_>=(0..pixels.len()).filter(|&i|marked[i]).collect();
    // A plane describes solid colors and smooth gradients. Its residual identifies
    // edges/texture that averaging would destroy, using only the untouched border.
    let mut residual=0.0_f64;
    for &index in &border {
        let p=position(index);
        for c in 0..4 {
            let predicted=plane[0][c]+p[1]*plane[1][c]+p[2]*plane[2][c];
            residual+=(predicted-f64::from(pixels[index][c])).powi(2);
        }
    }
    if residual/(border.len()*4) as f64>4.0 {
        drop(pixels);
        fill_detail(image,bounds,&marked,&border,&unknown);
        return Ok(());
    }
    for &index in &unknown {
        let p=position(index);
        for c in 0..4 {pixels[index][c]=(plane[0][c]+p[1]*plane[1][c]+p[2]*plane[2][c]).clamp(0.0,255.0) as f32;}
    }
    // Refine the exterior-derived fill without ever reading the original marked pixels.
    for _ in 0..48 {
        let mut change=0.0_f32;
        for reverse in [false,true] {
            for step in 0..unknown.len() {
                let index=unknown[if reverse {unknown.len()-step-1}else{step}];
                let mut sum=[0.0_f32;4];let mut n=0;
                for neighbor in neighbors(index).into_iter().flatten() {
                    n+=1;for c in 0..4 {sum[c]+=pixels[neighbor][c];}
                }
                if n==0 {continue;}
                for c in 0..4 {
                    let value=sum[c]/n as f32;
                    change=change.max((value-pixels[index][c]).abs());pixels[index][c]=value;
                }
            }
        }
        if change<0.05 {break;}
    }
    for index in unknown {
        image.put_pixel(bounds.x+(index%w) as u32,bounds.y+(index/w) as u32,from_sample(pixels[index]));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn edge_fixture(target:image::Rgba<u8>,donor:image::Rgba<u8>,object:image::Rgba<u8>) -> (RgbaImage,Bounds,Vec<bool>,Vec<usize>,Vec<usize>) {
        let mut image=RgbaImage::from_fn(80,24,|x,_|if x<24 {target}else{donor});
        let bounds=Bounds{x:0,y:0,w:24,h:24};
        let marked:Vec<_>=(0..24*24).map(|i|(6..18).contains(&(i%24)) && (6..18).contains(&(i/24))).collect();
        let unknown:Vec<_>=(0..marked.len()).filter(|&i|marked[i]).collect();
        let border:Vec<_>=(0..marked.len()).filter(|&i|!marked[i] && [
            i.checked_sub(1).filter(|_|i%24>0),Some(i+1).filter(|_|i%24<23),
            i.checked_sub(24),Some(i+24).filter(|&n|n<marked.len())
        ].into_iter().flatten().any(|n|marked[n])).collect();
        for &i in &unknown {image.put_pixel((i%24) as u32,(i/24) as u32,object);}
        (image,bounds,marked,border,unknown)
    }
    #[test]
    fn softens_only_the_two_pixel_inner_border_without_reusing_removed_colors() {
        let mut results=Vec::new();
        for object in [image::Rgba([0,0,0,255]),image::Rgba([255,0,255,255])] {
            let donor=image::Rgba([200,200,200,255]);
            let (image,bounds,marked,border,unknown)=edge_fixture(image::Rgba([100,100,100,255]),donor,object);
            let untouched=image.clone();let mut copied=vec![donor;unknown.len()];
            soften_patch_edge(&image,bounds,&marked,&border,&unknown,[32,0],&mut copied);
            for (&i,pixel) in unknown.iter().zip(&copied) {
                let x=i%24;let y=i/24;let depth=(x-5).min(18-x).min(y-5).min(18-y);
                let expected=match depth {1=>133,2=>167,_=>200};
                assert_eq!(*pixel,image::Rgba([expected,expected,expected,255]));
            }
            assert_eq!(image,untouched);results.push(copied);
        }
        assert_eq!(results[0],results[1]);
    }
    #[test]
    fn feathered_transparency_does_not_create_dark_or_hidden_color_halos() {
        let donor=image::Rgba([255,0,255,0]);
        let (image,bounds,marked,border,unknown)=edge_fixture(image::Rgba([20,100,220,128]),donor,image::Rgba([0,0,0,255]));
        let mut copied=vec![donor;unknown.len()];
        soften_patch_edge(&image,bounds,&marked,&border,&unknown,[32,0],&mut copied);
        for (&i,pixel) in unknown.iter().zip(&copied) {
            let x=i%24;let y=i/24;let depth=(x-5).min(18-x).min(y-5).min(18-y);
            if depth<=2 {
                assert_eq!(&pixel.0[..3],&[20,100,220]);
                assert_eq!(pixel[3],if depth==1 {85}else{43});
            } else {assert_eq!(*pixel,donor);}
        }
    }
    #[test]
    fn preserves_a_sharp_edge_instead_of_blending_its_colors() {
        let left=image::Rgba([20,100,220,255]);let right=image::Rgba([240,240,240,255]);
        let original=RgbaImage::from_fn(100,100,|x,_|if x<50 {left}else{right});
        let mut repaired=original.clone();
        for y in 42..58 {for x in 42..58 {repaired.put_pixel(x,y,image::Rgba([0,0,0,255]));}}
        apply(&mut repaired,&[[50.0,50.0]],30.0).unwrap();
        assert_eq!(repaired,original);
    }
    #[test]
    fn copies_repeating_texture_without_blur_or_original_object_colors() {
        let original=RgbaImage::from_fn(128,128,|x,y| {
            if (x/5+y/5)%2==0 {image::Rgba([35,95,160,180])}else{image::Rgba([230,210,185,255])}
        });
        for object in [[0,0,0,255],[255,0,255,0]] {
            let mut repaired=original.clone();
            for y in 56..72 {for x in 56..72 {repaired.put_pixel(x,y,image::Rgba(object));}}
            apply(&mut repaired,&[[64.0,64.0]],34.0).unwrap();
            assert_eq!(repaired,original);
        }
    }
    #[test]
    fn image_edge_fallback_does_not_mix_contrasting_colors() {
        let a=image::Rgba([20,100,220,255]);let b=image::Rgba([240,240,240,255]);
        let mut repaired=RgbaImage::from_fn(16,16,|x,_|if x<8 {a}else{b});
        let corners=[*repaired.get_pixel(0,0),*repaired.get_pixel(15,15)];
        for y in 4..12 {for x in 4..12 {repaired.put_pixel(x,y,image::Rgba([0,0,0,255]));}}
        apply(&mut repaired,&[[8.0,8.0]],18.0).unwrap();
        assert!(repaired.pixels().all(|p|*p==a || *p==b));
        assert_eq!(*repaired.get_pixel(0,0),corners[0]);assert_eq!(*repaired.get_pixel(15,15),corners[1]);
    }
    #[test]
    fn removes_black_circle_including_its_fringe_without_changing_the_background() {
        let blue=image::Rgba([20,100,220,255]);
        let mut im=RgbaImage::from_pixel(100,100,blue);
        for (x,y,p) in im.enumerate_pixels_mut() {
            let d=(x as f32-50.0).hypot(y as f32-50.0);
            if d<18.0 {*p=image::Rgba([0,0,0,255]);}
            else if d<19.0 {*p=image::Rgba([10,50,110,255]);}
        }
        apply(&mut im,&[[50.0,50.0]],42.0).unwrap();
        assert!(im.pixels().all(|p|*p==blue));
    }
    #[test]
    fn reconstructs_a_gradient_and_ignores_all_colors_inside_the_mask() {
        let original=RgbaImage::from_fn(80,80,|x,y|image::Rgba([x as u8*2,y as u8*2,180,255]));
        let mut im=original.clone();
        for y in 29..51 {for x in 29..51 {im.put_pixel(x,y,image::Rgba([0,0,0,255]));}}
        apply(&mut im,&[[40.0,40.0]],38.0).unwrap();
        for (x,y,p) in im.enumerate_pixels() {for c in 0..4 {assert!(p[c].abs_diff(original.get_pixel(x,y)[c])<=1,"pixel {x},{y}");}}
    }
    #[test]
    fn rejects_a_mask_with_no_surrounding_background_without_modifying_the_image() {
        let mut im=RgbaImage::from_pixel(10,10,image::Rgba([5,10,20,255]));let before=im.clone();
        assert!(apply(&mut im,&[[5.0,5.0]],30.0).is_err());assert_eq!(im,before);
    }
    #[test]
    fn respects_transparency_and_image_edges() {
        let blue=image::Rgba([20,100,220,128]);let mut im=RgbaImage::from_pixel(40,40,blue);
        for y in 0..8 {for x in 0..8 {im.put_pixel(x,y,image::Rgba([0;4]));}}
        apply(&mut im,&[[0.0,0.0]],26.0).unwrap();
        assert!(im.pixels().all(|p|*p==blue));
    }
}
