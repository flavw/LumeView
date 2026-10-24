//! Smooth-background removal using exterior samples only; no models or network.
use image::RgbaImage;
use crate::paint::{brush, Bounds};

fn sample(pixel: &image::Rgba<u8>) -> [f32;4] {
    let alpha = f32::from(pixel[3]);
    [f32::from(pixel[0])*alpha/255.0,f32::from(pixel[1])*alpha/255.0,
        f32::from(pixel[2])*alpha/255.0,alpha]
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
    let mut count=0usize;let mut mean=[0.0_f64;4];
    for index in 0..pixels.len() {
        if marked[index] {continue;}
        pixels[index]=sample(image.get_pixel(bounds.x+(index%w) as u32,bounds.y+(index/w) as u32));
        if !neighbors(index).into_iter().flatten().any(|n|marked[n]) {continue;}
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
        let p=pixels[index];let alpha=p[3].clamp(0.0,255.0);
        let pixel=if alpha<0.5 {[0;4]} else {
            [(p[0]*255.0/alpha).clamp(0.0,255.0).round() as u8,
             (p[1]*255.0/alpha).clamp(0.0,255.0).round() as u8,
             (p[2]*255.0/alpha).clamp(0.0,255.0).round() as u8,alpha.round() as u8]
        };
        image.put_pixel(bounds.x+(index%w) as u32,bounds.y+(index/w) as u32,image::Rgba(pixel));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
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
