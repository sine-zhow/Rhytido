// SPDX-License-Identifier: Apache-2.0
//! Module tests for `context` (src/context.rs): settings, frames, session
//! (ADR-11).
//!
//! Layer: module tests (AGENTS.md §4) — external view of the public API only.
//! The session primitives are total, so the "error" category pins what the API
//! *reports* for the cases a caller can get wrong (an unbound name, popping the
//! global frame, an id this session never interned) instead of failure returns.

use rhytido_backend::{DEFAULT_PRECISION, Integer, Rounding};
use rhytido_core::{Atom, Context as CoreContext, MIN_PRECISION};
use rhytido_lang::{DisplayMode, Frame, Session, Settings, SymbolId, SymbolTable, Value};

// ---- Helpers ---------------------------------------------------------------

fn boolean(value: bool) -> Value {
    Value::atom(Atom::boolean(value))
}

fn number(value: i64) -> Value {
    Value::atom(Atom::integer(Integer::from(value)))
}

fn flag_of(value: Option<&Value>) -> Option<bool> {
    value.and_then(Value::as_atom).and_then(Atom::as_bool)
}

fn integer_of(value: Option<&Value>) -> Option<Integer> {
    value
        .and_then(Value::as_atom)
        .and_then(Atom::as_integer)
        .cloned()
}

// ---- Normal cases: Settings -------------------------------------------------

#[test]
fn normal_settings_new_starts_from_core_defaults() {
    let settings = Settings::new();
    assert_eq!(settings.precision(), DEFAULT_PRECISION);
    assert_eq!(settings.rounding(), Rounding::Nearest);
    assert_eq!(settings.taylor_recursions(), 0);
    assert_eq!(settings.verbosity(), Settings::DEFAULT_VERBOSITY);
    assert_eq!(settings.points(), Settings::DEFAULT_POINTS);
    assert_eq!(settings.display(), DisplayMode::Readable);
    assert_eq!(settings, Settings::default());
}

#[test]
fn normal_settings_builders_change_one_knob_at_a_time() {
    let base = Settings::new();
    let precision = base.with_precision(300);
    assert_eq!(precision.precision(), 300);
    assert_eq!(precision.rounding(), base.rounding());
    assert_eq!(precision.verbosity(), base.verbosity());
    assert_eq!(base.precision(), DEFAULT_PRECISION, "builder is consuming");

    let rounded = base.with_rounding(Rounding::Down);
    assert_eq!(rounded.rounding(), Rounding::Down);
    assert_eq!(rounded.precision(), base.precision());

    let taylor = base.with_taylor_recursions(5);
    assert_eq!(taylor.taylor_recursions(), 5);
    assert_eq!(taylor.precision(), base.precision());
}

#[test]
fn normal_language_knobs_are_stored_verbatim() {
    let settings = Settings::new()
        .with_verbosity(9)
        .with_points(42)
        .with_display(DisplayMode::Full);
    assert_eq!(settings.verbosity(), 9);
    assert_eq!(settings.points(), 42);
    assert_eq!(settings.display(), DisplayMode::Full);
}

#[test]
fn normal_to_core_context_projects_only_the_numeric_half() {
    let settings = Settings::new()
        .with_precision(500)
        .with_rounding(Rounding::Up)
        .with_taylor_recursions(3)
        .with_verbosity(9)
        .with_points(42)
        .with_display(DisplayMode::Hexadecimal);
    let core: CoreContext = settings.to_core_context();
    assert_eq!(core.precision(), 500);
    assert_eq!(core.rounding(), Rounding::Up);
    assert_eq!(core.taylor_recursions(), 3);
    // The conversion trait gives the same context as the inherent method.
    assert_eq!(CoreContext::from(settings), core);
}

#[test]
fn normal_settings_is_copy_eq_and_hash() {
    let settings = Settings::new().with_precision(120);
    let copy = settings;
    assert_eq!(copy, settings);
    let changed = settings.with_precision(121);
    assert_ne!(changed, settings);
    assert_eq!(format!("{settings:?}"), format!("{copy:?}"));
}

