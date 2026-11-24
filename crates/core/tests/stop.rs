//! Stopping work under way: heard at once, from any thread, and not by the work that comes after.

use cowriting_core::turn::Halt;
use std::sync::Arc;

#[test]
fn work_that_would_never_end_ends_when_stopped_and_other_work_is_let_through() {
    let stop = Arc::new(Halt::default());
    assert_eq!(pollster::block_on(stop.or(async { 7 })), Some(7));

    let from_outside = stop.clone();
    let asking = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(50));
        from_outside.ask();
    });
    assert_eq!(pollster::block_on(stop.or(std::future::pending::<u8>())), None);
    asking.join().unwrap();

    // still asked: nothing new is begun until it is cleared
    assert_eq!(pollster::block_on(stop.or(async { 7 })), None);
    stop.clear();
    assert_eq!(pollster::block_on(stop.or(async { 7 })), Some(7));
}
