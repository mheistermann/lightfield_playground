extern crate cgmath;
extern crate image;
extern crate lightfield_loader;
#[macro_use]
extern crate log;
extern crate env_logger;

use std::f32;
use std::cmp;
use std::path::Path;
use std::fs::File;

use lightfield_loader::{Lightfield, LightfieldView};

use cgmath::{InnerSpace, Vector2, VectorSpace};

use image::{DynamicImage, GenericImage, ImageFormat, Pixel, Rgb};


fn ptr_eq<T>(a: *const T, b: *const T) -> bool {
    a == b
}

/// computate average camera position.
fn find_center(lf: &Lightfield) -> Vector2<f32> {
    let mut sum = Vector2::zero();
    for view in &lf.views {
        sum += view.pos;
    }
    let count = lf.views.len() as f32;
    sum / count
}

fn find_closest_view<'a>(lf: &'a Lightfield, centerpos: &Vector2<f32>) -> &'a LightfieldView {
    let mut best = &lf.views[0];
    let mut best_sqdist = f32::INFINITY;
    for view in &lf.views {
        let sqdist = (view.pos - centerpos).magnitude2();
        if sqdist < best_sqdist {
            best_sqdist = sqdist;
            best = view;
        }
    }
    return &best;
}

const PATCH_RADIUS: u32 = 3;
const NCHAN:usize = 3;
/// number of samples
const PATCH_LEN: usize = NCHAN * ((PATCH_RADIUS*2 + 1) * (PATCH_RADIUS*2 + 1)) as usize;
struct ImagePatch {
    data: [u8; PATCH_LEN as usize],
}

impl ImagePatch {
    pub fn new() -> ImagePatch {
        return ImagePatch { data: [0; PATCH_LEN] };
    }
    pub fn cmp(&self, other: &ImagePatch) -> f32 {
        let mut res = 0i32;
        for (a,b) in self.data.iter().zip(other.data.iter()) {
            let d = (*a as i32) - (*b as i32);
            res += d*d;
        }
        res as f32
    }
}

/// return false for out of bounds access
fn get_patch(img: &DynamicImage, posf: &Vector2<f32>, out: &mut ImagePatch) -> bool {
    // TODO: maybe implement subpixel accuracy! for now, round this stuff...
    let pos = Vector2::<u32>::new(posf.x.round() as u32, posf.y.round() as u32);
    let miny = pos.y-PATCH_RADIUS;
    let maxy = pos.y+PATCH_RADIUS;
    let minx = pos.x-PATCH_RADIUS;
    let maxx = pos.x+PATCH_RADIUS;
    if (minx as i32) < 0 || (miny as i32) < 0 || maxx >= img.width() || maxy >= img.height() {
        return false
    }
    let mut idx = 0;
    for y in miny..maxy+1 {
        for x in minx..maxx+1 {
            let p = img.get_pixel(x,y);
            let c = p.channels();
            out.data[idx+0] = c[0];
            out.data[idx+1] = c[1];
            out.data[idx+2] = c[2];
            idx += 3;
        }
    }
    // TODO: maybe blur first?
    // TODO implement me
    return true;
}

fn min<T: PartialOrd>(a: T, b: T) -> T {
    if a < b {
        a
    } else {
        b
    }
}

const DEBUG_IMAGES: bool = true;

/// pos is the pixel position - might support subpixel accuracy some day?
fn find_correspondences(lf: &Lightfield, mainview: &LightfieldView, pos: &Vector2<u32>) {
    // TODO convert to more suitable colorspace first!
    let mut main_patch: ImagePatch = ImagePatch::new();
    let mut test_patch: ImagePatch = ImagePatch::new();
    let fpos = Vector2::<f32>::new(pos.x as f32, pos.y as f32);
    get_patch(&mainview.image, &fpos, &mut main_patch);

    let debug_pixel = Rgb::from_channels(0, 255, 0, 0);

    let mut cnt = 0;
    for view in &lf.views {
        let mut debug_img = None;
        if DEBUG_IMAGES {
            debug_img = Some(view.image.to_rgb());
        }
        if ptr_eq(view, mainview) {
            if DEBUG_IMAGES {
                let mut d = debug_img.unwrap();
                d.put_pixel(pos.x, pos.y, debug_pixel);
                d.save("debug_000000_main.jpg");
            }
            continue;
        }
        let cam_offset = mainview.pos - view.pos;
        // let's scale this so the maximum movement in x or y is 1 pixel
        let ax = cam_offset.x.abs();
        let ay = cam_offset.y.abs();
        let search_step = cam_offset * min(1. / ay, 1. / ax);
        info!("corr: off {:?} -> search step {:?}",
              cam_offset,
              search_step);
        let mut cur = fpos;
        loop {
            if DEBUG_IMAGES {
                let mut d = debug_img.unwrap();
                d.put_pixel(cur.x.round() as u32, cur.y.round() as u32, debug_pixel);
                debug_img = Some(d);
            }
            let in_bounds = get_patch(&view.image, &cur, &mut test_patch);
            // TODO stop earlier by defining a minimum distance for objects
            if !in_bounds {
                break;
            }
            cur += search_step;
            let diff = main_patch.cmp(&test_patch);
        }
        if DEBUG_IMAGES {
            let name = format!("debug_{}.jpg", cnt);
            let mut d = debug_img.unwrap();
            d.save(name);
        }
        cnt += 1;
    }
}

fn main() {
    env_logger::init().unwrap();
    info!("starting up");

    let lf = Lightfield::from_zip("data/chess.jpg.zip").unwrap();
    let centerpos = find_center(&lf);
    println!("centerpos = {:?}", &centerpos);
    let centerview = find_closest_view(&lf, &centerpos);
    println!("closest = {:?}", &centerview.pos);

    // Assume all images have the same dimensions and color format!
    let img = &centerview.image;
    let center = Vector2::<u32>::new(img.width(), img.height()) / 2;


    // TODO: search vectors are independent of position, so compute them first.
    // then compute search step vectors for each view.
    //
    // maybe: ransac-like random subset?
    // maybe: 2 random views, align (rotation) so we can have a stereo line search
    // combine line search results
    // maybe find a way to refine?
    find_correspondences(&lf, centerview, &center);
}
