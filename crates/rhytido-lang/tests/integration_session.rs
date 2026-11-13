// SPDX-License-Identifier: Apache-2.0
//! Integration tests for `rhytido-lang`: modules working together end to end.
//!
//! Layer: integration tests (AGENTS.md §4). These are the cross-module and
//! cross-crate scenarios the skeleton exists to support — the symbol table, the
//! value model and the frame stack cooperating inside a session, plus the
//! contracts ADR-11 says a session must honour: zero global state with several
//! independent instances, dynamic scope by frame stack, and atom semantics
//! reaching the language layer unchanged through `Value::Atom`.

use rhytido_backend::{BigFloat, Integer, Rational, Rounding};
use rhytido_core::message::MSG_ROUNDING_OCCURRED;
use rhytido_core::{Atom, Bound, Context as CoreContext, Range};
use rhytido_lang::{DisplayMode, Session, Settings, SymbolId, Value};

// ---- Helpers ---------------------------------------------------------------

fn integer(value: i64) -> Value {
    Value::atom(Atom::integer(Integer::from(value)))
}

fn list_of(values: impl IntoIterator<Item = Value>) -> Value {
    Value::list_from_iter(values)
}

fn as_integer(value: Option<&Value>) -> Option<Integer> {
    value
        .and_then(Value::as_atom)
        .and_then(Atom::as_integer)
        .cloned()
}

/// Applies a procedure-like call: bind the parameters in a fresh frame, run the
/// body, and hand the result back with the frame popped.
fn call(session: &mut Session, parameters: &[(SymbolId, Value)], body: impl FnOnce(&mut Session)) {
    session.with_frame(|session| {
        for (name, value) in parameters {
            session.define_current(*name, value.clone());
        }
        body(session);
    });
}

// ---- Multi-instance independence (ADR-11 §2) --------------------------------

#[test]
fn integration_two_sessions_do_not_interfere() {
    let mut first = Session::new();
    let mut second = Session::new();

    let shared_spelling = first.intern("n");
    // The same spelling in the second session is its own id, bound to nothing.
    let other_spelling = second.intern("n");
    assert_eq!(shared_spelling, other_spelling, "ids coincide per table");
    first.assign(shared_spelling, integer(1));
    second.assign(other_spelling, integer(2));

    assert_eq!(
        as_integer(first.lookup(shared_spelling)),
        Some(Integer::from(1))
    );
    assert_eq!(
        as_integer(second.lookup(other_spelling)),
        Some(Integer::from(2))
    );

    // Depth, settings and suppression are per session too.
    first.push_frame();
    first.set_settings(Settings::new().with_precision(800));
    first.catalog_mut().suppress(MSG_ROUNDING_OCCURRED.code);
    assert_eq!(first.current_depth(), 2);
    assert_eq!(second.current_depth(), 1);
    assert_eq!(second.settings().precision(), Settings::new().precision());
    assert!(!second.catalog().is_suppressed(MSG_ROUNDING_OCCURRED.code));
}

#[test]
fn integration_clone_is_an_independent_session() {
    let mut original = Session::new();
    let name = original.intern("x");
    original.assign(name, list_of([integer(1)]));
    original.set_settings(Settings::new().with_verbosity(7));

    let mut copy = original.clone();
    // The copy starts from the same state...
    assert_eq!(copy.text(name), Some("x"));
    assert_eq!(copy.lookup(name).and_then(Value::len), Some(1));
    assert_eq!(copy.settings().verbosity(), 7);

    // ...and diverges in every direction a session can be mutated.
    let longer = copy
        .lookup(name)
        .and_then(|v| v.append(integer(2)))
        .expect("list");
    copy.assign(name, longer);
    copy.push_frame();
    copy.define_current(name, integer(3));
    copy.set_settings(Settings::new().with_verbosity(0));
    copy.catalog_mut().suppress(1);

    assert_eq!(original.lookup(name).and_then(Value::len), Some(1));
    assert_eq!(original.current_depth(), 1);
    assert_eq!(original.settings().verbosity(), 7);
    assert!(!original.catalog().is_suppressed(1));
    assert_eq!(as_integer(copy.lookup(name)), Some(Integer::from(3)));
    assert_eq!(copy.current_depth(), 2);
}

