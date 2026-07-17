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

fn field_write_type_meta(field: &Field) -> TokenStream {
    if field.skip.is_some() {
        quote! {
            wincode::TypeMeta::Static {
                size: 0,
                zero_copy: false,
            }
        }
    } else {
        let target = field.target_resolved();
        quote! {
            <#target as wincode::SchemaWrite<wincode::config::DefaultConfig>>::TYPE_META
        }
    }
}

pub(crate) fn generate(input: DeriveInput) -> Result<TokenStream> {
    let args = Args::from_derive_input(&input)?;
    if let Data::Enum(variants) = &args.data {
        validate_variant_tags(variants)?;
    }
    let crate_name = args.get_crate_name();
    let ident = &args.ident;
    let (_, ty_generics, _) = args.generics.split_for_impl();
    let mut impl_generics = args.generics.clone();
    {
        let where_clause = impl_generics.make_where_clause();
        // SERIALIZED_SIZE and the runtime schema consult the container's own TYPE_META. Keep those
        // requirements explicit because wincode's implementations may have bounds beyond the
        // field adapters below, particularly for generic fields annotated with `skip` or `with`.
        where_clause.predicates.push(parse_quote!(
            #ident #ty_generics:
                wincode::SchemaWrite<wincode::config::DefaultConfig>
        ));
        where_clause.predicates.push(parse_quote!(
            for<'__wincode_dynamic_de> #ident #ty_generics:
                wincode::SchemaRead<
                    '__wincode_dynamic_de,
                    wincode::config::DefaultConfig,
                >
        ));
        let mut add_field_bounds = |field: &Field| {
            if field.skip.is_some() {
                return;
            }
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
            // The runtime schema above is read-oriented, but SERIALIZED_SIZE describes bytes
            // produced by the field's effective serialization type.
            let target = field.target_resolved();
            where_clause.predicates.push(parse_quote!(
                #target: wincode::SchemaWrite<
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
    let tag_encoding = args
        .tag_encoding
        .as_ref()
        .map(Cow::Borrowed)
        .unwrap_or_else(|| {
            Cow::Owned(parse_quote! {
                <wincode::config::DefaultConfig as wincode::config::Config>::TagEncoding
            })
        });
    // Keep the const-evaluation machinery local to the anonymous const emitted by the derive. This
    // avoids adding derive implementation details to wincode-dynamic's public API.
    let max_serialized_size_helper = quote! {
        const fn serialized_size(
            type_meta: wincode::TypeMeta,
            fields: &[wincode::TypeMeta],
        ) -> #crate_name::SerializedSize {
            if let wincode::TypeMeta::Static { size, .. } = type_meta {
                return #crate_name::SerializedSize::Static(size);
            }

            let mut fixed_size = 0usize;
            let mut is_static = true;
            let mut field_index = 0usize;
            while field_index < fields.len() {
                match fields[field_index] {
                    wincode::TypeMeta::Static { size, .. } => {
                        fixed_size = fixed_size
                            .checked_add(size)
                            .expect("serialized size overflow");
                    }
                    wincode::TypeMeta::Dynamic => is_static = false,
                }
                field_index += 1;
            }

            if is_static {
                #crate_name::SerializedSize::Static(fixed_size)
            } else {
                #crate_name::SerializedSize::Dynamic(fixed_size)
            }
        }

        const fn enum_serialized_size(
            enum_meta: wincode::TypeMeta,
            tag_meta: wincode::TypeMeta,
            variants: &[#crate_name::SerializedSize],
        ) -> #crate_name::SerializedSize {
            if let wincode::TypeMeta::Static { size, .. } = enum_meta {
                return #crate_name::SerializedSize::Static(size);
            }
            if variants.is_empty() {
                return #crate_name::SerializedSize::Static(0);
            }

            let (tag_size, mut is_static) = match tag_meta {
                wincode::TypeMeta::Static { size, .. } => (size, true),
                wincode::TypeMeta::Dynamic => (0, false),
            };
            let mut maximum_variant_size = 0usize;
            let mut variant_index = 0usize;
            while variant_index < variants.len() {
                let size = match variants[variant_index] {
                    #crate_name::SerializedSize::Static(size) => size,
                    #crate_name::SerializedSize::Dynamic(size) => {
                        is_static = false;
                        size
                    }
                };
                if size > maximum_variant_size {
                    maximum_variant_size = size;
                }
                variant_index += 1;
            }

            let size = tag_size
                .checked_add(maximum_variant_size)
                .expect("serialized size overflow");
            if is_static {
                #crate_name::SerializedSize::Static(size)
            } else {
                #crate_name::SerializedSize::Dynamic(size)
            }
        }
    };

    // SchemaWrite owns the serialized representation. SchemaRead currently reports the same sizes
    // for wincode's primitive types, but using SchemaWrite makes the contract explicit.
    let max_serialized_size = match &args.data {
        Data::Struct(fields) => {
            let field_type_metas = fields.iter().map(field_write_type_meta);
            quote! {
                serialized_size(
                    <#ident #ty_generics as wincode::SchemaWrite<wincode::config::DefaultConfig>>::TYPE_META,
                    &[#(#field_type_metas),*],
                )
            }
        }
        Data::Enum(variants) => {
            // Wincode marks an enum Dynamic when its variants have different sizes. Aggregate the
            // sequential fields within each variant, then retain the largest known contribution.
            let variant_sizes = variants.iter().map(|variant| {
                let field_type_metas = variant.fields.iter().map(field_write_type_meta);
                quote! {
                    serialized_size(
                        wincode::TypeMeta::Dynamic,
                        &[#(#field_type_metas),*],
                    )
                }
            });

            quote! {
                enum_serialized_size(
                    <#ident #ty_generics as wincode::SchemaWrite<wincode::config::DefaultConfig>>::TYPE_META,
                    <#tag_encoding as wincode::SchemaWrite<wincode::config::DefaultConfig>>::TYPE_META,
                    &[#(#variant_sizes),*],
                )
            }
        }
    };

    let schema = match &args.data {
        Data::Struct(fields) => {
            let f = fields
                .iter()
                .enumerate()
                .filter(|(_, field)| field.skip.is_none())
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
                    .filter(|(_, field)| field.skip.is_none())
                    .map(|(index, field)| field_to_tokens(&crate_name, field, index));
                let field_sizes = variant
                    .fields
                    .iter()
                    .filter(|field| field.skip.is_none())
                    .map(|field| {
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
                const SERIALIZED_SIZE: #crate_name::SerializedSize = #max_serialized_size;

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
    fn parses_wincode_field_attributes() {
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
        assert!(field.skip.is_some());
        assert!(field.with.is_none());
    }

    #[test]
    fn resolves_wincode_with_inference() {
        let input = parse_quote! {
            struct Message {
                #[wincode(with = "Adapter<_>")]
                value: Vec<u64>,
            }
        };

        let args = Args::from_derive_input(&input).unwrap();
        let Data::Struct(fields) = args.data else {
            panic!("expected struct");
        };
        let field = fields.iter().next().unwrap();
        assert_eq!(field.target_resolved(), parse_quote!(Adapter<u64>));
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
