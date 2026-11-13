// SPDX-License-Identifier: Apache-2.0
//! Session state: settings, message suppression, symbols, and the dynamic-scope
//! frame stack (ADR-11).
//!
//! [`Session`] is the single carrier of *language* state: the settings that
//! steer evaluation, the suppression set that steers diagnostics, the
//! identifiers interned so far, and the stack of scopes that gives the language
//! its dynamic scope. It is a value, not a global — every operation threads
//! `&mut Session` explicitly, so any number of sessions coexist, each with its
//! own frame stack and its own symbol table (AGENTS.md §3, 零全局状态; ADR-11
//! §2). That is also what makes them testable: two sessions that must not
//! interfere are two `Session::new()` calls, not a save/restore dance around
//! process state.
//!
//! ## Frame stack, not a scope tree
//!
//! Scope is a `Vec<Frame>` with the global frame at the bottom, never removed.
//! A procedure application pushes a frame on entry and pops it on exit, so a
//! name resolves to the *innermost* frame that binds it — dynamic scope, a
//! semantic contract the architecture forbids optimizing away (AGENTS.md §3).
//! The three rules ADR-11 fixes are implemented literally:
//!
//! - [`Session::lookup`] — innermost frame first, walking outward; an unbound
//!   name yields `None` (the symbol-fallback contract itself is the evaluator's
//!   to pin, M3-C).
//! - [`Session::assign`] — write the innermost frame that *already* binds the
//!   name; if none does, define it in the current frame. So an inner
//!   assignment to an outer binding updates the outer frame rather than
//!   shadowing it.
//! - [`Session::with_frame`] — a scope guard: push a frame, run the closure,
//!   pop on every path the closure returns by.
//!
//! ## Settings
//!
//! [`Settings`] holds the numeric knobs derived from `rhytido_core::Context`
//! (precision, rounding, Taylor recursions) plus the language knobs
//! ([`Settings::verbosity`], [`Settings::points`], [`Settings::display`]). It
//! converts *down* into a core context via [`Settings::to_core_context`] so an
//! evaluation entry point can hand core its settings without core learning that
//! a language layer exists (ADR-11 §2).
//!
//! # Examples
//!
//! ```
//! use rhytido_core::Atom;
//! use rhytido_lang::{Session, Value};
//!
//! let mut session = Session::new();
//! let x = session.intern("x");
//!
//! // One global frame, empty: nothing is bound yet.
//! assert_eq!(session.current_depth(), 1);
//! assert!(session.lookup(x).is_none());
//!
//! session.assign(x, Value::atom(Atom::boolean(true)));
//!
//! // `assign` writes the innermost frame that *already* binds the name, so an
//! // assignment made inside a procedure updates the global binding...
//! session.with_frame(|session| session.assign(x, Value::atom(Atom::boolean(false))));
//! let bound = |session: &Session| {
//!     session
//!         .lookup(x)
//!         .and_then(Value::as_atom)
//!         .and_then(Atom::as_bool)
//! };
//! assert_eq!(bound(&session), Some(false));
//!
//! // ...while `define_current` introduces a shadow that vanishes with its
//! // frame, leaving the outer binding untouched.
//! session.with_frame(|session| session.define_current(x, Value::atom(Atom::boolean(true))));
//! assert_eq!(bound(&session), Some(false));
//! assert_eq!(session.current_depth(), 1);
//! ```

use rhytido_backend::Rounding;
use rhytido_core::{Context as CoreContext, MessageCatalog};

use crate::symbol::{SymbolId, SymbolTable};
use crate::value::Value;

/// How values are to be rendered.
///
/// A placeholder: this layer only records the *setting*, and no engine code
/// reads it yet. Which keywords select which mode, and what each mode does to
/// output, is presentation behaviour settled when the settings commands are
/// distilled from the reference (M3-E); ADR-9 keeps byte-level rendering out of
/// the engine entirely (AGENTS.md §3, 语义/表现分离).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum DisplayMode {
    /// The default rendering, suitable for reading back into the language.
    #[default]
    Readable,
    /// Every digit held by the value.
    Full,
    /// A hexadecimal rendering of the binary representation.
    Hexadecimal,
    /// A binary rendering of the significand.
    Binary,
}

