// SPDX-License-Identifier: Apache-2.0
//! Build script: compile the `src/*.lalrpop` grammar into `OUT_DIR`.
//!
//! `lalrpop::process_src()` walks `src/`, generates a `.rs` for every
//! `.lalrpop` file into the output directory, and refreshes them only when the
//! grammar changes (via the `rerun-if-changed` directives it emits). The
//! generated module is pulled in by `src/parse.rs` with
//! `lalrpop_util::lalrpop_mod!`.

fn main() {
    // A grammar-authoring failure is a build failure: surfacing it here is the
    // build script's whole job, so aborting the process on `Err` is correct.
    if let Err(e) = lalrpop::process_src() {
        eprintln!("lalrpop grammar generation failed: {e}");
        std::process::exit(1);
    }
}