#[test]
fn normal_display_mode_variants_are_distinct_and_default_readable() {
    let modes = [
        DisplayMode::Readable,
        DisplayMode::Full,
        DisplayMode::Hexadecimal,
        DisplayMode::Binary,
    ];
    for (index, first) in modes.iter().enumerate() {
        assert_eq!(DisplayMode::default(), DisplayMode::Readable);
        for (other, second) in modes.iter().enumerate() {
            if index == other {
                assert_eq!(first, second);
            } else {
                assert_ne!(first, second);
            }
        }
        let _ = format!("{first:?}");
    }
}

// ---- Normal cases: Frame ----------------------------------------------------

#[test]
fn normal_frame_new_is_empty_and_default_matches() {
    let frame = Frame::new();
    assert!(frame.is_empty());
    assert_eq!(frame.len(), 0);
    assert_eq!(frame.iter().count(), 0);
    assert_eq!(frame, Frame::default());
}

#[test]
fn normal_frame_set_then_get_returns_the_bound_value() {
    let name = SymbolId::new(0);
    let mut frame = Frame::new();
    frame.set(name, boolean(true));
    assert_eq!(frame.len(), 1);
    assert!(!frame.is_empty());
    assert_eq!(flag_of(frame.get(name)), Some(true));
}

#[test]
fn normal_frame_set_replaces_an_existing_binding() {
    let name = SymbolId::new(0);
    let mut frame = Frame::new();
    frame.set(name, boolean(true));
    frame.set(name, boolean(false));
    assert_eq!(frame.len(), 1, "overwrite, not duplicate");
    assert_eq!(flag_of(frame.get(name)), Some(false));
}

#[test]
fn normal_frame_iter_follows_first_definition_order() {
    let mut frame = Frame::new();
    let a = SymbolId::new(0);
    let b = SymbolId::new(1);
    frame.set(a, number(1));
    frame.set(b, number(2));
    // Redefining `a` must not move it behind `b`.
    frame.set(a, number(3));
    let order: Vec<SymbolId> = frame.iter().map(|(name, _)| *name).collect();
    assert_eq!(order, vec![a, b]);
    assert_eq!(frame.len(), 2);
}

#[test]
fn normal_equal_frames_compare_equal_and_hash_alike() {
    let name = SymbolId::new(7);
    let mut first = Frame::new();
    let mut second = Frame::new();
    first.set(name, boolean(true));
    second.set(name, boolean(true));
    assert_eq!(first, second);
    let mut different = Frame::new();
    different.set(name, boolean(false));
    assert_ne!(first, different);
    let _ = format!("{first:?}");
}

// ---- Normal cases: Session --------------------------------------------------

#[test]
fn normal_session_new_has_exactly_one_global_frame() {
    let session = Session::new();
    assert_eq!(session.current_depth(), 1);
    assert_eq!(session.settings(), Settings::new());
    assert_eq!(session.symbols().len(), 0);
    assert!(session.lookup(SymbolId::new(0)).is_none());
    assert_eq!(session.current_depth(), Session::default().current_depth());
}

#[test]
fn normal_intern_and_text_transit_the_symbol_table() {
    let mut session = Session::new();
    let name = session.intern("x");
    assert_eq!(session.text(name), Some("x"));
    assert_eq!(session.intern("x"), name, "same text, same id");
    assert_eq!(session.symbols().len(), 1);
    assert_eq!(session.symbols().text(name), Some("x"));
}

#[test]
fn normal_assign_then_lookup_in_the_global_frame() {
    let mut session = Session::new();
    let name = session.intern("x");
    session.assign(name, boolean(true));
    assert_eq!(flag_of(session.lookup(name)), Some(true));
    assert_eq!(flag_of(session.lookup_current(name)), Some(true));
}

#[test]
fn normal_lookup_of_an_unbound_name_is_none() {
    let mut session = Session::new();
    let name = session.intern("never_bound");
    assert!(session.lookup(name).is_none());
    assert!(session.lookup_current(name).is_none());
    session.assign(name, number(0));
    // Bound now, but a view of the wrong kind still reads as absent.
    assert_eq!(
        session
            .lookup(name)
            .and_then(Value::as_atom)
            .and_then(Atom::as_bool),
        None
    );
}

