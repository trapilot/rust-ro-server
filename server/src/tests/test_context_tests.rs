#[cfg(test)]
mod tests {
    use std::thread;
    use std::time::Duration;

    use crate::server::model::events::persistence_event::{PersistenceEvent, SavePositionUpdate};
    use crate::tests::common::sync_helper::CountDownLatch;
    use crate::tests::common::{TestContext, create_mpsc};

    fn test_context() -> TestContext {
        let (client_notification_sender, client_notification_receiver) = create_mpsc();
        let (persistence_event_sender, persistence_event_receiver) = create_mpsc();
        TestContext::new(
            client_notification_sender,
            client_notification_receiver,
            persistence_event_sender,
            persistence_event_receiver,
            CountDownLatch::new(0),
        )
    }

    fn save_position(x: u16) -> PersistenceEvent {
        PersistenceEvent::SaveCharacterPosition(SavePositionUpdate {
            char_id: 150000,
            account_id: 2000000,
            map_name: "geffen.gat".to_string(),
            x,
            y: 120,
        })
    }

    #[test]
    fn test_wait_for_persistence_event_should_accept_event_received_before_waiting() {
        // Given
        let context = test_context();
        context.received_persistence_events().lock().unwrap().push(save_position(120));
        // When
        // A zero timeout proves the first check happens before the deadline check.
        context.wait_for_persistence_event(&save_position(120), Duration::ZERO);
        // Then returns without panicking
    }

    #[test]
    fn test_wait_for_persistence_event_should_accept_event_arriving_after_previous_200ms_latch() {
        // Given
        let context = test_context();
        let sender = context.persistence_event_sender();
        // When
        // 300 ms is past the old 200 ms latch, which is the delay that made the test
        // flaky.
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            sender.send(save_position(120)).unwrap();
        });
        // Then
        context.wait_for_persistence_event(&save_position(120), Duration::from_secs(2));
    }

    #[test]
    #[should_panic(expected = "not received within")]
    fn test_wait_for_persistence_event_should_fail_when_only_a_different_event_is_received() {
        // Given
        let context = test_context();
        context.received_persistence_events().lock().unwrap().push(save_position(121));
        // When
        context.wait_for_persistence_event(&save_position(120), Duration::from_millis(50));
    }

    #[test]
    #[should_panic(expected = "received: []")]
    fn test_wait_for_persistence_event_should_fail_with_received_events_when_nothing_arrives() {
        // Given
        let context = test_context();
        // When
        context.wait_for_persistence_event(&save_position(120), Duration::from_millis(50));
    }
}