#[test]
fn integration_restart_is_reconstructing_the_session() {
    // ADR-11 §5: restart rebuilds the session rather than unwinding state.
    let mut polluted = Session::new();
    let name = polluted.intern("leftover");
    polluted.assign(name, Atom::error("stuck", Some(3)).into());
    polluted.push_frame();
    polluted.push_frame();
    polluted.catalog_mut().suppress(1);
    polluted.set_settings(Settings::new().with_precision(4096));

    let restarted = Session::new();
    assert_eq!(restarted.current_depth(), 1);
    assert!(restarted.lookup(name).is_none());
    assert!(!restarted.catalog().is_suppressed(1));
    assert_eq!(restarted.settings(), Settings::new());
    assert_eq!(restarted.symbols().len(), 0);
    assert!(polluted.current_depth() > restarted.current_depth());
}

// ---- Frame stack and dynamic scope (ADR-11 §3) ------------------------------

#[test]
fn integration_push_lookup_assign_and_with_frame_together() {
    let mut session = Session::new();
    let counter = session.intern("counter");
    let local = session.intern("local");
    session.assign(counter, integer(0));

    let observed = session.with_frame(|session| {
        // A local only exists inside this frame.
        session.define_current(local, integer(10));
        // `assign` to a name the global frame owns updates the global frame.
        session.assign(counter, integer(1));
        let seen = as_integer(session.lookup(counter));
        session.push_frame();
        let depth = session.current_depth();
        // The local is still visible through the added frame.
        let still_local = as_integer(session.lookup(local));
        session.pop_frame();
        (seen, depth, still_local)
    });

    assert_eq!(
        observed,
        (Some(Integer::from(1)), 3, Some(Integer::from(10)))
    );
    assert_eq!(session.current_depth(), 1);
    assert_eq!(as_integer(session.lookup(counter)), Some(Integer::from(1)));
    assert!(session.lookup(local).is_none(), "local died with its frame");
}

#[test]
fn integration_assign_hits_the_outer_frame_not_the_current_one() {
    let mut session = Session::new();
    let name = session.intern("shared");
    session.assign(name, integer(0));

    session.push_frame();
    session.push_frame();
    // Two frames out from the binding, and neither of them binds `shared`.
    assert_eq!(session.current_depth(), 3);
    session.assign(name, integer(42));

    // The write landed in the global frame, so it outlives both inner frames.
    session.pop_frame();
    assert_eq!(as_integer(session.lookup(name)), Some(Integer::from(42)));
    session.pop_frame();
    assert_eq!(as_integer(session.lookup(name)), Some(Integer::from(42)));
    assert_eq!(session.current_depth(), 1);
}

#[test]
fn integration_same_spelling_shadows_across_frames_then_unwinds() {
    let mut session = Session::new();
    let name = session.intern("x");
    session.assign(name, integer(0));

    let mut seen = Vec::new();
    for depth in 1..=5 {
        session.push_frame();
        session.define_current(name, integer(depth));
        seen.push(as_integer(session.lookup(name)));
    }
    assert_eq!(
        seen,
        (1..=5).map(|d| Some(Integer::from(d))).collect::<Vec<_>>()
    );
    for depth in (0..5).rev() {
        session.pop_frame();
        assert_eq!(as_integer(session.lookup(name)), Some(Integer::from(depth)));
    }
    assert_eq!(session.current_depth(), 1);
}

#[test]
fn integration_procedure_application_pushes_and_pops_a_frame() {
    let mut session = Session::new();
    let parameter = session.intern("a");
    let result = session.intern("result");
    session.assign(result, Atom::void().into());

    // A call that binds its result to a global name, as a procedure would.
    call(&mut session, &[(parameter, integer(7))], |session| {
        let input = session.lookup(parameter).cloned().expect("parameter bound");
        session.assign(result, list_of([input.clone(), input]));
    });

    assert_eq!(session.current_depth(), 1);
    assert!(
        session.lookup(parameter).is_none(),
        "parameter frame popped"
    );
    let produced = session
        .lookup(result)
        .and_then(Value::as_list)
        .expect("list");
    assert_eq!(produced.len(), 2);
    assert_eq!(produced[0], integer(7));
    assert_eq!(produced[1], integer(7));
}

#[test]
fn integration_nested_calls_keep_scopes_separate() {
    let mut session = Session::new();
    let inner_name = session.intern("inner");
    let outer_name = session.intern("outer");

    let depths = session.with_frame(|session| {
        session.define_current(outer_name, integer(1));
        let inner_depth = session.with_frame(|session| {
            session.define_current(inner_name, integer(2));
            // The outer binding is visible from the inner frame.
            assert_eq!(
                as_integer(session.lookup(outer_name)),
                Some(Integer::from(1))
            );
            session.current_depth()
        });
        // The inner binding is not visible from the outer frame.
        assert!(session.lookup(inner_name).is_none());
        (session.current_depth(), inner_depth)
    });

    assert_eq!(depths, (2, 3));
    assert_eq!(session.current_depth(), 1);
    assert!(session.lookup(outer_name).is_none());
    assert!(session.lookup(inner_name).is_none());
}