#[test]
fn normal_assign_of_a_new_name_defines_it_in_the_current_frame() {
    let mut session = Session::new();
    let name = session.intern("fresh");
    session.push_frame();
    session.assign(name, boolean(true));
    assert_eq!(flag_of(session.lookup(name)), Some(true));
    assert_eq!(session.current_depth(), 2);
    assert!(session.pop_frame());
    // The binding lived in the popped frame, so it is gone with it.
    assert!(session.lookup(name).is_none());
}

#[test]
fn normal_define_current_shadows_instead_of_reaching_out() {
    let mut session = Session::new();
    let name = session.intern("shadowed");
    session.assign(name, boolean(true));
    session.push_frame();
    session.define_current(name, boolean(false));
    assert_eq!(flag_of(session.lookup(name)), Some(false));
    session.pop_frame();
    assert_eq!(flag_of(session.lookup(name)), Some(true));
}

#[test]
fn normal_lookup_prefers_the_innermost_binding() {
    let mut session = Session::new();
    let name = session.intern("depth");
    session.assign(name, number(0));
    session.push_frame();
    session.define_current(name, number(1));
    session.push_frame();
    session.define_current(name, number(2));
    assert_eq!(integer_of(session.lookup(name)), Some(Integer::from(2)));
    session.pop_frame();
    assert_eq!(integer_of(session.lookup(name)), Some(Integer::from(1)));
    session.pop_frame();
    assert_eq!(integer_of(session.lookup(name)), Some(Integer::from(0)));
}

#[test]
fn normal_with_frame_returns_the_closure_result_and_restores_depth() {
    let mut session = Session::new();
    let name = session.intern("body");
    let returned = session.with_frame(|session| {
        session.define_current(name, boolean(true));
        session.current_depth()
    });
    assert_eq!(returned, 2);
    assert_eq!(session.current_depth(), 1);
    assert!(
        session.lookup(name).is_none(),
        "frame discarded with its binding"
    );
}

#[test]
fn normal_with_frame_nesting_tracks_depth() {
    let mut session = Session::new();
    let outer_depth = session.with_frame(|session| {
        let inner_depth = session.with_frame(|session| session.current_depth());
        assert_eq!(inner_depth, 3);
        session.current_depth()
    });
    assert_eq!(outer_depth, 2);
    assert_eq!(session.current_depth(), 1);
}

#[test]
fn normal_push_and_pop_report_what_they_did() {
    let mut session = Session::new();
    assert_eq!(session.current_depth(), 1);
    session.push_frame();
    assert_eq!(session.current_depth(), 2);
    assert!(session.pop_frame());
    assert_eq!(session.current_depth(), 1);
}

#[test]
fn normal_settings_accessor_round_trips() {
    let mut session = Session::new();
    let settings = Settings::new().with_precision(240).with_verbosity(4);
    session.set_settings(settings);
    assert_eq!(session.settings(), settings);
    assert_eq!(session.settings().precision(), 240);
    assert_eq!(session.settings().to_core_context().precision(), 240);
}

#[test]
fn normal_clone_has_its_own_settings() {
    let mut session = Session::new();
    session.set_settings(Settings::new().with_precision(400));
    let mut copy = session.clone();
    copy.set_settings(Settings::new().with_precision(100));
    assert_eq!(session.settings().precision(), 400);
    assert_eq!(copy.settings().precision(), 100);
}

// ---- Extreme cases ----------------------------------------------------------

#[test]
fn extreme_precision_below_the_minimum_is_clamped() {
    for precision in [0usize, 1, MIN_PRECISION - 1] {
        let settings = Settings::new().with_precision(precision);
        assert_eq!(settings.precision(), MIN_PRECISION);
        assert_eq!(settings.to_core_context().precision(), MIN_PRECISION);
    }
    assert_eq!(
        Settings::new().with_precision(MIN_PRECISION).precision(),
        MIN_PRECISION
    );
}

