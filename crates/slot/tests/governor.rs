mod common;

use common::{app_booting_with_governors, tmp_root_with_carts};
use slot::app::{full_clock_for, governor_for};
use slot_store::{Core, Platform};

#[test]
fn the_device_boots_on_ondemand() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (_a, governors) = app_booting_with_governors(d.path());
    assert_eq!(*governors.lock().unwrap(), ["ondemand"]);
}

#[test]
fn a_gba_game_runs_on_performance_and_hands_back_ondemand_when_it_stops() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, governors) = app_booting_with_governors(d.path());
    a.set_core(Core::Mgba);
    a.set_platform(Platform::Gba);
    a.set_core_running(true);
    a.set_core_running(true);
    a.set_core_running(false);
    a.set_core_running(false);
    a.set_core_running(true);
    assert_eq!(
        *governors.lock().unwrap(),
        ["ondemand", "performance", "ondemand", "performance"],
        "the governor was written again with nothing changed, or not at all"
    );
}

#[test]
fn gpsp_runs_its_gba_games_on_performance_too() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, governors) = app_booting_with_governors(d.path());
    a.set_core(Core::Gpsp);
    a.set_platform(Platform::Gba);
    a.set_core_running(true);
    assert_eq!(*governors.lock().unwrap(), ["ondemand", "performance"]);
}

#[test]
fn game_boy_games_stay_on_ondemand() {
    let d = tmp_root_with_carts(&["Emerald"]);
    let (mut a, governors) = app_booting_with_governors(d.path());
    for platform in [Platform::Gb, Platform::Gbc] {
        a.set_core(Core::Mgba);
        a.set_platform(platform);
        a.set_core_running(true);
        a.set_core_running(false);
    }
    assert_eq!(*governors.lock().unwrap(), ["ondemand"]);
}

#[test]
fn only_gba_games_get_the_full_clock() {
    assert!(full_clock_for(Platform::Gba));
    assert!(!full_clock_for(Platform::Gb));
    assert!(!full_clock_for(Platform::Gbc));
    assert_eq!(governor_for(true), "performance");
    assert_eq!(governor_for(false), "ondemand");
}
