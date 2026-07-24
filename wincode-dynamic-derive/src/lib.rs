use {
    darling::{
        FromDeriveInput, FromField, FromMeta, FromVariant,
        ast::{Data, Fields},
    },
    proc_macro::TokenStream,
    std::collections::VecDeque,
    syn::{
        DeriveInput, Expr, GenericArgument, Generics, Ident, Path, Type, parse_macro_input,
        parse_quote,
        visit::{self, Visit},
        visit_mut::{self, VisitMut},
    },
};
mod wincode_dynamic;

pub(crate) type ImplBody = Data<Variant, Field>;

#[derive(FromVariant)]
#[darling(attributes(wincode_dynamic, wincode), forward_attrs)]
pub(crate) struct Variant {
    pub(crate) ident: Ident,
    pub(crate) fields: Fields<Field>,
    #[darling(default)]
    pub(crate) tag: Option<Expr>,
}

#[derive(FromMeta)]
#[darling(from_word = || Ok(Self::Default))]
#[allow(dead_code)] // The initializer only needs to be parsed so we accept wincode's full syntax.
pub(crate) enum SkipMode {
    Default,
    DefaultVal(Expr),
}

#[derive(FromField)]
#[darling(attributes(wincode_dynamic, wincode))]
pub(crate) struct Field {
    pub(crate) ident: Option<Ident>,
    pub(crate) ty: Type,
    #[darling(default)]
    pub(crate) with: Option<Type>,
    pub(crate) skip: Option<SkipMode>,
}

impl Field {
    /// Return the field's wire type, resolving wincode's `_` shorthand from the
    /// Rust field type.
    pub(crate) fn target_resolved(&self) -> Type {
        let Some(with) = &self.with else {
            return self.ty.clone();
        };
        let mut target = with.clone();
        let mut generic_types = GenericTypes::default();
        generic_types.visit_type(&self.ty);
        if generic_types.0.is_empty() {
            generic_types.0.push_back(self.ty.clone());
        }
        InferGeneric(generic_types.0).visit_type_mut(&mut target);
        target
    }
}

/// Generic arguments collected in the same order wincode uses to resolve
/// adapter placeholders.
#[derive(Default)]
struct GenericTypes(VecDeque<Type>);

impl<'ast> Visit<'ast> for GenericTypes {
    fn visit_generic_argument(&mut self, argument: &'ast GenericArgument) {
        if let GenericArgument::Type(ty) = argument {
            match ty {
                Type::Slice(slice) => {
                    self.0.push_back((*slice.elem).clone());
                    return;
                }
                Type::Array(array) => {
                    self.0.push_back((*array.elem).clone());
                    return;
                }
                Type::Path(path)
                    if path.path.segments.iter().any(|segment| {
                        matches!(segment.arguments, syn::PathArguments::AngleBracketed(_))
                    }) => {}
                _ => self.0.push_back(ty.clone()),
            }
        }

        visit::visit_generic_argument(self, argument);
    }
}

/// Replaces adapter inference placeholders with the collected field types.
struct InferGeneric(VecDeque<Type>);

impl VisitMut for InferGeneric {
    fn visit_generic_argument_mut(&mut self, argument: &mut GenericArgument) {
        if let GenericArgument::Type(Type::Infer(_)) = argument {
            *argument = GenericArgument::Type(
                self.0
                    .pop_front()
                    .expect("wincode-dynamic: not enough field types to resolve `_`"),
            );
        }
        visit_mut::visit_generic_argument_mut(self, argument);
    }

    fn visit_type_array_mut(&mut self, array: &mut syn::TypeArray) {
        if let Type::Infer(_) = &*array.elem {
            *array.elem = self
                .0
                .pop_front()
                .expect("wincode-dynamic: not enough field types to resolve `_`");
        }
        visit_mut::visit_type_array_mut(self, array);
    }
}

#[derive(FromDeriveInput)]
#[darling(attributes(wincode_dynamic, wincode), forward_attrs)]
pub(crate) struct Args {
    pub(crate) ident: Ident,
    pub(crate) generics: Generics,
    pub(crate) data: ImplBody,

    /// Helper to determine the crate path.
    ///
    /// If `internal` is `true`, the generated code will use the `crate::` path.
    /// Otherwise, it will use the default crate name path.
    #[darling(default)]
    pub(crate) internal: bool,
    /// Specifies the path to the crate.
    ///
    /// Useful when the crate is renamed in `Cargo.toml` or re-exported from
    /// another module. The path is emitted as written and resolved from the
    /// derive expansion site.
    #[darling(rename = "crate", default)]
    pub(crate) crate_path: Option<Path>,
    #[darling(default)]
    pub(crate) tag_encoding: Option<Type>,
}

impl Args {
    pub(crate) fn get_crate_name(&self) -> Path {
        if let Some(crate_path) = &self.crate_path {
            crate_path.clone()
        } else if self.internal {
            parse_quote!(crate)
        } else {
            parse_quote!(::wincode_dynamic)
        }
    }
}

#[proc_macro_derive(SchemaDynamic, attributes(wincode_dynamic, wincode))]
pub fn derive_wincode_dynamic(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match wincode_dynamic::generate(input) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.write_errors().into(),
    }
}
