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
            // The runtime schema above is read-oriented, but MAX_SERIALIZED_SIZE describes
            // bytes produced by serialization and therefore uses SchemaWrite metadata.
            where_clause.predicates.push(parse_quote!(
                #ty: wincode::SchemaWrite<
                    wincode::config::DefaultConfig,
                    Src = #ty,
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
    // SchemaWrite owns the serialized representation. SchemaRead currently reports the same
    // sizes for wincode's primitive types, but using SchemaWrite makes the contract explicit.
    let type_meta_max_serialized_size = quote! {
        match <#ident #ty_generics as wincode::SchemaWrite<wincode::config::DefaultConfig>>::TYPE_META {
            wincode::TypeMeta::Static { size, .. } => Some(size),
            wincode::TypeMeta::Dynamic => None,
        }
    };
    let (max_serialized_size_helper, max_serialized_size) = match &args.data {
        Data::Struct(_) => (quote! {}, Cow::Borrowed(&type_meta_max_serialized_size)),
        Data::Enum(variants) => {
            // `with` and `skip` can make a field's Rust type differ from its wire encoding. In
            // that case, only the enum's own TYPE_META is authoritative; do not infer through
            // the raw field types.
            let has_field_overrides = variants
                .iter()
                .any(|variant| variant.fields.iter().any(|field| !field.attrs.is_empty()));

            if variants.is_empty() || has_field_overrides {
                (quote! {}, Cow::Borrowed(&type_meta_max_serialized_size))
            } else {
                // Wincode marks an enum Dynamic when its otherwise-static variants have different
                // sizes. Aggregate the sequential fields within each variant so the helper below
                // can select the largest variant instead of losing that finite upper bound.
                let variant_type_metas = variants.iter().map(|variant| {
                    let field_type_metas = variant.fields.iter().map(|field| {
                        let ty = &field.ty;
                        quote! {
                            <#ty as wincode::SchemaWrite<wincode::config::DefaultConfig>>::TYPE_META
                        }
                    });

                    quote! {
                        wincode::TypeMeta::join_types([#(#field_type_metas),*])
                    }
                });

                // Keep the const-evaluation machinery local to the anonymous const emitted by the
                // derive. This avoids adding implementation details to wincode-dynamic's public API.
                let helper = quote! {
                    const fn enum_max_serialized_size(
                        enum_meta: wincode::TypeMeta,
                        tag_meta: wincode::TypeMeta,
                        variants: &[wincode::TypeMeta],
                    ) -> Option<usize> {
                        if let wincode::TypeMeta::Static { size, .. } = enum_meta {
                            return Some(size);
                        }
                        if variants.is_empty() {
                            return None;
                        }

                        let wincode::TypeMeta::Static { size: tag_size, .. } = tag_meta else {
                            return None;
                        };
                        let mut maximum_variant_size = 0usize;
                        let mut variant_index = 0usize;

                        while variant_index < variants.len() {
                            let wincode::TypeMeta::Static { size, .. } = variants[variant_index] else {
                                return None;
                            };
                            if size > maximum_variant_size {
                                maximum_variant_size = size;
                            }
                            variant_index += 1;
                        }

                        tag_size.checked_add(maximum_variant_size)
                    }
                };
                let size = quote! {
                    enum_max_serialized_size(
                        <#ident #ty_generics as wincode::SchemaWrite<wincode::config::DefaultConfig>>::TYPE_META,
                        <#tag_encoding as wincode::SchemaWrite<wincode::config::DefaultConfig>>::TYPE_META,
                        &[#(#variant_type_metas),*],
                    )
                };

                (helper, Cow::Owned(size))
            }
        }
    };

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
            #max_serialized_size_helper

            impl #impl_generics #crate_name::SchemaDynamic for #ident #ty_generics #where_clause {
                const MAX_SERIALIZED_SIZE: Option<usize> = #max_serialized_size;

                #[inline]
                fn schema() -> #crate_name::RootSchema {
                   #schema
                }
            }
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
    fn forwards_only_wincode_field_attributes() {
        let input = parse_quote! {
            enum Message {
                Item(
                    #[allow(dead_code)]
                    #[wincode(skip)]
                    u64,
                ),
            }
        };

        let args = Args::from_derive_input(&input).unwrap();
        let Data::Enum(variants) = args.data else {
            panic!("expected enum");
        };
        let field = variants[0].fields.iter().next().unwrap();
        assert_eq!(field.attrs.len(), 1);
        assert!(field.attrs[0].path().is_ident("wincode"));
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