/// Tunable settings carried by a [`Session`].
///
/// The three numeric fields mirror `rhytido_core::Context` and reach it through
/// [`Settings::to_core_context`]; the rest are language-level knobs that the
/// evaluator and the presentation layer will consume once the reference
/// behaviour assigns them meaning (M3-E).
///
/// Like the core context it feeds, `Settings` is `Copy` and uses consuming
/// builders, so changing one knob is an expression rather than a statement.
///
/// # Examples
///
/// ```
/// use rhytido_backend::Rounding;
/// use rhytido_core::Context;
/// use rhytido_lang::{DisplayMode, Settings};
///
/// let settings = Settings::new()
///     .with_precision(200)
///     .with_rounding(Rounding::Up)
///     .with_display(DisplayMode::Full);
/// assert_eq!(settings.precision(), 200);
/// assert_eq!(settings.display(), DisplayMode::Full);
///
/// // Only the numeric half crosses into core; the language knobs stay here.
/// let core: Context = settings.to_core_context();
/// assert_eq!(core.precision(), 200);
/// assert_eq!(core.rounding(), Rounding::Up);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Settings {
    /// Working precision in bits, as `rhytido_core::Context` understands it.
    ///
    /// The setter clamps values below [`Self::MIN_PRECISION`] up to it, exactly
    /// mirroring the core context, so the language can never ask core for a
    /// precision the numeric layer itself would refuse. The meaning of a
    /// particular precision is a numeric concern, not a language one.
    precision: usize,
    /// Rounding mode handed to every core evaluation.
    ///
    /// Stored as given; see `rhytido_core::Context::with_rounding`.
    rounding: Rounding,
    /// Taylor recursion setting for the interval refiner.
    ///
    /// Interpreted by core exactly as `Context::with_taylor_recursions`
    /// documents it (the mean-value rule applies `k + 1` times).
    taylor_recursions: u32,
    /// Verbosity level.
    ///
    /// A raw level: which diagnostics a given number admits is fixed by the
    /// reference behaviour of the settings commands (M3-E). Nothing filters
    /// messages on it yet, and `Default` picks `1`, the ordinary level.
    verbosity: u8,
    /// Number of points shown for a function or list.
    ///
    /// A placeholder count with no consumer yet, for the same reason as
    /// [`Settings::verbosity`]. `Default` picks `0`, meaning "unset" rather
    /// than "show none": deciding that is M3-E's job.
    points: u32,
    /// How values are to be rendered.
    ///
    /// See [`DisplayMode`]; not consulted by this layer.
    display: DisplayMode,
}

impl Settings {
    /// The smallest working precision, in bits, shared with the core context.
    pub const MIN_PRECISION: usize = rhytido_core::MIN_PRECISION;

    /// The default verbosity level (`1`).
    pub const DEFAULT_VERBOSITY: u8 = 1;

    /// The default value of [`Settings::points`] (`0`, i.e. unset).
    pub const DEFAULT_POINTS: u32 = 0;

    /// Creates settings with the core defaults for the numeric knobs, verbosity
    /// [`Settings::DEFAULT_VERBOSITY`], `points` [`Settings::DEFAULT_POINTS`],
    /// and [`DisplayMode::Readable`].
    ///
    /// # Panics
    ///
    /// Never.
    pub fn new() -> Self {
        let core = CoreContext::new();
        Self {
            precision: core.precision(),
            rounding: core.rounding(),
            taylor_recursions: core.taylor_recursions(),
            verbosity: Self::DEFAULT_VERBOSITY,
            points: Self::DEFAULT_POINTS,
            display: DisplayMode::default(),
        }
    }

