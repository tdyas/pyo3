use clippy_utils::diagnostics::span_lint_and_help;
use std::collections::HashSet;
use rustc_hir::def::{DefKind, Res};
use rustc_hir::def_id::LocalDefId;
use rustc_hir::intravisit::{self, Visitor};
use rustc_hir::{BodyOwnerKind, Expr, ExprKind};
use rustc_lint::{LateContext, LateLintPass};
use rustc_lint_defs::{LintPass, LintVec};
use rustc_middle::ty::{self, Ty};
use rustc_session::declare_lint;

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

pub struct MutexLockPyAttached {
    implied_attached: HashSet<LocalDefId>,
    wrappers_scanned: bool,
}

impl Default for MutexLockPyAttached {
    fn default() -> Self {
        Self {
            implied_attached: HashSet::new(),
            wrappers_scanned: false,
        }
    }
}

impl LintPass for MutexLockPyAttached {
    fn name(&self) -> &'static str {
        "MutexLockPyAttached"
    }

    fn get_lints(&self) -> LintVec {
        vec![&MUTEX_LOCK_PY_ATTACHED]
    }
}

impl MutexLockPyAttached {
    fn ensure_wrappers_scanned<'tcx>(&mut self, cx: &LateContext<'tcx>) {
        if self.wrappers_scanned {
            return;
        }

        self.wrappers_scanned = true;

        for def_id in cx.tcx.hir_body_owners() {
            if !matches!(cx.tcx.hir_body_owner_kind(def_id), BodyOwnerKind::Fn) {
                continue;
            }

            if !is_pyo3_wrapper(cx, def_id) {
                continue;
            }

            let body = cx.tcx.hir_body_owned_by(def_id);
            let mut collector = WrapperCollector {
                cx,
                exclude: def_id,
                attached: &mut self.implied_attached,
            };
            collector.visit_body(body);
        }
    }
}

impl<'tcx> LateLintPass<'tcx> for MutexLockPyAttached {
    fn check_expr(&mut self, cx: &LateContext<'tcx>, expr: &'tcx Expr<'_>) {
        self.ensure_wrappers_scanned(cx);

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
            if !has_python_token_in_scope(cx, expr, &self.implied_attached) {
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

struct WrapperCollector<'a, 'tcx> {
    cx: &'a LateContext<'tcx>,
    exclude: LocalDefId,
    attached: &'a mut HashSet<LocalDefId>,
}

impl<'tcx> Visitor<'tcx> for WrapperCollector<'_, 'tcx> {
    fn visit_expr(&mut self, expr: &'tcx Expr<'tcx>) {
        if let ExprKind::Path(ref qpath) = expr.kind {
            let res = self.cx.qpath_res(qpath, expr.hir_id);
            if let Res::Def(def_kind, def_id) = res {
                if matches!(def_kind, DefKind::Fn | DefKind::AssocFn | DefKind::Ctor(..)) {
                    if let Some(local) = def_id.as_local() {
                        if local != self.exclude {
                            self.attached.insert(local);
                        }
                    }
                }
            }
        }

        intravisit::walk_expr(self, expr);
    }
}

fn is_pyo3_wrapper(cx: &LateContext<'_>, def_id: LocalDefId) -> bool {
    let Some(name) = cx.tcx.opt_item_name(def_id.to_def_id()) else {
        return false;
    };
    let name_str = name.as_str();
    if !name_str.starts_with("__pyfunction_")
        && !name_str.starts_with("__pymethod_")
        && !name_str.starts_with("__pymodule_")
    {
        return false;
    }

    let sig = cx.tcx.fn_sig(def_id.to_def_id()).skip_binder();
    let first_input = match sig.inputs().skip_binder().first() {
        Some(ty) => *ty,
        None => return false,
    };

    is_python_token_type(cx, first_input)
}

/// Check if the type is std::sync::Mutex or parking_lot::Mutex
fn is_mutex_type(cx: &LateContext<'_>, ty: Ty<'_>) -> bool {
    const MUTEX_TYPES: &[&[&str]] = &[
        &["std", "sync", "poison", "mutex", "Mutex"],
        &["parking_lot", "Mutex"],
        &["lock_api", "mutex", "Mutex"],
    ];

    match ty.kind() {
        ty::Adt(adt, _) => {
            let def_id = adt.did();
            MUTEX_TYPES
                .iter()
                .any(|path| crate::utils::match_def_path(cx, def_id, *path))
        }
        _ => false,
    }
}

/// Check if there's a Python<'_> token in the current function scope
fn has_python_token_in_scope(
    cx: &LateContext<'_>,
    expr: &Expr<'_>,
    implied_attached: &HashSet<LocalDefId>,
) -> bool {
    // Get the body owner (the function containing this expression).
    let owner_id = cx.tcx.hir_enclosing_body_owner(expr.hir_id);

    if implied_attached.contains(&owner_id) {
        return true;
    }

    let body = cx.tcx.hir_body_owned_by(owner_id);

    // Check all function parameters for Python<'_>.
    for param in body.params {
        let param_ty = cx.typeck_results().node_type(param.pat.hir_id);
        if is_python_token_type(cx, param_ty) {
            return true;
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
