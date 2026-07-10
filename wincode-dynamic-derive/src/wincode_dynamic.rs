use {
    crate::{Args, Field},
    darling::{FromDeriveInput, Result, ast::Data},
    proc_macro2::{Span, TokenStream},
    quote::quote,
    syn::{DeriveInput, LitStr, Path, parse_quote},
};

fn field_to_tokens(crate_name: &Path, field: &Field, index: usize) -> TokenStream {
    let ty = &field.ty;
    let name = match &field.ident {
        Some(ident) => quote!(stringify!(#ident)),
        None => {
            let index = LitStr::new(&index.to_string(), Span::call_site());
            quote!(#index)
        }
    };

    quote! {
       #crate_name::Field::new(
           #name,
           <#ty as #crate_name::DynTy>::TYPE,
           match <#ty as wincode::SchemaRead<wincode::config::DefaultConfig>>::TYPE_META {
               wincode::TypeMeta::Static { size, .. } => Some(size),
               _ => None,
           }
       )
    }
}

pub(crate) fn generate(input: DeriveInput) -> Result<TokenStream> {
    let args = Args::from_derive_input(&input)?;
    let crate_name = args.get_crate_name();
    let mut impl_generics = args.generics.clone();
    {
        let where_clause = impl_generics.make_where_clause();
        let mut add_field_bounds = |field: &Field| {
            let ty = &field.ty;
            where_clause
                .predicates
                .push(parse_quote!(#ty: #crate_name::DynTy));
            where_clause.predicates.push(parse_quote!(
                for<'__wincode_dynamic_de> #ty:
                    wincode::SchemaRead<
                        '__wincode_dynamic_de,
                        wincode::config::DefaultConfig,
                        Dst = #ty,
                    >
            ));
        };

        match &args.data {
            Data::Struct(fields) => fields.iter().for_each(&mut add_field_bounds),
            Data::Enum(variants) => variants
                .iter()
                .flat_map(|variant| variant.fields.iter())
                .for_each(add_field_bounds),
        }
    }
    let (impl_generics, _, where_clause) = impl_generics.split_for_impl();
    let (_, ty_generics, _) = args.generics.split_for_impl();
    let ident = &args.ident;

    let schema = match &args.data {
        Data::Struct(fields) => {
            let f = fields
                .iter()
                .enumerate()
                .map(|(index, field)| field_to_tokens(&crate_name, field, index));

            quote! {
                #crate_name::RootSchema::Struct(#crate_name::Schema::new(
                    stringify!(#ident),
                    Vec::from([#(#f),*]).into_boxed_slice(),
                    match <#ident #ty_generics as wincode::SchemaRead<wincode::config::DefaultConfig>>::TYPE_META {
                        wincode::TypeMeta::Static { size, .. } => Some(size),
                        _ => None,
                    }
                ))
            }
        }
        Data::Enum(variants) => {
            let variants = variants.iter().map(|variant| {
                let variant_ident = &variant.ident;
                let fields = variant
                    .fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| field_to_tokens(&crate_name, field, index));
                let field_sizes = variant.fields.iter().map(|field| {
                    let ty = &field.ty;
                    quote! {
                        .and_then(|total| {
                            match <#ty as wincode::SchemaRead<wincode::config::DefaultConfig>>::TYPE_META {
                                wincode::TypeMeta::Static { size, .. } => total.checked_add(size),
                                wincode::TypeMeta::Dynamic => None,
                            }
                        })
                    }
                });

                quote! {
                    #crate_name::Schema::new(
                        stringify!(#variant_ident),
                        Vec::from([#(#fields),*]).into_boxed_slice(),
                        Some(0usize)#(#field_sizes)*,
                    )
                }
            });

            quote! {
                #crate_name::RootSchema::Enum {
                    name: stringify!(#ident).into(),
                    variants: Vec::from([#(#variants),*]).into_boxed_slice(),
                    size: match <#ident #ty_generics as wincode::SchemaRead<wincode::config::DefaultConfig>>::TYPE_META {
                        wincode::TypeMeta::Static { size, .. } => Some(size),
                        wincode::TypeMeta::Dynamic => None,
                    },
                }
            }
        }
    };

    Ok(quote! {
        const _: () = {
            impl #impl_generics #crate_name::SchemaDynamic for #ident #ty_generics #where_clause {
                #[inline]
                fn schema() -> #crate_name::RootSchema {
                   #schema
                }
            }
        };
    })
}
