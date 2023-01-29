use std::ops::ControlFlow;

use crate::ast;
use crate::symbol::{ConstOwner, Symbol};

/// Determine the depth of traversal functions
#[derive(Clone, Copy)]
pub enum SymbolFilter {
    /// Only extract the top-level item
    ItemsOnly,
    /// Extract the top-level and its direct children (e.g.: parcelable + fields)
    ItemsAndItemElements,
    /// Extract all symbols (incl. types)
    All,
}

/// Traverse the AST and provide the symbols to the given closure
///
/// This function works like the visitor pattern.
pub fn walk_symbols<'a, F: FnMut(Symbol<'a>)>(ast: &'a ast::Aidl, filter: SymbolFilter, mut f: F) {
    walk_symbols_with_control_flow(ast, filter, |smb| -> ControlFlow<()> {
        f(smb);
        ControlFlow::Continue(())
    });
}

/// Traverse the AST and filter the symbols based on the given predicate.
///
/// For each symbol, the predicate is called and the symbol will be
/// added to the returned vector when the return value is true.
///
/// See also: [`walk_symbols`]
pub fn filter_symbols<'a, F>(ast: &'a ast::Aidl, filter: SymbolFilter, mut f: F) -> Vec<Symbol<'a>>
where
    F: FnMut(&Symbol<'a>) -> bool,
{
    let mut v = Vec::new();
    walk_symbols(ast, filter, |symbol| {
        if f(&symbol) {
            v.push(symbol);
        }
    });

    v
}

/// Look for a symbol inside the AST based on the given predicate.
///
/// Return the first symbol for which the predicate returns true, or `None`
/// if no matching symbol has been found.
///
/// See also: [`walk_symbols`]
pub fn find_symbol<'a, F>(ast: &'a ast::Aidl, filter: SymbolFilter, mut f: F) -> Option<Symbol<'a>>
where
    F: FnMut(&Symbol<'a>) -> bool,
{
    let res = walk_symbols_with_control_flow(ast, filter, |smb| -> ControlFlow<Symbol<'a>> {
        if f(&smb) {
            ControlFlow::Break(smb)
        } else {
            ControlFlow::Continue(())
        }
    });

    match res {
        ControlFlow::Continue(_) => None,
        ControlFlow::Break(smb) => Some(smb),
    }
}

/// Look for a symbol at a given position.
///
/// Return the first symbol whose range includes the given position, or `None`
/// if no matching symbol has been found.
///
/// See also: [`find_symbol`]
pub fn find_symbol_at_line_col(
    ast: &ast::Aidl,
    filter: SymbolFilter,
    line_col: (usize, usize),
) -> Option<Symbol> {
    find_symbol(ast, filter, |smb| range_contains(smb.get_range(), line_col))
}

