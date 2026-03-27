//! Readback cleanup: unwrap Archon boundary wrapper nodes from readback terms.
//!
//! When Archon's boundary handlers transform terms, they introduce wrapper nodes
//! like `__archon_bang`, `__closure`, `__fo_clause`, `__tensor_seq`, etc.
//! This module strips those wrappers to recover the user-visible term.

use apeiron::parser::Sexp;
use apeiron::parser::Span;

/// Strip Archon boundary wrapper nodes from a readback Sexp.
/// Returns the cleaned term with all `__archon_*` and `__thermo_*` prefixes removed,
/// and structural wrappers like `__closure`, `__fo_clause` unwrapped.
pub fn strip_boundary_wrappers(sexp: &Sexp) -> Sexp {
    let sp = Span::default();
    match sexp {
        Sexp::Atom(name, _) => {
            // Strip __archon_ prefix from atom names if present.
            if let Some(stripped) = name.strip_prefix("__archon_") {
                Sexp::Atom(stripped.to_string(), sp)
            } else {
                sexp.clone()
            }
        }
        Sexp::List(items, _) => {
            if items.is_empty() {
                return sexp.clone();
            }
            let head = items[0].as_atom().unwrap_or("");

            match head {
                // Unwrap single-child wrappers: (wrapper child) → child
                "__closure" | "__fo_clause" | "__reified_ctx"
                | "__archon_bang" | "__modal_guard" | "__transport_pending"
                    if items.len() == 2 =>
                {
                    strip_boundary_wrappers(&items[1])
                }

                // Unwrap kripke threading: (__kripke_threaded inner world) → inner
                "__kripke_threaded" if items.len() >= 2 => {
                    strip_boundary_wrappers(&items[1])
                }

                // Unwrap nominal scopes: (__nominal_scope_N inner) → inner
                s if s.starts_with("__nominal_scope_") && items.len() == 2 => {
                    strip_boundary_wrappers(&items[1])
                }

                // Unwrap tensor sequence: (__tensor_seq a b) → (⊗ a b)
                "__tensor_seq" if items.len() == 3 => {
                    Sexp::List(
                        vec![
                            Sexp::Atom("⊗".to_string(), sp),
                            strip_boundary_wrappers(&items[1]),
                            strip_boundary_wrappers(&items[2]),
                        ],
                        sp,
                    )
                }

                // Strip __thermo_ prefix: (__thermo_and a b) → (and a b)
                s if s.starts_with("__thermo_") => {
                    let real_name = s.strip_prefix("__thermo_").unwrap();
                    let mut new_items = vec![Sexp::Atom(real_name.to_string(), sp)];
                    for item in &items[1..] {
                        new_items.push(strip_boundary_wrappers(item));
                    }
                    Sexp::List(new_items, sp)
                }

                // Default: recurse into children.
                _ => {
                    Sexp::List(
                        items.iter().map(strip_boundary_wrappers).collect(),
                        sp,
                    )
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use apeiron::parser::parse;

    fn s(input: &str) -> Sexp {
        parse(input).unwrap().into_iter().next().unwrap()
    }

    #[test]
    fn unwrap_closure() {
        let input = s("[__closure [lambda x body]]");
        let result = strip_boundary_wrappers(&input);
        assert_eq!(format!("{}", result), "[lambda x body]");
    }

    #[test]
    fn unwrap_thermo() {
        let input = s("[__thermo_and a b]");
        let result = strip_boundary_wrappers(&input);
        assert_eq!(format!("{}", result), "[and a b]");
    }

    #[test]
    fn unwrap_nested() {
        let input = s("[__closure [__thermo_not x]]");
        let result = strip_boundary_wrappers(&input);
        assert_eq!(format!("{}", result), "[not x]");
    }

    #[test]
    fn passthrough_normal() {
        let input = s("[add x y]");
        let result = strip_boundary_wrappers(&input);
        assert_eq!(format!("{}", result), "[add x y]");
    }
}