    /// Returns settings with the working precision set to `precision` bits,
    /// clamped up to [`Settings::MIN_PRECISION`] as the core context clamps.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn with_precision(mut self, precision: usize) -> Self {
        self.precision = precision.max(Self::MIN_PRECISION);
        self
    }

    /// Returns settings with the rounding mode set to `rounding`.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn with_rounding(mut self, rounding: Rounding) -> Self {
        self.rounding = rounding;
        self
    }

    /// Returns settings with the Taylor recursion setting set to `k`.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn with_taylor_recursions(mut self, k: u32) -> Self {
        self.taylor_recursions = k;
        self
    }

    /// Returns settings with the verbosity level set to `verbosity`.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn with_verbosity(mut self, verbosity: u8) -> Self {
        self.verbosity = verbosity;
        self
    }

    /// Returns settings with the point count set to `points`.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn with_points(mut self, points: u32) -> Self {
        self.points = points;
        self
    }

    /// Returns settings with the display mode set to `display`.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn with_display(mut self, display: DisplayMode) -> Self {
        self.display = display;
        self
    }

    /// Returns the working precision in bits.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn precision(&self) -> usize {
        self.precision
    }

    /// Returns the rounding mode.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn rounding(&self) -> Rounding {
        self.rounding
    }

    /// Returns the Taylor recursion setting.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn taylor_recursions(&self) -> u32 {
        self.taylor_recursions
    }

    /// Returns the verbosity level.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn verbosity(&self) -> u8 {
        self.verbosity
    }

    /// Returns the point count.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn points(&self) -> u32 {
        self.points
    }

    /// Returns the display mode.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn display(&self) -> DisplayMode {
        self.display
    }

    /// Derives the core evaluation context carrying the numeric settings.
    ///
    /// The language knobs have no core counterpart, which is exactly the point
    /// of ADR-11 §2: the entry point to evaluation hands this value down, and
    /// core never sees a session.
    ///
    /// # Panics
    ///
    /// Never. The core setters are total (precision clamps, the others store
    /// verbatim).
    pub fn to_core_context(self) -> CoreContext {
        CoreContext::new()
            .with_precision(self.precision)
            .with_rounding(self.rounding)
            .with_taylor_recursions(self.taylor_recursions)
    }
}

impl Default for Settings {
    /// The default settings (see [`Settings::new`]).
    fn default() -> Self {
        Self::new()
    }
}

impl From<Settings> for CoreContext {
    /// Derives the core context from the numeric half of `settings`.
    ///
    /// Equivalent to [`Settings::to_core_context`].
    fn from(settings: Settings) -> Self {
        settings.to_core_context()
    }
}

/// One dynamic scope: the bindings introduced by one procedure application (or,
/// at the bottom of the stack, by the session itself).
///
/// A linear `Vec` of name/value pairs, scanned on lookup. The choice is
/// deliberate (ADR-11 §1): a frame holds a handful of names, so a hash table
/// would buy nothing and would make iteration order unpredictable, which is a
/// behaviour difference waiting to leak into printing. Optimizing this table is
/// a job for a benchmark that proves it hot, and not before (先正确后快).
///
/// A name is bound at most once per frame: [`Frame::set`] overwrites, so
/// `assign`ing twice in one frame leaves one entry and no stale duplicate a
/// later lookup could disagree about.
///
/// The fields are private on purpose — mutating a frame from outside its
/// session could break the invariant that a session always owns at least one
/// frame (AGENTS.md §2.4 C-STRUCT-PRIVATE).
///
/// # Examples
///
/// ```
/// use rhytido_core::Atom;
/// use rhytido_lang::{Frame, SymbolId, Value};
///
/// let name = SymbolId::new(0);
/// let mut frame = Frame::new();
/// assert!(frame.is_empty());
/// frame.set(name, Value::atom(Atom::boolean(true)));
/// frame.set(name, Value::atom(Atom::boolean(false)));
/// // Overwrite, not duplicate.
/// assert_eq!(frame.len(), 1);
/// let flag = frame.get(name).and_then(Value::as_atom).and_then(Atom::as_bool);
/// assert_eq!(flag, Some(false));
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Frame {
    bindings: Vec<(SymbolId, Value)>,
}