#[allow(clippy::needless_borrow)] // because of false-positives when invoking macros...
fn walk_symbols_with_control_flow<'a, V, F>(
    ast: &'a ast::Aidl,
    filter: SymbolFilter,
    mut f: F,
) -> ControlFlow<V>
where
    F: FnMut(Symbol<'a>) -> ControlFlow<V>,
{
    macro_rules! visit_type_helper {
        ($t:expr, $f:ident) => {
            if $t.kind == ast::TypeKind::Array {
                // For arrays, start with the array element type, then on the array itself
                $t.generic_types
                    .iter()
                    .try_for_each(|t| $f(Symbol::Type(t)))?;
                $f(Symbol::Type($t))?;
            } else {
                // For other types, start with the main type and then its generic types
                $f(Symbol::Type($t))?;
                $t.generic_types
                    .iter()
                    .try_for_each(|t| $f(Symbol::Type(t)))?;
            }
        };
    }

    if let SymbolFilter::All = filter {
        f(Symbol::Package(&ast.package));

        for import in &ast.imports {
            f(Symbol::Import(import))?;
        }
    }

    match ast.item {
        ast::Item::Interface(ref i) => {
            f(Symbol::Interface(i, &ast.package))?;
            if let SymbolFilter::ItemsOnly = filter {
                return ControlFlow::Continue(());
            }

            i.elements.iter().try_for_each(|el| match el {
                ast::InterfaceElement::Method(m) => {
                    f(Symbol::Method(m, i))?;
                    if let SymbolFilter::All = filter {
                        visit_type_helper!(&m.return_type, f);
                        m.args.iter().try_for_each(|arg| {
                            f(Symbol::Arg(arg, m))?;
                            visit_type_helper!(&arg.arg_type, f);
                            ControlFlow::Continue(())
                        })?;
                    }
                    ControlFlow::Continue(())
                }
                ast::InterfaceElement::Const(c) => {
                    f(Symbol::Const(c, ConstOwner::Interface(i)))?;
                    if let SymbolFilter::All = filter {
                        visit_type_helper!(&c.const_type, f);
                    }
                    ControlFlow::Continue(())
                }
            })?;
        }
        ast::Item::Parcelable(ref p) => {
            f(Symbol::Parcelable(p, &ast.package))?;
            if let SymbolFilter::ItemsOnly = filter {
                return ControlFlow::Continue(());
            }

            p.elements.iter().try_for_each(|el| match el {
                ast::ParcelableElement::Field(fi) => {
                    f(Symbol::Field(fi, p))?;
                    if let SymbolFilter::All = filter {
                        visit_type_helper!(&fi.field_type, f);
                    }

                    ControlFlow::Continue(())
                }
                ast::ParcelableElement::Const(c) => {
                    f(Symbol::Const(c, ConstOwner::Parcelable(p)))?;
                    if let SymbolFilter::All = filter {
                        visit_type_helper!(&c.const_type, f);
                    }
                    ControlFlow::Continue(())
                }
            })?;
        }
        ast::Item::Enum(ref e) => {
            f(Symbol::Enum(e, &ast.package))?;
            if let SymbolFilter::ItemsOnly = filter {
                return ControlFlow::Continue(());
            }

            e.elements.iter().try_for_each(|el| {
                f(Symbol::EnumElement(el, e))?;
                ControlFlow::Continue(())
            })?;
        }
    }

    ControlFlow::Continue(())
}

fn range_contains(range: &ast::Range, line_col: (usize, usize)) -> bool {
    if range.start.line_col.0 > line_col.0 {
        return false;
    }

    if range.start.line_col.0 == line_col.0 && range.start.line_col.1 > line_col.1 {
        return false;
    }

    if range.end.line_col.0 < line_col.0 {
        return false;
    }

    if range.end.line_col.0 == line_col.0 && range.end.line_col.1 < line_col.1 {
        return false;
    }

    true
}

/// Traverse the AST and provide the types to the given closure
pub fn walk_types<F: FnMut(&ast::Type)>(ast: &ast::Aidl, mut f: F) {
    let mut visit_type_helper = move |type_: &ast::Type| {
        if type_.kind == ast::TypeKind::Array {
            // For arrays, start with the array element type, then on the array itself
            type_.generic_types.iter().for_each(&mut f);
            f(type_);
        } else {
            // For other types, start with the main type and then its generic types
            f(type_);
            type_.generic_types.iter().for_each(&mut f);
        }
    };

    match ast.item {
        ast::Item::Interface(ref i) => {
            i.elements.iter().for_each(|el| match el {
                ast::InterfaceElement::Method(m) => {
                    visit_type_helper(&m.return_type);
                    m.args.iter().for_each(|arg| {
                        visit_type_helper(&arg.arg_type);
                    })
                }
                ast::InterfaceElement::Const(c) => {
                    visit_type_helper(&c.const_type);
                }
            });
        }
        ast::Item::Parcelable(ref p) => {
            p.elements.iter().for_each(|el| match el {
                ast::ParcelableElement::Field(fi) => {
                    visit_type_helper(&fi.field_type);
                }
                ast::ParcelableElement::Const(c) => {
                    visit_type_helper(&c.const_type);
                }
            });
        }
        ast::Item::Enum(_) => (),
    }
}

pub(crate) fn walk_types_mut<F: FnMut(&mut ast::Type)>(ast: &mut ast::Aidl, mut f: F) {
    let mut visit_type_helper = move |type_: &mut ast::Type| {
        f(type_);
        type_.generic_types.iter_mut().for_each(&mut f);
    };

    match ast.item {
        ast::Item::Interface(ref mut i) => {
            i.elements.iter_mut().for_each(|el| match el {
                ast::InterfaceElement::Method(m) => {
                    visit_type_helper(&mut m.return_type);
                    m.args.iter_mut().for_each(|arg| {
                        visit_type_helper(&mut arg.arg_type);
                    })
                }
                ast::InterfaceElement::Const(c) => {
                    visit_type_helper(&mut c.const_type);
                }
            });
        }
        ast::Item::Parcelable(ref mut p) => {
            p.elements.iter_mut().for_each(|el| match el {
                ast::ParcelableElement::Field(fi) => {
                    visit_type_helper(&mut fi.field_type);
                }
                ast::ParcelableElement::Const(c) => {
                    visit_type_helper(&mut c.const_type);
                }
            });
        }
        ast::Item::Enum(_) => (),
    }
}

/// Traverse the AST and provide the methods to the given closure
pub fn walk_methods<'a, F: FnMut(&'a ast::Method)>(ast: &'a ast::Aidl, mut f: F) {
    match ast.item {
        ast::Item::Interface(ref i) => {
            i.elements.iter().for_each(|el| match el {
                ast::InterfaceElement::Method(m) => f(m),
                ast::InterfaceElement::Const(_) => (),
            });
        }
        ast::Item::Parcelable(_) => (),
        ast::Item::Enum(_) => (),
    }
}

/// Traverse the AST and provide the method arguments to the given closure
pub fn walk_args<'a, F: FnMut(&'a ast::Method, &'a ast::Arg)>(ast: &'a ast::Aidl, mut f: F) {
    match ast.item {
        ast::Item::Interface(ref i) => {
            i.elements.iter().for_each(|el| match el {
                ast::InterfaceElement::Method(m) => m.args.iter().for_each(|arg| {
                    f(m, arg);
                }),
                ast::InterfaceElement::Const(_) => (),
            });
        }
        ast::Item::Parcelable(_) => (),
        ast::Item::Enum(_) => (),
    }
}
