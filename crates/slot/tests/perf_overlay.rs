mod common;

use common::{app_playing_in, app_playing_with_overlay, tmp_root_with_carts};
use slot::app::{perf_line, App};
use slot_ui::{Draw, OUT_H};

// One second of play: the frames the core showed in it, counted from `shown` on.
fn play_a_second(a: &mut App, shown: u64, frames: u64) -> u64 {
    let t0 = a.now();
    a.note_frames(shown);
    a.tick_ms(t0 + 500);
    a.note_frames(shown + frames / 2);
    a.tick_ms(t0 + 1000);
    a.note_frames(shown + frames);
    shown + frames
}

fn plates(a: &App) -> usize {
    let mut out = Vec::new();
    a.draw(&mut out);
    out.iter()
        .filter(|d| matches!(d, Draw::Rect { x, y, .. } if *x < 20.0 && *y > OUT_H as f32 / 2.0))
        .count()
}

#[test]
fn a_second_of_play_reads_out_the_frames_shown_and_the_cpu_clock() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_with_overlay(d.path(), "Emerald", Some(1_512_000));
    a.note_frames(0);
    assert_eq!(a.perf_text(), None, "a number before a second was counted");
    let shown = play_a_second(&mut a, 0, 60);
    assert_eq!(a.perf_text(), Some("60.0 FPS  1512 MHz"));
    play_a_second(&mut a, shown, 45);
    assert_eq!(a.perf_text(), Some("45.0 FPS  1512 MHz"));
}

#[test]
fn half_a_second_is_not_yet_counted() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_with_overlay(d.path(), "Emerald", None);
    let t0 = a.now();
    a.note_frames(0);
    a.tick_ms(t0 + 999);
    a.note_frames(59);
    assert_eq!(a.perf_text(), None);
}

#[test]
fn a_device_that_does_not_say_its_clock_shows_the_frames_alone() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_with_overlay(d.path(), "Emerald", None);
    play_a_second(&mut a, 0, 60);
    assert_eq!(a.perf_text(), Some("60.0 FPS"));
}

#[test]
fn a_core_started_afresh_counts_from_its_own_first_frame() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_with_overlay(d.path(), "Emerald", None);
    play_a_second(&mut a, 5000, 60);
    let t0 = a.now();
    a.note_frames(10);
    a.tick_ms(t0 + 1000);
    a.note_frames(70);
    assert_eq!(
        a.perf_text(),
        Some("60.0 FPS"),
        "the new core's frames were counted against the old one's"
    );
    play_a_second(&mut a, 70, 30);
    assert_eq!(a.perf_text(), Some("30.0 FPS"));
}

#[test]
fn with_the_overlay_off_nothing_is_counted_or_drawn() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_in(d.path(), "Emerald");
    a.set_game_ready(true);
    for _ in 0..600 {
        a.update(1.0 / 60.0);
    }
    play_a_second(&mut a, 0, 60);
    assert_eq!(a.perf_text(), None);
    assert_eq!(plates(&a), 0);
}

#[test]
fn the_overlay_is_drawn_once_there_is_a_number_to_show() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_with_overlay(d.path(), "Emerald", Some(1_008_000));
    // Until the screen has come on, as it does once the core shows its first frame.
    a.set_game_ready(true);
    for _ in 0..600 {
        a.update(1.0 / 60.0);
    }
    assert_eq!(plates(&a), 0, "a plate with nothing on it yet");
    play_a_second(&mut a, 0, 60);
    // What the frontend does with the line once it has printed it.
    let face = slot_gfx::TexId::from_raw(900);
    a.set_perf_face(face, 160);
    let mut out = Vec::new();
    a.draw(&mut out);
    assert_eq!(plates(&a), 1);
    assert!(
        out.iter()
            .any(|d| matches!(d, Draw::Tex { tex, w, .. } if *tex == face && *w == 160.0)),
        "the printed line is not on the plate"
    );
}

#[test]
fn the_line_rounds_to_a_tenth_of_a_frame() {
    assert_eq!(perf_line(59.7275, Some(1512)), "59.7 FPS  1512 MHz");
    assert_eq!(perf_line(0.0, None), "0.0 FPS");
}

#[test]
fn a_pause_under_a_menu_is_not_counted_against_the_frames() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_with_overlay(d.path(), "Emerald", None);
    let shown = play_a_second(&mut a, 0, 60);
    let t0 = a.now();
    a.pause_perf();
    assert_eq!(
        a.perf_text(),
        None,
        "a number shown while the core was held"
    );
    a.tick_ms(t0 + 600);
    play_a_second(&mut a, shown, 60);
    assert_eq!(a.perf_text(), Some("60.0 FPS"));
}

#[test]
fn a_slowdown_shows_within_a_quarter_second_counted_over_the_last_second() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let mut a = app_playing_with_overlay(d.path(), "Emerald", None);
    let t0 = a.now();
    // A second at 60, a quarter at a time.
    for q in 0..=4 {
        a.tick_ms(t0 + q * 250);
        a.note_frames(q * 15);
    }
    assert_eq!(a.perf_text(), Some("60.0 FPS"));
    // In between quarters, the line stays as it was.
    a.tick_ms(t0 + 1100);
    a.note_frames(60);
    assert_eq!(a.perf_text(), Some("60.0 FPS"));
    // A quarter with no frames at all: the last second held three quarters' worth.
    a.tick_ms(t0 + 1250);
    a.note_frames(60);
    assert_eq!(a.perf_text(), Some("45.0 FPS"));
}