impl Frame {
    /// Creates an empty frame.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn new() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }

    /// Borrows the value bound to `name` in this frame, or `None`.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn get(&self, name: SymbolId) -> Option<&Value> {
        self.bindings
            .iter()
            .rev()
            .find(|(bound, _)| *bound == name)
            .map(|(_, value)| value)
    }

    /// Binds `name` to `value`, replacing any binding of that name already in
    /// this frame.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn set(&mut self, name: SymbolId, value: Value) {
        if let Some(entry) = self
            .bindings
            .iter_mut()
            .rev()
            .find(|(bound, _)| *bound == name)
        {
            entry.1 = value;
        } else {
            self.bindings.push((name, value));
        }
    }

    /// The number of distinct names bound in this frame.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Returns `true` if this frame binds no names.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Iterates the bindings in the order they were first introduced.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn iter(&self) -> impl Iterator<Item = &(SymbolId, Value)> {
        self.bindings.iter()
    }
}

/// The state of one language session: settings, suppression, symbols, scope.
///
/// `Session` is where the dynamic scope lives, and the only place mutable
/// language state is allowed to live at all (ADR-11). Its frame stack has
/// exactly one frame — the global frame — in a fresh session, and keeps at
/// least that one for its whole life, so a `Session` always has somewhere to
/// bind a name.
///
/// Cloning produces an *independent* session: the settings, the suppression
/// catalog, and the symbol table are duplicated, and the frames are copied as
/// bindings that share their values by reference count. Mutating the clone —
/// assigning a name, interning, suppressing a message — never disturbs the
/// original, and the shared values are immutable, so no clone can be observed
/// to have changed a value the other one also holds. Reconstructing a session
/// wholesale is also the restart mechanism ADR-11 §5 settles on, rather than an
/// unwind protocol over live state.
///
/// # Examples
///
/// ```
/// use rhytido_backend::Integer;
/// use rhytido_core::Atom;
/// use rhytido_lang::{Session, Value};
///
/// let mut outer = Session::new();
/// let name = outer.intern("n");
/// outer.assign(name, Value::atom(Atom::integer(Integer::from(1))));
///
/// // A clone is an independent session with the same starting bindings.
/// let mut inner = outer.clone();
/// inner.assign(name, Value::atom(Atom::integer(Integer::from(2))));
/// inner.push_frame();
/// assert_eq!(inner.current_depth(), 2);
/// assert_eq!(outer.current_depth(), 1);
///
/// let read = |session: &Session| {
///     session
///         .lookup(name)
///         .and_then(Value::as_atom)
///         .and_then(Atom::as_integer)
///         .cloned()
/// };
/// assert_eq!(read(&outer), Some(Integer::from(1)));
/// assert_eq!(read(&inner), Some(Integer::from(2)));
/// ```
#[derive(Clone, Debug)]
pub struct Session {
    settings: Settings,
    catalog: MessageCatalog,
    symbols: SymbolTable,
    frames: Vec<Frame>,
}

