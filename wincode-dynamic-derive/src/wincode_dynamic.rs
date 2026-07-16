use {
    crate::{Args, Field, Variant},
    darling::{FromDeriveInput, Result, ast::Data},
    proc_macro2::{Span, TokenStream},
    quote::quote,
    std::borrow::Cow,
    syn::{DeriveInput, LitStr, Path, parse_quote},
};

fn validate_variant_tags(variants: &[Variant]) -> Result<()> {
    for variant in variants {
        if let Some(tag) = &variant.tag {
            return Err(darling::Error::custom(
                "#[wincode(tag = ...)] is not supported by SchemaDynamic; RootSchema stores enum \
                 variants by declaration index",
            )
            .with_span(tag));
        }
    }

    Ok(())
}

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
       #crate_name::FieldDef::new(
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
    if let Data::Enum(variants) = &args.data {
        validate_variant_tags(variants)?;
    }
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
    let tag_encoding = args
        .tag_encoding
        .as_ref()
        .map(Cow::Borrowed)
        .unwrap_or_else(|| {
            Cow::Owned(parse_quote! {
                <wincode::config::DefaultConfig as wincode::config::Config>::TagEncoding
            })
        });
    let inferred_max_serialized_size = match &args.data {
        Data::Struct(_) => quote! {
            match <#ident #ty_generics as wincode::SchemaRead<wincode::config::DefaultConfig>>::TYPE_META {
                wincode::TypeMeta::Static { size, .. } => Some(size),
                wincode::TypeMeta::Dynamic => None,
            }
        },
        Data::Enum(variants) => {
            let variant_sizes = variants.iter().map(|variant| {
                let field_sizes = variant.fields.iter().map(|field| {
                    let ty = &field.ty;
                    quote! {
                        match <#ty as wincode::SchemaRead<wincode::config::DefaultConfig>>::TYPE_META {
                            wincode::TypeMeta::Static { size, .. } => {
                                variant_size = match variant_size.checked_add(size) {
                                    Some(size) => size,
                                    None => panic!("maximum serialized size overflow"),
                                };
                            }
                            wincode::TypeMeta::Dynamic => variant_is_static = false,
                        }
                    }
                });

                quote! {
                    {
                        #[allow(unused_mut)]
                        let mut variant_size = 0usize;
                        #[allow(unused_mut)]
                        let mut variant_is_static = true;
                        #(#field_sizes)*
                        if variant_is_static {
                            if variant_size > maximum_variant_size {
                                maximum_variant_size = variant_size;
                            }
                        } else {
                            all_variants_are_static = false;
                        }
                    }
                }
            });

            quote! {
                match <#ident #ty_generics as wincode::SchemaRead<wincode::config::DefaultConfig>>::TYPE_META {
                    wincode::TypeMeta::Static { size, .. } => Some(size),
                    wincode::TypeMeta::Dynamic => {
                        match <#tag_encoding as wincode::SchemaRead<wincode::config::DefaultConfig>>::TYPE_META {
                            wincode::TypeMeta::Static { size: tag_size, .. } => {
                                #[allow(unused_mut)]
                                let mut maximum_variant_size = 0usize;
                                #[allow(unused_mut)]
                                let mut all_variants_are_static = true;
                                #(#variant_sizes)*
                                if all_variants_are_static {
                                    Some(match tag_size.checked_add(maximum_variant_size) {
                                        Some(size) => size,
                                        None => panic!("maximum serialized size overflow"),
                                    })
                                } else {
                                    None
                                }
                            }
                            wincode::TypeMeta::Dynamic => None,
                        }
                    }
                }
            }
        }
    };
    let max_serialized_size = if let Some(max_serialized_size) = &args.max_serialized_size {
        quote! {
            match #inferred_max_serialized_size {
                Some(_) => {
                    panic!("max_serialized_size must not be set when the maximum can be inferred")
                }
                None => #max_serialized_size,
            }
        }
    } else {
        quote! {
            match #inferred_max_serialized_size {
                Some(size) => size,
                None => #crate_name::UNBOUNDED_SERIALIZED_SIZE,
            }
        }
    };
    // Force evaluation for concrete types so an invalid size declaration fails
    // where the type is defined. Generic types are evaluated when their
    // associated constant is used for a concrete instantiation.
    let validate_max_serialized_size = args.generics.params.is_empty().then(|| {
        quote! {
            const _: usize =
                <#ident as #crate_name::SchemaDynamic>::MAX_SERIALIZED_SIZE;
        }
    });

    let schema = match &args.data {
        Data::Struct(fields) => {
            let f = fields
                .iter()
                .enumerate()
                .map(|(index, field)| field_to_tokens(&crate_name, field, index));

            quote! {
                #crate_name::RootSchema::Struct(#crate_name::Schema::new(
                    stringify!(#ident),
                    [#(#f),*].into(),
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
                        [#(#fields),*].into(),
                        Some(0usize)#(#field_sizes)*,
                    )
                }
            });

            quote! {
                #crate_name::RootSchema::Enum {
                    name: stringify!(#ident).into(),
                    variants: [#(#variants),*].into(),
                    size: match <#ident #ty_generics as wincode::SchemaRead<wincode::config::DefaultConfig>>::TYPE_META {
                        wincode::TypeMeta::Static { size, .. } => Some(size),
                        wincode::TypeMeta::Dynamic => None,
                    },
                    tag_encoding: <#tag_encoding as #crate_name::DynPrimitiveTy>::TYPE,
                }
            }
        }
    };

    Ok(quote! {
        const _: () = {
            impl #impl_generics #crate_name::SchemaDynamic for #ident #ty_generics #where_clause {
                const MAX_SERIALIZED_SIZE: usize = #max_serialized_size;

                #[inline]
                fn schema() -> #crate_name::RootSchema {
                   #schema
                }
            }

            #validate_max_serialized_size
        };
    })
}

#[cfg(test)]
mod tests {
    use {super::*, syn::parse_quote};

    #[test]
    fn accepts_enum_without_variant_tags() {
        let input = parse_quote! {
            enum Message {
                First,
                Second,
            }
        };

        assert!(generate(input).is_ok());
    }

    #[test]
    fn rejects_variant_tag() {
        let input = parse_quote! {
            enum Message {
                #[wincode(tag = 0)]
                First,
                Second,
            }
        };

        let error = generate(input).unwrap_err().write_errors().to_string();
        assert!(
            error.contains("#[wincode(tag = ...)] is not supported by SchemaDynamic"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn rejects_variant_tag_expression() {
        let input = parse_quote! {
            enum Message {
                #[wincode(tag = FIRST_TAG)]
                First,
            }
        };

        let error = generate(input).unwrap_err().write_errors().to_string();
        assert!(
            error.contains("#[wincode(tag = ...)] is not supported by SchemaDynamic"),
            "unexpected error: {error}"
        );
    }
}
