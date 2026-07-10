use {
    darling::{
        FromDeriveInput, FromField, FromVariant,
        ast::{Data, Fields},
    },
    proc_macro::TokenStream,
    syn::{DeriveInput, Generics, Ident, Path, Type, parse_macro_input, parse_quote},
};
mod wincode_dynamic;

pub(crate) type ImplBody = Data<Variant, Field>;

#[derive(FromVariant)]
#[darling(attributes(wincode_dynamic), forward_attrs)]
#[expect(unused)]
pub(crate) struct Variant {
    pub(crate) ident: Ident,
    pub(crate) fields: Fields<Field>,
}

#[derive(FromField)]
#[darling(attributes(wincode_dynamic), forward_attrs)]
pub(crate) struct Field {
    pub(crate) ident: Option<Ident>,
    pub(crate) ty: Type,
}

#[derive(FromDeriveInput)]
#[darling(attributes(wincode_dynamic), forward_attrs)]
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
    /// Useful when the crate is renamed in `Cargo.toml` or re-exported from another module.
    /// The path is emitted as written and resolved from the derive expansion site.
    #[darling(rename = "crate", default)]
    pub(crate) crate_path: Option<Path>,
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

#[proc_macro_derive(SchemaDynamic, attributes(wincode_dynamic))]
pub fn derive_wincode_dynamic(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match wincode_dynamic::generate(input) {
        Ok(tokens) => tokens.into(),
        Err(e) => e.write_errors().into(),
    }
}
