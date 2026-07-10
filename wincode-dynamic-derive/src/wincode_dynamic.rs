use {
    crate::Args,
    darling::{FromDeriveInput, Result, ast::Data},
    proc_macro2::TokenStream,
    quote::quote,
    syn::DeriveInput,
};

pub(crate) fn generate(input: DeriveInput) -> Result<TokenStream> {
    let args = Args::from_derive_input(&input)?;
    let crate_name = args.get_crate_name();
    let (impl_generics, ty_generics, where_clause) = args.generics.split_for_impl();
    let ident = &args.ident;

    let header = match &args.data {
        Data::Struct(fields) => fields.iter().map(|field| {
            let ty = &field.ty;
            let ident = &field.ident;
            quote! {
               #crate_name::Field::new(
                   stringify!(#ident),
                   <#ty as #crate_name::DynTy>::TYPE,
                   match <#ty as wincode::SchemaRead<wincode::config::DefaultConfig>>::TYPE_META {
                       wincode::TypeMeta::Static { size, .. } => Some(size),
                       _ => None,
                   }
               )
            }
        }),
        Data::Enum(_) => return Err(darling::Error::custom("enums unsupported")),
    };

    Ok(quote! {
        const _: () = {
            impl #impl_generics #crate_name::SchemaDynamic for #ident #ty_generics #where_clause {
                #[inline]
                fn schema() -> #crate_name::Schema {
                    #crate_name::Schema::new(
                        stringify!(#ident),
                        Vec::from([#(#header),*]).into_boxed_slice(),
                        match <#ident as wincode::SchemaRead<wincode::config::DefaultConfig>>::TYPE_META {
                            wincode::TypeMeta::Static { size, .. } => Some(size),
                            _ => None,
                        }
                    )
                }
            }
        };
    })
}
