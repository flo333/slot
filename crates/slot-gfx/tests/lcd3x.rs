use slot_gfx::{
    canvas_size, lcd3x_factors, lcd3x_mask, lcd3x_mask_fits, mask_texture_rgba8, WHOLE_TEXTURE,
};
use std::f64::consts::PI;

const SRC_W: usize = 240;
const SRC_H: usize = 160;
const OUT_W: usize = 720;
const OUT_H: usize = 480;
const SCALE: usize = 3;

const BRIGHTEN_SCANLINES: f64 = 16.0;
const BRIGHTEN_LCD: f64 = 4.0;

fn pseudorandom_240x160() -> Vec<u8> {
    let mut s: u32 = 0x9e37_79b9;
    (0..SRC_W * SRC_H * 3)
        .map(|_| {
            s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (s >> 24) as u8
        })
        .collect()
}

fn render_reference(src: &[u8]) -> Vec<u8> {
    let mut out = vec![0u8; OUT_W * OUT_H * 3];
    for oy in 0..OUT_H {
        let yfactor = (BRIGHTEN_SCANLINES + (PI * (oy as f64 + 0.5) * 2.0 / 3.0).sin())
            / (BRIGHTEN_SCANLINES + 1.0);
        for ox in 0..OUT_W {
            for c in 0..3 {
                let xfactor = (BRIGHTEN_LCD
                    + (PI * (ox as f64 + 0.5) * 2.0 / 3.0 + PI * (0.5 - c as f64 * 2.0 / 3.0))
                        .sin())
                    / (BRIGHTEN_LCD + 1.0);
                let texel = src[((oy / SCALE) * SRC_W + ox / SCALE) * 3 + c] as f64;
                out[(oy * OUT_W + ox) * 3 + c] = (texel * yfactor * xfactor).round() as u8;
            }
        }
    }
    out
}

fn render_with_mask(src: &[u8], mask: &[[[f32; 3]; 3]; 3]) -> Vec<u8> {
    let mut out = vec![0u8; OUT_W * OUT_H * 3];
    for oy in 0..OUT_H {
        for ox in 0..OUT_W {
            let cell = &mask[oy % 3][ox % 3];
            for c in 0..3 {
                let texel = src[((oy / SCALE) * SRC_W + ox / SCALE) * 3 + c] as f32;
                out[(oy * OUT_W + ox) * 3 + c] = (texel * cell[c]).round() as u8;
            }
        }
    }
    out
}

#[test]
fn mask_matches_reference_shader_exactly() {
    let src = pseudorandom_240x160();
    let reference = render_reference(&src);
    let optimized = render_with_mask(&src, &lcd3x_mask());
    let worst = reference
        .iter()
        .zip(&optimized)
        .map(|(a, b)| (*a as i32 - *b as i32).abs())
        .max()
        .unwrap();
    assert!(
        worst <= 1,
        "max channel deviation {worst}, expected <= 1 (rounding only)"
    );
}

#[test]
fn every_column_of_a_source_pixel_dims_exactly_one_channel() {
    for row in lcd3x_mask() {
        for cell in row {
            let dimmest = cell.iter().cloned().fold(f32::MAX, f32::min);
            assert_eq!(
                cell.iter().filter(|v| **v == dimmest).count(),
                1,
                "{cell:?}"
            );
        }
    }
}

#[test]
fn the_pattern_repeats_once_per_source_pixel_at_any_scale() {
    for scale in [2.0f32, 2.6667, 3.0, 4.5] {
        for i in 0..20 {
            let x = i as f32 / scale;
            let a = lcd3x_factors(x, 0.25);
            let b = lcd3x_factors(x + 1.0, 0.25);
            for c in 0..3 {
                assert!((a[c] - b[c]).abs() < 1e-4, "scale {scale}");
            }
        }
    }
}

#[test]
fn quantised_mask_texture_stays_within_one_lsb_of_the_reference() {
    let src = pseudorandom_240x160();
    let reference = render_reference(&src);
    let tex = mask_texture_rgba8();
    let mut mask = [[[0.0f32; 3]; 3]; 3];
    for (y, row) in mask.iter_mut().enumerate() {
        for (x, cell) in row.iter_mut().enumerate() {
            for (c, v) in cell.iter_mut().enumerate() {
                *v = tex[(y * 3 + x) * 4 + c] as f32 / 255.0;
            }
        }
    }
    let optimized = render_with_mask(&src, &mask);
    let worst = reference
        .iter()
        .zip(&optimized)
        .map(|(a, b)| (*a as i32 - *b as i32).abs())
        .max()
        .unwrap();
    assert!(
        worst <= 1,
        "max channel deviation {worst} from the 8 bit mask"
    );
}

fn fbo_scale(window: (u32, u32)) -> [f32; 2] {
    let (w, h) = canvas_size(window);
    [w as f32 / OUT_W as f32, h as f32 / OUT_H as f32]
}

#[test]
fn the_mask_texture_draws_only_where_every_game_pixel_is_three_by_three() {
    for window in [(720, 480), (1280, 720), (1440, 960)] {
        assert!(
            lcd3x_mask_fits(fbo_scale(window), WHOLE_TEXTURE, WHOLE_TEXTURE),
            "{window:?} is an exact 3x canvas but skipped the mask"
        );
    }
    for window in [(640, 480), (1024, 768), (480, 320)] {
        assert!(
            !lcd3x_mask_fits(fbo_scale(window), WHOLE_TEXTURE, WHOLE_TEXTURE),
            "{window:?} is not 3x but took the mask"
        );
    }
}

#[test]
fn a_cropped_source_or_picture_keeps_the_computed_grille() {
    let gb = [40.0 / 240.0, 8.0 / 160.0, 160.0 / 240.0, 144.0 / 160.0];
    assert!(!lcd3x_mask_fits([1.0, 1.0], gb, WHOLE_TEXTURE));
    let pic = [40.0 / 240.0, 8.0 / 160.0, 200.0 / 240.0, 152.0 / 160.0];
    assert!(!lcd3x_mask_fits([1.0, 1.0], WHOLE_TEXTURE, pic));
}
