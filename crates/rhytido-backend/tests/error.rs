// SPDX-License-Identifier: Apache-2.0
//! Module-test layer (AGENTS.md §4): public API of `src/error.rs`.
//!
//! External-crate view of the `Error` enum (new coverage; the module had no
//! inline tests). Four categories: normal / extreme / error / robustness.

use std::error::Error as StdError;

use rhytido_backend::Error;

// ---- Normal cases -----------------------------------------------------

#[test]
fn normal_display_messages() {
    assert_eq!(
        Error::Parse("12x".to_string()).to_string(),
        "invalid numeric literal: 12x"
    );
    assert_eq!(Error::DivisionByZero.to_string(), "division by zero");
}

#[test]
fn normal_clone_and_partial_eq() {
    let parse = Error::Parse("bad".to_string());
    assert_eq!(parse.clone(), parse);
    assert_eq!(Error::DivisionByZero.clone(), Error::DivisionByZero);
    assert_ne!(parse, Error::Parse("other".to_string()));
    assert_ne!(parse, Error::DivisionByZero);
    let a = Error::Parse("a".to_string());
    let b = Error::Parse("b".to_string());
    assert_ne!(a, b);
}

#[test]
fn normal_usable_as_std_error() {
    let parse = Error::Parse("bad".to_string());
    let dyn_parse: &dyn StdError = &parse;
    assert_eq!(dyn_parse.to_string(), "invalid numeric literal: bad");
    assert!(dyn_parse.source().is_none());
    let div: &dyn StdError = &Error::DivisionByZero;
    assert_eq!(div.to_string(), "division by zero");
    assert!(div.source().is_none());
}

// ---- Extreme cases ----------------------------------------------------

#[test]
fn extreme_parse_payload_edges() {
    assert_eq!(
        Error::Parse(String::new()).to_string(),
        "invalid numeric literal: "
    );
    let long = "x".repeat(10_000);
    let e = Error::Parse(long.clone());
    assert_eq!(e.to_string(), format!("invalid numeric literal: {long}"));
    assert_eq!(e.clone(), e);
}

// ---- Error cases (异常报错) -------------------------------------------

#[test]
fn error_result_propagation() {
    fn parse_positive(src: &str) -> Result<i64, Error> {
        let parsed = src.parse::<i64>();
        let v = parsed.map_err(|e| Error::Parse(e.to_string()))?;
        if v == 0 {
            return Err(Error::DivisionByZero);
        }
        Ok(v)
    }
    assert_eq!(parse_positive("42").unwrap(), 42);
    assert_eq!(parse_positive("0"), Err(Error::DivisionByZero));
    let e = parse_positive("not_a_number").unwrap_err();
    assert!(matches!(e, Error::Parse(_)));
    assert!(e.to_string().contains("invalid numeric literal"));
}

#[test]
fn error_boxed_downcast_roundtrip() {
    let boxed: Box<dyn StdError> = Box::new(Error::Parse("bad".to_string()));
    let recovered = boxed.downcast::<Error>().unwrap();
    assert_eq!(*recovered, Error::Parse("bad".to_string()));
    let boxed: Box<dyn StdError> = Box::new(Error::DivisionByZero);
    let recovered = boxed.downcast::<Error>().unwrap();
    assert_eq!(*recovered, Error::DivisionByZero);
}

// ---- Interruption / robustness cases (异常中断) -----------------------

#[test]
fn robustness_clone_and_display_stable() {
    let e = Error::Parse("bad".to_string());
    let c = e.clone();
    // Formatting is repeatable and clones stay equal.
    for _ in 0..10 {
        assert_eq!(e.to_string(), c.to_string());
        assert_eq!(e, c);
    }
    // Debug output is available for diagnostics.
    assert!(!format!("{e:?}").is_empty());
    assert!(!format!("{:?}", Error::DivisionByZero).is_empty());
}
