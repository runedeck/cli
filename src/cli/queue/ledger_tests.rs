use super::ledger::*;

fn event(id: &str, state: State, session: &str, message: &str) -> Event {
    Event {
        timestamp: "2026-10-25T08:11:10.575Z".to_string(),
        id: id.to_string(),
        state,
        sender: "claude:42e43c0b".to_string(),
        harness: "agy".to_string(),
        session: session.to_string(),
        message: message.to_string(),
    }
}

#[test]
fn a_line_round_trips_tabs_newlines_and_backslashes() {
    let original = event(
        "m-1",
        State::Posted,
        "seat\tone",
        "two\nlines \\ and\ta tab",
    );
    let line = original.line();
    assert!(!line.contains('\n'), "{line}");
    assert_eq!(line.split('\t').count(), 7, "{line}");
    assert_eq!(Event::parse(&line), Some(original));
}

#[test]
fn comments_blank_lines_and_short_lines_are_not_events() {
    assert_eq!(Event::parse("# timestamp\tid"), None);
    assert_eq!(Event::parse("   "), None);
    assert_eq!(Event::parse("2026\tm-1\tposted"), None);
    assert_eq!(
        Event::parse("2026\tm-1\tlost\ta\tb\tc\td"),
        None,
        "an unknown state"
    );
}

#[test]
fn the_last_line_of_an_id_is_its_state() {
    let posted = event("m-1", State::Posted, "seat", "hello");
    let other = event("m-2", State::Posted, "seat", "second");
    let messages = fold(&[
        posted.clone(),
        other,
        Event::change(&fold(&[posted])[0], State::Read, "later"),
    ]);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].state, State::Read);
    assert_eq!(messages[0].text, "hello", "the text of the first line");
    assert_eq!(messages[1].state, State::Posted);
}

#[test]
fn the_last_line_of_an_id_is_its_session() {
    let posted = event("m-1", State::Posted, "ouroboros", "hello");
    let queued = Event {
        state: State::Queued,
        session: "01a0abff-74d9-7390-b827-57ef47c982c2".to_string(),
        message: String::new(),
        ..posted.clone()
    };
    let messages = fold(&[posted, queued]);
    assert_eq!(messages[0].session, "01a0abff-74d9-7390-b827-57ef47c982c2");
    assert_eq!(messages[0].text, "hello");
}

#[test]
fn a_prompt_takes_only_the_queued_message_it_repeats() {
    let folder = tempfile::tempdir().expect("folder");
    let path = folder.path().join(".rune").join(FILE_NAME);
    append(
        &path,
        &[
            event("m-1", State::Posted, "seat", "  for the seat\n"),
            event("m-1", State::Queued, "seat", ""),
            event("m-2", State::Posted, "seat", "for the seat"),
        ],
    )
    .expect("append");

    assert!(
        take_prompted(&path, "seat", "   ", "later")
            .expect("take")
            .is_empty(),
        "an empty prompt takes nothing"
    );
    let taken = take_prompted(&path, "seat", "for the seat", "later").expect("take");
    assert_eq!(
        taken
            .iter()
            .map(|message| message.id.as_str())
            .collect::<Vec<_>>(),
        ["m-1"],
        "a posted message waits for the postbox reader"
    );
    assert_eq!(
        take_prompted(&path, "seat", "for the seat", "again").expect("take again"),
        Vec::new(),
        "a read message is not taken twice"
    );
}

#[test]
fn taking_marks_read_and_keeps_every_line() {
    let folder = tempfile::tempdir().expect("folder");
    let path = folder.path().join(".rune").join(FILE_NAME);
    append(
        &path,
        &[
            event("m-1", State::Posted, "seat", "for the seat"),
            event("m-2", State::Posted, "other", "for another"),
        ],
    )
    .expect("append");

    let taken = take_posted(&path, &["seat".to_string()], "later").expect("take");
    assert_eq!(taken.len(), 1);
    assert_eq!(taken[0].text, "for the seat");
    assert!(
        take_posted(&path, &["seat".to_string()], "again")
            .expect("take again")
            .is_empty(),
        "a read message is not taken twice"
    );

    let text = std::fs::read_to_string(&path).expect("ledger");
    assert!(text.starts_with("# timestamp\tid\tstate"), "{text}");
    let (events, skipped) = load(&path).expect("load");
    assert_eq!(skipped, 0);
    assert_eq!(events.len(), 3, "two posts and one read, nothing removed");
    let states: Vec<State> = fold(&events).iter().map(|message| message.state).collect();
    assert_eq!(states, [State::Read, State::Posted]);
}

#[test]
fn a_missing_ledger_is_empty_and_is_not_created_by_a_read() {
    let folder = tempfile::tempdir().expect("folder");
    let path = folder.path().join(".rune").join(FILE_NAME);
    assert_eq!(load(&path).expect("load"), (Vec::new(), 0));
    assert_eq!(
        take_posted(&path, &["seat".to_string()], "now").expect("take"),
        Vec::new()
    );
    assert!(!path.exists());
}

#[test]
fn ids_differ_and_sort_by_time() {
    let first = new_id();
    let second = new_id();
    assert_ne!(first, second);
    assert_eq!(first.len(), 21, "{first}");
    assert!(first[..12] <= second[..12]);
}
