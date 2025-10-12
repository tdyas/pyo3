use rustc_hir::def_id::DefId;
use rustc_lint::LateContext;
use rustc_span::symbol::Symbol;

/// Checks if the given `DefId` matches the path.
pub fn match_def_path(cx: &LateContext<'_>, did: DefId, syms: &[&str]) -> bool {
    // We should probably move to Symbols in Clippy as well rather than interning every time.
    let path = cx.get_def_path(did);
    // eprintln!("{path:?}");
    syms.iter()
        .map(|x| Symbol::intern(x))
        .eq(path.iter().copied())
}
