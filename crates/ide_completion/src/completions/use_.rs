//! Completion for use trees
//!
//! This module uses a bit of static metadata to provide completions
//! for built-in attributes.
//! Non-built-in attribute (excluding derives attributes) completions are done in [`super::unqualified_path`].

use std::iter;

use hir::ScopeDef;
use syntax::{ast, AstNode};

use crate::{
    context::{CompletionContext, PathCompletionContext, PathKind},
    Completions,
};

pub(crate) fn complete_use_tree(acc: &mut Completions, ctx: &CompletionContext) {
    let (is_trivial_path, qualifier, use_tree_parent) = match ctx.path_context {
        Some(PathCompletionContext {
            kind: Some(PathKind::Use),
            is_trivial_path,
            use_tree_parent,
            ref qualifier,
            ..
        }) => (is_trivial_path, qualifier, use_tree_parent),
        _ => return,
    };

    match qualifier {
        Some((path, qualifier)) => {
            let is_super_chain = iter::successors(Some(path.clone()), |p| p.qualifier())
                .all(|p| p.segment().and_then(|s| s.super_token()).is_some());
            if is_super_chain {
                acc.add_keyword(ctx, "super::");
            }
            // only show `self` in a new use-tree when the qualifier doesn't end in self
            let not_preceded_by_self = use_tree_parent
                && !matches!(
                    path.segment().and_then(|it| it.kind()),
                    Some(ast::PathSegmentKind::SelfKw)
                );
            if not_preceded_by_self {
                acc.add_keyword(ctx, "self");
            }

            let qualifier = match qualifier {
                Some(it) => it,
                None => return,
            };

            match qualifier {
                hir::PathResolution::Def(hir::ModuleDef::Module(module)) => {
                    let module_scope = module.scope(ctx.db, ctx.module);
                    let unknown_is_current = |name: &hir::Name| {
                        matches!(
                            ctx.name_syntax.as_ref(),
                            Some(ast::NameLike::NameRef(name_ref))
                                if name_ref.syntax().text() == name.to_smol_str().as_str()
                        )
                    };
                    for (name, def) in module_scope {
                        let add_resolution = match def {
                            ScopeDef::Unknown if unknown_is_current(&name) => {
                                // for `use self::foo$0`, don't suggest `foo` as a completion
                                cov_mark::hit!(dont_complete_current_use);
                                continue;
                            }
                            ScopeDef::ModuleDef(_) | ScopeDef::MacroDef(_) | ScopeDef::Unknown => {
                                true
                            }
                            _ => false,
                        };

                        if add_resolution {
                            acc.add_resolution(ctx, name, def);
                        }
                    }
                }
                hir::PathResolution::Def(hir::ModuleDef::Adt(hir::Adt::Enum(e))) => {
                    cov_mark::hit!(enum_plain_qualified_use_tree);
                    e.variants(ctx.db)
                        .into_iter()
                        .for_each(|variant| acc.add_enum_variant(ctx, variant, None));
                }
                _ => {}
            }
        }
        // fresh use tree with leading colon2, only show crate roots
        None if !is_trivial_path => {
            cov_mark::hit!(use_tree_crate_roots_only);
            ctx.process_all_names(&mut |name, res| match res {
                ScopeDef::ModuleDef(hir::ModuleDef::Module(m)) if m.is_crate_root(ctx.db) => {
                    acc.add_resolution(ctx, name, res);
                }
                _ => (),
            });
        }
        // only show modules in a fresh UseTree
        None => {
            cov_mark::hit!(unqualified_path_only_modules_in_import);
            ctx.process_all_names(&mut |name, res| {
                if let ScopeDef::ModuleDef(hir::ModuleDef::Module(_)) = res {
                    acc.add_resolution(ctx, name, res);
                }
            });
            ["self::", "super::", "crate::"].into_iter().for_each(|kw| acc.add_keyword(ctx, kw));
        }
    }
}