#[test]
fn extreme_largest_knob_values_are_stored_without_failing() {
    let settings = Settings::new()
        .with_precision(usize::MAX)
        .with_taylor_recursions(u32::MAX)
        .with_verbosity(u8::MAX)
        .with_points(u32::MAX);
    assert_eq!(settings.precision(), usize::MAX);
    assert_eq!(settings.taylor_recursions(), u32::MAX);
    assert_eq!(settings.verbosity(), u8::MAX);
    assert_eq!(settings.points(), u32::MAX);
    // Deriving a core context stays total at every extreme.
    let core = settings.to_core_context();
    assert_eq!(core.precision(), usize::MAX);
    assert_eq!(core.taylor_recursions(), u32::MAX);
}

#[test]
fn extreme_thousand_frame_stack_resolves_innermost_first() {
    let mut session = Session::new();
    let name = session.intern("deep");
    session.assign(name, number(0));
    for depth in 0..1000 {
        session.push_frame();
        if depth == 500 {
            session.define_current(name, number(500));
        }
    }
    assert_eq!(session.current_depth(), 1001);
    assert_eq!(integer_of(session.lookup(name)), Some(Integer::from(500)));
    // Unwinding past the mid binding reaches the global one again.
    for _ in 0..500 {
        assert!(session.pop_frame());
    }
    assert_eq!(session.current_depth(), 501);
    assert_eq!(integer_of(session.lookup(name)), Some(Integer::from(0)));
}

#[test]
fn extreme_wide_frame_keeps_every_binding_reachable() {
    let mut table = SymbolTable::new();
    let mut frame = Frame::new();
    for index in 0..1000 {
        let name = table.intern(&format!("binding_{index}"));
        frame.set(name, number(i64::from(index)));
    }
    assert_eq!(frame.len(), 1000);
    for index in 0..1000 {
        let name = table.intern(&format!("binding_{index}"));
        let value = frame.get(name);
        assert_eq!(integer_of(value), Some(Integer::from(i64::from(index))));
    }
}

#[test]
fn extreme_unbound_names_stay_unbound_across_a_thousand_frames() {
    let mut session = Session::new();
    let name = session.intern("absent");
    for _ in 0..1000 {
        session.push_frame();
    }
    assert!(session.lookup(name).is_none());
    assert!(session.lookup_current(name).is_none());
    assert_eq!(session.current_depth(), 1001);
}

#[test]
fn extreme_with_frame_one_thousand_deep_returns_and_restores() {
    fn recurse(session: &mut Session, remaining: usize) -> usize {
        if remaining == 0 {
            return session.current_depth();
        }
        session.with_frame(|session| recurse(session, remaining - 1))
    }
    let mut session = Session::new();
    assert_eq!(recurse(&mut session, 1000), 1001);
    assert_eq!(session.current_depth(), 1);
}

// ---- Error cases ------------------------------------------------------------

#[test]
fn error_popping_the_global_frame_is_reported_and_changes_nothing() {
    let mut session = Session::new();
    let name = session.intern("global");
    session.assign(name, boolean(true));
    for _ in 0..4 {
        assert!(!session.pop_frame(), "the global frame is not a target");
        assert_eq!(session.current_depth(), 1);
        assert_eq!(flag_of(session.lookup(name)), Some(true));
    }
}

#[test]
fn error_lookup_of_a_name_interned_elsewhere_is_none() {
    let mut session = Session::new();
    let mut other = Session::new();
    let foreign = other.intern("elsewhere");
    // The id may coincidentally be valid here, but it names nothing bound.
    assert!(session.lookup(foreign).is_none());
    let mine = session.intern("mine");
    session.assign(mine, boolean(true));
    assert!(other.lookup(mine).is_none());
}

#[test]
fn error_text_of_an_uninterned_id_is_none() {
    let mut session = Session::new();
    session.intern("only");
    assert_eq!(session.text(SymbolId::new(0)), Some("only"));
    assert_eq!(session.text(SymbolId::new(9)), None);
    assert_eq!(session.text(SymbolId::new(u32::MAX)), None);
}

#[test]
fn error_binding_dropped_with_its_frame_is_not_reachable() {
    let mut session = Session::new();
    let inner_name = session.intern("inner");
    let outer_name = session.intern("outer");
    session.assign(outer_name, boolean(true));
    let depth_inside = session.with_frame(|session| {
        session.define_current(inner_name, boolean(false));
        session.current_depth()
    });
    assert_eq!(depth_inside, 2);
    assert!(session.lookup(inner_name).is_none());
    assert_eq!(flag_of(session.lookup(outer_name)), Some(true));
}

