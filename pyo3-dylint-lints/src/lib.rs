#![feature(rustc_private)]
#![warn(unused_extern_crates)]

extern crate rustc_hir;
extern crate rustc_lint;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;

pub mod mutex_lock_py_attached;
pub mod utils;

dylint_linting::dylint_library!();

#[doc(hidden)]
#[unsafe(no_mangle)]
pub fn register_lints(_session: &rustc_session::Session, lint_store: &mut rustc_lint::LintStore) {
    lint_store.register_lints(&[mutex_lock_py_attached::MUTEX_LOCK_PY_ATTACHED]);
    lint_store.register_late_pass(|_| Box::new(mutex_lock_py_attached::MutexLockPyAttached));
}

#[test]
fn ui() {
    dylint_testing::ui_test(env!("CARGO_PKG_NAME"), "ui");
}
