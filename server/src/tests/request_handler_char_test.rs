// Building a `Request` needs a live socket and a session, so the handlers' decisions live in pure
// functions that are tested here.
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use configuration::configuration::CityConfig;

use crate::server::model::events::game_event::{CharacterRemoveFromMap, GameEvent};
use crate::server::request_handler::char::{
    delete_reserve_ack, disconnect_events, leave_game_events, pick_starting_map, DeleteReserveAck,
};

const CHAR_ID: u32 = 150000;

#[test]
fn disconnect_removes_character_from_map_then_leaves_game() {
    let events = disconnect_events(Some(CHAR_ID), |char_id| {
        assert_eq!(char_id, CHAR_ID);
        Some(("prontera".to_string(), 0))
    });

    assert_eq!(
        events,
        vec![
            (
                GameEvent::CharacterRemoveFromMap(CharacterRemoveFromMap {
                    char_id: CHAR_ID,
                    map_name: "prontera".to_string(),
                    instance_id: 0,
                }),
                1
            ),
            (GameEvent::CharacterLeaveGame((CHAR_ID, 0)), 2),
        ]
    );
}

#[test]
fn disconnect_without_selected_character_queues_nothing() {
    let events = disconnect_events(None, |_| panic!("must not look up a character when the session has none"));

    assert!(events.is_empty());
}

#[test]
fn disconnect_with_character_missing_from_state_queues_nothing() {
    let events = disconnect_events(Some(CHAR_ID), |_| None);

    assert!(events.is_empty());
}

#[test]
fn restart_and_disconnect_share_the_leave_order() {
    let events = leave_game_events(CHAR_ID, "geffen".to_string(), 1, 1);

    assert!(matches!(events[0], (GameEvent::CharacterRemoveFromMap(_), 1)));
    assert_eq!(events[1], (GameEvent::CharacterLeaveGame((CHAR_ID, 1)), 2));
}

#[test]
fn delete_reserve_ack_sends_absolute_delete_date() {
    let now = UNIX_EPOCH + Duration::from_secs(1_700_000_000);

    let ack = delete_reserve_ack(CHAR_ID, true, now);

    assert_eq!(
        ack,
        DeleteReserveAck {
            gid: CHAR_ID,
            result: 1,
            delete_reserved_date: 1_700_086_400,
        }
    );
}

#[test]
fn delete_reserve_ack_reports_db_error_instead_of_success() {
    let ack = delete_reserve_ack(CHAR_ID, false, SystemTime::now());

    // The client opens the delete dialog for result 0 or 1 and shows "A database error occurred." for 3.
    assert_eq!(ack.result, 3);
    assert_eq!(ack.delete_reserved_date, 0);
}

fn city(name: &str, x: u16, y: u16) -> CityConfig {
    CityConfig { name: name.to_string(), x, y }
}

fn configured_starting_maps() -> Vec<CityConfig> {
    vec![city("tanthu", 32, 32), city("new_1-1", 53, 111), city("prontera", 156, 191)]
}

#[test]
fn starting_map_falls_back_to_tanthu_when_none_configured() {
    assert_eq!(pick_starting_map(&[], None), ("tanthu".to_string(), 32, 32));
    assert_eq!(pick_starting_map(&[], Some(3)), ("tanthu".to_string(), 32, 32));
}

#[test]
fn starting_map_defaults_to_first_entry_without_or_with_out_of_range_selection() {
    let maps = configured_starting_maps();

    assert_eq!(pick_starting_map(&maps, None), ("tanthu".to_string(), 32, 32));
    assert_eq!(pick_starting_map(&maps, Some(99)), ("tanthu".to_string(), 32, 32));
}

#[test]
fn starting_map_honours_a_valid_selected_index() {
    let maps = configured_starting_maps();

    assert_eq!(pick_starting_map(&maps, Some(0)), ("tanthu".to_string(), 32, 32));
    assert_eq!(pick_starting_map(&maps, Some(1)), ("new_1-1".to_string(), 53, 111));
    assert_eq!(pick_starting_map(&maps, Some(2)), ("prontera".to_string(), 156, 191));
}