#[test]
fn error_precision_zero_never_reaches_the_core_context() {
    // A zero precision would be refused by the numeric layer; the settings
    // clamp is what keeps a session from ever handing it down.
    let settings = Settings::new().with_precision(0);
    assert!(settings.precision() >= MIN_PRECISION);
    assert!(settings.to_core_context().precision() >= MIN_PRECISION);
}

// ---- Robustness -------------------------------------------------------------

#[test]
fn robustness_with_frame_pops_on_an_early_error_return() {
    let mut session = Session::new();
    let name = session.intern("may_fail");
    let outcome: Result<usize, &'static str> = session.with_frame(|session| {
        session.define_current(name, boolean(true));
        // The `?` exit path must leave the stack as it was found (ADR-11 §4).
        let stop: Result<usize, &'static str> = Err("stop");
        stop?;
        Ok(session.current_depth())
    });
    assert_eq!(outcome, Err("stop"));
    assert_eq!(session.current_depth(), 1);
    assert!(session.lookup(name).is_none());
}

#[test]
fn robustness_with_frame_pops_on_an_option_shortcut() {
    let mut session = Session::new();
    let found = session.with_frame(|session| {
        let name = session.intern("optional");
        session.define_current(name, boolean(true));
        session
            .lookup(name)
            .and_then(Value::as_atom)
            .and_then(Atom::as_bool)
    });
    assert_eq!(found, Some(true));
    assert_eq!(session.current_depth(), 1);
}

#[test]
fn robustness_panic_in_a_body_propagates_and_leaves_the_stack_as_documented() {
    let mut session = Session::new();
    let name = session.intern("panicky");
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        session.with_frame(|session| {
            session.define_current(name, boolean(true));
            panic!("body exploded");
        })
    }));
    assert!(caught.is_err(), "the panic must reach the caller unchanged");
    // `with_frame` documents that unwinding does not pop: containing panics is
    // the CLI boundary's job (M3-E), which rebuilds the session instead.
    assert_eq!(session.current_depth(), 2);
    // The global frame survives, so the session is still usable.
    assert!(session.pop_frame());
    assert_eq!(session.current_depth(), 1);
    assert!(session.lookup(name).is_none());
}

#[test]
fn robustness_clone_shares_values_but_not_bindings() {
    let mut session = Session::new();
    let name = session.intern("shared_list");
    let bound = Value::list_from_iter([boolean(true)]);
    session.assign(name, bound.clone());
    let mut copy = session.clone();
    let original_list = session.lookup(name).and_then(Value::as_list).expect("list");
    let copied_list = copy.lookup(name).and_then(Value::as_list).expect("list");
    // The element buffer aliases: cloning a session is not a deep copy.
    assert!(std::ptr::eq(original_list.as_ptr(), copied_list.as_ptr()));
    // Rebuilding in the copy leaves the original binding untouched.
    let longer = bound.append(boolean(false)).expect("list");
    copy.assign(name, longer);
    assert_eq!(session.lookup(name).and_then(Value::len), Some(1));
    assert_eq!(copy.lookup(name).and_then(Value::len), Some(2));
}

#[test]
fn robustness_suppression_catalog_is_per_session() {
    let session = Session::new();
    let mut copy = session.clone();
    assert!(!session.catalog().is_suppressed(1));
    copy.catalog_mut().suppress(1);
    assert!(copy.catalog().is_suppressed(1));
    assert!(!session.catalog().is_suppressed(1));
    assert_eq!(session.catalog().verbosity(), copy.catalog().verbosity());
}

#[test]
fn robustness_repeated_intern_and_frames_converge() {
    let mut session = Session::new();
    let mut names = Vec::new();
    for round in 0..24 {
        let name = session.intern(&format!("name_{round}"));
        names.push(name);
        session.with_frame(|session| {
            for existing in &names {
                session.define_current(*existing, number(1));
            }
        });
        assert_eq!(session.current_depth(), 1);
        for existing in &names {
            assert!(session.lookup(*existing).is_none());
        }
    }
    assert_eq!(session.symbols().len(), 24);
    assert_eq!(session.intern("name_0"), names[0]);
}