impl Session {
    /// Creates a session with default settings, an empty message catalog, an
    /// empty symbol table, and one empty global frame.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn new() -> Self {
        Self {
            settings: Settings::new(),
            catalog: MessageCatalog::new(),
            symbols: SymbolTable::new(),
            frames: vec![Frame::new()],
        }
    }

    /// Returns the session's settings.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn settings(&self) -> Settings {
        self.settings
    }

    /// Replaces the session's settings.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn set_settings(&mut self, settings: Settings) {
        self.settings = settings;
    }

    /// Borrows the message catalog, which holds the session's suppression set.
    ///
    /// The catalog is core's own mechanism, reused rather than duplicated
    /// (ADR-11 §1); suppression lives per session because nothing here is
    /// global.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn catalog(&self) -> &MessageCatalog {
        &self.catalog
    }

    /// Borrows the message catalog mutably, to suppress or unsuppress codes.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn catalog_mut(&mut self) -> &mut MessageCatalog {
        &mut self.catalog
    }

    /// Borrows the session's symbol table.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn symbols(&self) -> &SymbolTable {
        &self.symbols
    }

    /// Interns `text` in this session's symbol table, returning its name.
    ///
    /// Same text, same [`SymbolId`], every time, for the life of the session.
    ///
    /// # Panics
    ///
    /// Only on exhausting the identifier space, as [`SymbolTable::intern`]
    /// documents.
    pub fn intern(&mut self, text: &str) -> SymbolId {
        self.symbols.intern(text)
    }

    /// Borrows the text `name` was interned from, or `None` if this session
    /// never interned that name.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn text(&self, name: SymbolId) -> Option<&str> {
        self.symbols.text(name)
    }

    /// Looks `name` up in the dynamic scope: the innermost frame that binds it
    /// wins, walking outward (ADR-11 §3).
    ///
    /// Returns `None` when no frame binds the name; the fallback behaviour for
    /// an unbound name is an evaluator contract, settled by reference
    /// fixtures in M3-C.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn lookup(&self, name: SymbolId) -> Option<&Value> {
        self.frames.iter().rev().find_map(|frame| frame.get(name))
    }

    /// Assigns `value` to `name` by the ADR-11 §3 rule: write the innermost
    /// frame that already binds it, and otherwise define it in the current
    /// (innermost) frame.
    ///
    /// The distinction matters for dynamic scope. Assigning a name bound two
    /// frames out updates *that* frame and stays visible when the inner one is
    /// popped, while [`Session::define_current`] always introduces a shadowing
    /// binding in the current frame instead.
    ///
    /// # Panics
    ///
    /// Never. The frame stack is never empty, so there is always a current
    /// frame to define into.
    pub fn assign(&mut self, name: SymbolId, value: Value) {
        for frame in self.frames.iter_mut().rev() {
            if frame.get(name).is_some() {
                frame.set(name, value);
                return;
            }
        }
        if let Some(current) = self.frames.last_mut() {
            current.set(name, value);
        }
    }

    /// Binds `name` to `value` in the current frame, shadowing any outer
    /// binding rather than updating it.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn define_current(&mut self, name: SymbolId, value: Value) {
        if let Some(current) = self.frames.last_mut() {
            current.set(name, value);
        }
    }

    /// Pushes an empty frame, the entry half of a procedure application.
    ///
    /// Prefer [`Session::with_frame`], which cannot be paired wrongly.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn push_frame(&mut self) {
        self.frames.push(Frame::new());
    }

    /// Pops the innermost frame, the exit half of a procedure application.
    ///
    /// Returns `false` and changes nothing when only the global frame is left:
    /// popping it is a caller bug, not a state this type is allowed to reach,
    /// and reporting it beats panicking inside an evaluator that may well be
    /// mid-recursion.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn pop_frame(&mut self) -> bool {
        if self.frames.len() <= 1 {
            return false;
        }
        self.frames.pop();
        true
    }

    /// Runs `body` with a freshly pushed frame, and pops that frame on the way
    /// out.
    ///
    /// The guard is the point (ADR-11 §4): a procedure that returns early, or
    /// propagates an error with `?`, leaves the stack exactly as it found it,
    /// because the pop happens on the sole path out of this method rather than
    /// at each call site.
    ///
    /// A `body` that *panics* is a different matter: unwinding past the pop
    /// would need a drop guard holding the session by reference, and the
    /// resulting borrow discipline buys nothing here. Panics are contained at
    /// the CLI boundary, where a session is rebuilt instead (ADR-11 §5; M3-E),
    /// so `with_frame` documents the frame stack as unreliable across a panic
    /// and does not try to repair one.
    ///
    /// # Panics
    ///
    /// If `body` panics, the panic propagates and the frame is *not* popped;
    /// see above.
    pub fn with_frame<R>(&mut self, body: impl FnOnce(&mut Self) -> R) -> R {
        self.push_frame();
        let result = body(self);
        self.pop_frame();
        result
    }

    /// The current frame depth, counting the global frame.
    ///
    /// At least `1`, and `1` for a session that is not inside a procedure call.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn current_depth(&self) -> usize {
        self.frames.len()
    }

    /// Borrows the values bound to `name` in the current frame only, ignoring
    /// outer bindings.
    ///
    /// # Panics
    ///
    /// Never.
    pub fn lookup_current(&self, name: SymbolId) -> Option<&Value> {
        self.frames.last().and_then(|frame| frame.get(name))
    }
}

