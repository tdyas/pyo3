use clippy_utils::diagnostics::span_lint_and_help;
use rustc_hir::{Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_middle::ty::{self, Ty};

use rustc_session::{
    declare_lint,
    declare_lint_pass,
};

declare_lint! {
    /// ### What it does
    /// Checks for direct calls to `.lock()` on `Mutex` types when a `Python<'_>` token
    /// is available in the current scope, which could cause deadlocks with the Python
    /// runtime.
    ///
    /// ### Why is this bad?
    /// When a thread is attached to the Python runtime (indicated by having a `Python<'_>`
    /// token), calling `.lock()` on a mutex can cause deadlocks. This is because the thread
    /// holds attachment to Python while waiting for the lock, which can conflict with the
    /// GIL and other Python synchronization events.
    ///
    /// PyO3 provides `MutexExt::lock_py_attached(py)` which safely detaches from the Python
    /// runtime before acquiring the lock and re-attaches afterwards.
    ///
    /// ### Example
    /// ```rust,ignore
    /// use pyo3::prelude::*;
    /// use std::sync::Mutex;
    ///
    /// fn bad_example(py: Python<'_>, mutex: &Mutex<i32>) {
    ///     let guard = mutex.lock().unwrap(); // ⚠️ Could deadlock!
    /// }
    /// ```
    ///
    /// Use instead:
    /// ```rust,ignore
    /// use pyo3::prelude::*;
    /// use pyo3::sync::MutexExt;
    /// use std::sync::Mutex;
    ///
    /// fn good_example(py: Python<'_>, mutex: &Mutex<i32>) {
    ///     let guard = mutex.lock_py_attached(py).unwrap(); // ✓ Safe!
    /// }
    /// ```
    pub MUTEX_LOCK_PY_ATTACHED,
    Warn,
    "calling `.lock()` on a Mutex when Python token is available may cause deadlocks"
}

declare_lint_pass!(MutexLockPyAttached => [MUTEX_LOCK_PY_ATTACHED]);

impl<'tcx> LateLintPass<'tcx> for MutexLockPyAttached {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'_>) {
        // Check if this is a method call to `lock()`
        if let ExprKind::MethodCall(path, receiver, args, _) = expr.kind {
            if path.ident.name.as_str() != "lock" {
                return;
            }

            // Only trigger if lock() is called with no arguments
            if !args.is_empty() {
                return;
            }

            let receiver_ty = cx.typeck_results().expr_ty(receiver).peel_refs();

            // Check if the receiver is a Mutex type
            if !is_mutex_type(cx, receiver_ty) {
                return;
            }

            // Check if there's a Python<'_> token available in scope
            if !has_python_token_in_scope(cx, expr) {
                return;
            }

            span_lint_and_help(
                cx,
                MUTEX_LOCK_PY_ATTACHED,
                expr.span,
                "calling `.lock()` on a Mutex when a Python token is available may cause deadlocks",
                None,
                "consider using `.lock_py_attached(py)` instead, which safely detaches from the Python runtime before locking",
            );
        }
    }
}

/// Check if the type is std::sync::Mutex or parking_lot::Mutex
fn is_mutex_type(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    match ty.kind() {
        ty::Adt(adt, _) => {
            let def_id = adt.did();

            // Check for std::sync::Mutex
            if crate::utils::match_def_path(cx, def_id, &["std", "sync", "mutex", "Mutex"]) {
                return true;
            }

            // Check for parking_lot::Mutex
            if crate::utils::match_def_path(cx, def_id, &["parking_lot", "Mutex"]) {
                return true;
            }

            // Check for lock_api::Mutex
            if crate::utils::match_def_path(cx, def_id, &["lock_api", "mutex", "Mutex"]) {
                return true;
            }

            false
        }
        _ => false,
    }
}

/// Check if there's a Python<'_> token in the current function scope
fn has_python_token_in_scope(cx: &LateContext<'_>, expr: &Expr<'_>) -> bool {
    // Get the body owner (the function containing this expression).
    let owner_id = cx.tcx.hir_enclosing_body_owner(expr.hir_id);

    // Get the function signature
    if let Some(fn_decl) = cx
        .tcx
        .hir_fn_decl_by_hir_id(cx.tcx.local_def_id_to_hir_id(owner_id))
    {
        // Check all function parameters for Python<'_>.
        for param in fn_decl.inputs {
            let param_ty = cx.typeck_results().node_type(param.hir_id);
            if is_python_token_type(cx, param_ty) {
                return true;
            }
        }
    }

    false
}

/// Check if a type is pyo3::Python<'_>.
fn is_python_token_type(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    match ty.kind() {
        ty::Adt(adt, _) => {
            let def_id = adt.did();
            crate::utils::match_def_path(cx, def_id, &["pyo3", "marker", "Python"])
        }
        _ => false,
    }
}