#[test]
fn integration_deep_call_chain_restores_every_frame() {
    fn descend(session: &mut Session, remaining: usize, counter: SymbolId) -> usize {
        if remaining == 0 {
            return session.current_depth();
        }
        session.with_frame(|session| {
            session.define_current(counter, integer(remaining as i64));
            let deepest = descend(session, remaining - 1, counter);
            // Still seeing this frame's own binding after the inner call.
            assert_eq!(
                as_integer(session.lookup(counter)),
                Some(Integer::from(remaining as i64))
            );
            deepest
        })
    }

    let mut session = Session::new();
    let counter = session.intern("level");
    assert_eq!(descend(&mut session, 1000, counter), 1001);
    assert_eq!(session.current_depth(), 1);
    assert!(session.lookup(counter).is_none());
}

// ---- Atom semantics through Value::Atom (ADR-9/ADR-10) ----------------------

#[test]
fn integration_atom_equality_transits_unchanged_into_bindings() {
    let mut session = Session::new();
    let name = session.intern("v");

    // Precision label irrelevant, exactly as the core layer defines it.
    session.assign(name, Value::atom(Atom::number(BigFloat::from_i64(2))));
    let bound = session.lookup(name).and_then(Value::as_atom).expect("atom");
    assert_eq!(
        bound,
        &Atom::number(BigFloat::from_i64(2).with_precision(512))
    );

    // NaN equals NaN as a value, and signed zeros stay apart.
    session.assign(name, Value::atom(Atom::number(BigFloat::NAN)));
    let nan = session.lookup(name).and_then(Value::as_atom).expect("atom");
    assert_eq!(nan, &Atom::number(BigFloat::NAN));
    assert!(nan.as_number().is_some_and(BigFloat::is_nan));
    session.assign(name, Value::atom(Atom::number(-BigFloat::zero())));
    let negative_zero = session.lookup(name).and_then(Value::as_atom).expect("atom");
    assert_ne!(negative_zero, &Atom::number(BigFloat::zero()));

    // Strings compare by content, ranges and errors structurally.
    session.assign(name, Value::atom(Atom::string("text")));
    assert_eq!(
        session.lookup(name).and_then(Value::as_atom),
        Some(&Atom::string(String::from("text")))
    );
    let range = Range::new(Bound::closed(BigFloat::one()), Bound::pos_infinity());
    session.assign(name, Value::atom(Atom::range(range.clone())));
    assert_eq!(
        session.lookup(name).and_then(Value::as_atom),
        Some(&Atom::range(range))
    );
}

#[test]
fn integration_error_value_is_an_ordinary_binding() {
    // error 一等值 (AGENTS.md §3): an error sits in a frame like any value, and
    // nothing unwinds because of it.
    let mut session = Session::new();
    let name = session.intern("failure");
    let error: Value = Atom::error("cannot evaluate", Some(12)).into();
    session.assign(name, error.clone());

    assert!(session.lookup(name).is_some_and(Value::is_error));
    assert_eq!(session.lookup(name), Some(&error));
    // It nests in a list and survives a frame change without propagating.
    session.push_frame();
    session.define_current(name, list_of([error.clone(), Atom::void().into()]));
    let items = session.lookup(name).and_then(Value::as_list).expect("list");
    assert!(items[0].is_error());
    assert!(items[1].is_void());
    assert_eq!(session.current_depth(), 2);
}

#[test]
fn integration_void_is_a_bindable_value_not_an_absence() {
    let mut session = Session::new();
    let name = session.intern("nothing");
    session.assign(name, Value::default());
    assert!(session.lookup(name).is_some(), "bound to void, not unbound");
    assert!(session.lookup(name).is_some_and(Value::is_void));
    assert_eq!(session.lookup(name), Some(&Value::atom(Atom::Void)));
}

// ---- Lists as session values ------------------------------------------------

#[test]
fn integration_list_bindings_are_rebuilt_not_mutated() {
    let mut session = Session::new();
    let name = session.intern("xs");
    session.assign(name, list_of([integer(1), integer(2)]));
    let before = session.lookup(name).cloned().expect("binding");

    // Read-modify-write: the update is a rebuild, and the value that was there
    // before is still exactly what it was, for anyone still holding it.
    let after = before.append(integer(3)).expect("list");
    session.assign(name, after);
    assert_eq!(before.len(), Some(2));
    assert_eq!(session.lookup(name).and_then(Value::len), Some(3));
    assert_eq!(
        session
            .lookup(name)
            .and_then(Value::as_list)
            .map(|items| items[2].clone()),
        Some(integer(3))
    );
}