impl Default for Session {
    /// A fresh session (see [`Session::new`]).
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rhytido_core::Atom;

    // These unit tests reach into internals that the public API deliberately
    // keeps hidden — the shape of the frame vector and the sharing that cloning
    // establishes. Every observable behaviour is tested from the outside in
    // `crates/rhytido-lang/tests/context.rs` (AGENTS.md §4).

    fn name_of(session: &mut Session, text: &str) -> SymbolId {
        session.intern(text)
    }

    #[test]
    fn normal_frames_vector_keeps_global_frame_first() {
        let mut session = Session::new();
        session.push_frame();
        session.push_frame();
        assert_eq!(session.frames.len(), 3);
        assert!(session.frames[0].is_empty());
        assert!(session.frames[2].is_empty());
    }

    #[test]
    fn normal_clone_shares_a_list_binding_by_reference_count() {
        let mut session = Session::new();
        let name = name_of(&mut session, "l");
        session.assign(name, Value::list_from_iter([Value::default()]));
        let clone = session.clone();
        let original = session.lookup(name).and_then(Value::as_list).expect("list");
        let copied = clone.lookup(name).and_then(Value::as_list).expect("list");
        // The element buffers alias, so cloning a session is not a deep copy.
        assert!(std::ptr::eq(original.as_ptr(), copied.as_ptr()));
        assert_eq!(original.len(), copied.len());
    }

    #[test]
    fn extreme_assign_reuses_one_slot_in_a_frame() {
        let mut session = Session::new();
        let name = name_of(&mut session, "x");
        for _ in 0..32 {
            session.assign(name, Value::default());
        }
        assert_eq!(session.frames[0].len(), 1);
    }

    #[test]
    fn extreme_frame_scan_order_follows_definition() {
        let mut frame = Frame::new();
        for index in 0..64u32 {
            frame.set(SymbolId::new(index), Value::default());
        }
        let order: Vec<u32> = frame.iter().map(|(name, _)| name.index()).collect();
        assert_eq!(order.first(), Some(&0));
        assert_eq!(order.last(), Some(&63));
    }

    #[test]
    fn error_assign_to_unbound_name_in_empty_frame_defines_current() {
        // `assign` never fails; the internal shape records that as exactly one
        // new binding in the innermost frame.
        let mut session = Session::new();
        session.push_frame();
        let name = name_of(&mut session, "fresh");
        session.assign(name, Value::default());
        assert!(session.frames[0].is_empty());
        assert_eq!(session.frames[1].len(), 1);
    }

    #[test]
    fn robustness_pop_below_global_frame_leaves_vector_untouched() {
        let mut session = Session::new();
        let before = session.frames.len();
        for _ in 0..8 {
            assert!(!session.pop_frame());
        }
        assert_eq!(session.frames.len(), before);
        assert_eq!(session.frames.len(), 1);
    }

    #[test]
    fn robustness_deep_stack_lookup_reaches_global_binding() {
        let mut session = Session::new();
        let name = name_of(&mut session, "g");
        session.assign(name, Value::atom(Atom::boolean(true)));
        for _ in 0..256 {
            session.push_frame();
        }
        // Innermost-first scanning still finds the global binding, and the
        // borrow it returns comes from frame zero.
        assert!(session.lookup(name).is_some());
        assert_eq!(session.frames[256].len(), 0);
        assert_eq!(session.frames[0].len(), 1);
    }
}
