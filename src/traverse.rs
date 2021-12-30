use crate::ast;
use crate::symbol::Symbol;

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
/// This function works like the visitor pattern. The depth is determined
/// by the given filter.
pub fn walk_symbols<'a, F: FnMut(Symbol<'a>)>(ast: &'a ast::Aidl, filter: SymbolFilter, mut f: F) {
    macro_rules! visit_type_helper {
        ($t:expr, $f:ident) => {
            $f(Symbol::Type($t));
            $t.generic_types.iter().for_each(|t| $f(Symbol::Type(t)));
        };
    }

    match ast.item {
        ast::Item::Interface(ref i) => {
            f(Symbol::Interface(i));
            if let SymbolFilter::ItemsOnly = filter {
                return;
            }

            i.elements.iter().for_each(|el| match el {
                ast::InterfaceElement::Method(m) => {
                    f(Symbol::Method(m));
                    if let SymbolFilter::All = filter {
                        visit_type_helper!(&m.return_type, f);
                        m.args.iter().for_each(|arg| {
                            visit_type_helper!(&arg.arg_type, f);
                        })
                    }
                }
                ast::InterfaceElement::Const(c) => {
                    f(Symbol::Const(c));
                    if let SymbolFilter::All = filter {
                        visit_type_helper!(&c.const_type, f);
                    }
                }
            });
        }
        ast::Item::Parcelable(ref p) => {
            f(Symbol::Parcelable(p));
            if let SymbolFilter::ItemsOnly = filter {
                return;
            }

            p.members.iter().for_each(|m| {
                f(Symbol::Member(m));

                if let SymbolFilter::All = filter {
                    visit_type_helper!(&m.member_type, f);
                }
            });
        }
        ast::Item::Enum(ref e) => {
            f(Symbol::Enum(e));
            if let SymbolFilter::ItemsOnly = filter {
                return;
            }

            e.elements.iter().for_each(|el| {
                f(Symbol::EnumElement(el));
            });
        }
    }
}

/// Traverse the AST and provide the types to the given closure
pub fn walk_types<F: FnMut(&ast::Type)>(ast: &ast::Aidl, mut f: F) {
    let mut visit_type_helper = move |type_: &ast::Type| {
        f(type_);
        type_.generic_types.iter().for_each(&mut f);
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
            p.members.iter().for_each(|m| {
                visit_type_helper(&m.member_type);
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
            p.members.iter_mut().for_each(|m| {
                visit_type_helper(&mut m.member_type);
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