#[test]
fn integration_a_shared_list_binding_aliases_between_sessions() {
    let mut session = Session::new();
    let name = session.intern("shared");
    session.assign(name, list_of([integer(1), integer(2), integer(3)]));
    let copy = session.clone();

    let original_items = session.lookup(name).and_then(Value::as_list).expect("list");
    let copied_items = copy.lookup(name).and_then(Value::as_list).expect("list");
    assert!(std::ptr::eq(original_items.as_ptr(), copied_items.as_ptr()));
    assert_eq!(original_items.len(), 3);
}

// ---- Settings, core context and the message catalog -------------------------

#[test]
fn integration_session_settings_derive_the_core_context() {
    // The evaluation entry point hands core a context derived from the session;
    // core never sees the session (ADR-11 §2).
    let mut session = Session::new();
    session.set_settings(
        Settings::new()
            .with_precision(1024)
            .with_rounding(Rounding::Down)
            .with_taylor_recursions(2)
            .with_verbosity(4)
            .with_points(9)
            .with_display(DisplayMode::Binary),
    );

    let core: CoreContext = session.settings().into();
    assert_eq!(core.precision(), 1024);
    assert_eq!(core.rounding(), Rounding::Down);
    assert_eq!(core.taylor_recursions(), 2);
    // The language knobs stay behind, which is the point of the projection.
    assert_eq!(session.settings().verbosity(), 4);
    assert_eq!(session.settings().points(), 9);
    assert_eq!(session.settings().display(), DisplayMode::Binary);
}

#[test]
fn integration_settings_clamp_before_core_can_refuse_a_precision() {
    let mut session = Session::new();
    session.set_settings(Settings::new().with_precision(0));
    let core = session.settings().to_core_context();
    assert_eq!(core.precision(), session.settings().precision());
    assert!(core.precision() >= Settings::MIN_PRECISION);
}

#[test]
fn integration_suppression_set_is_the_core_catalog_reused() {
    let mut session = Session::new();
    let message = MSG_ROUNDING_OCCURRED;
    assert!(session.catalog().should_emit(message.code, message.level));
    assert!(session.catalog().render(&message).is_some());

    session.catalog_mut().suppress(message.code);
    assert!(!session.catalog().should_emit(message.code, message.level));
    assert!(session.catalog().render(&message).is_none());

    // Verbosity gating is the same mechanism, still per session.
    session.catalog_mut().unsuppress(message.code);
    session.catalog_mut().set_verbosity(0);
    assert!(!session.catalog().should_emit(message.code, message.level));
    let other = Session::new();
    assert!(other.catalog().should_emit(message.code, message.level));
}

// ---- Symbols across the modules ---------------------------------------------

#[test]
fn integration_symbols_are_interned_once_and_reused_everywhere() {
    let mut session = Session::new();
    let first = session.intern("reuse");
    for _ in 0..64 {
        assert_eq!(session.intern("reuse"), first);
    }
    // The same id works in the table, the frame and the stack at once.
    session.assign(first, integer(1));
    session.push_frame();
    assert_eq!(as_integer(session.lookup(first)), Some(Integer::from(1)));
    assert_eq!(session.text(first), Some("reuse"));
    assert_eq!(session.symbols().text(first), Some("reuse"));
    assert_eq!(session.symbols().len(), 1);
}

#[test]
fn integration_values_from_different_kinds_compare_as_the_core_decides() {
    // `Integer::from(2)` and `BigFloat::from_i64(2)` denote the same number but
    // stay distinct values: the variant is part of the identity (ADR-10).
    let mut session = Session::new();
    let name = session.intern("two");
    session.assign(name, integer(2));
    let as_integer_value = session.lookup(name).cloned().expect("binding");
    session.assign(name, Value::atom(Atom::number(BigFloat::from_i64(2))));
    let as_number_value = session.lookup(name).cloned().expect("binding");
    assert_ne!(as_integer_value, as_number_value);

    // A rational equal to an integer still differs by variant.
    let rational = Value::atom(Atom::rational(Rational::from_i64(2)));
    assert_ne!(rational, as_integer_value);
    assert_ne!(rational, as_number_value);
    // And the same variant is equal wherever it travels.
    assert_eq!(integer(2), as_integer_value);
    assert_eq!(
        as_number_value,
        Value::atom(Atom::number(BigFloat::from_i64(2)))
    );
}
