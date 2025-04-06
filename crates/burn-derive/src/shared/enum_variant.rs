use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{FieldsNamed, Variant};

/// Process a variant of an enum where the output is the result of the given mapper.
pub(crate) fn map_enum_variant<Mapper>(
    variant: &Variant,
    mapper: Mapper,
) -> (TokenStream, TokenStream)
where
    Mapper: Fn(&Ident) -> TokenStream,
{
    let gen_fields_unnamed = |num: usize| {
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();

        for i in 0..num {
            let arg_name = Ident::new(&format!("arg_{i}"), Span::call_site());
            let input = quote! { #arg_name };
            let output = mapper(&arg_name);

            inputs.push(input);
            outputs.push(output);
        }

        (quote! (( #(#inputs),* )), quote! (( #(#outputs),* )))
    };
    let gen_fields_named = |fields: &FieldsNamed| {
        let mut inputs = Vec::new();
        let mut outputs = Vec::new();

        fields.named.iter().for_each(|field| {
            let ident = field.ident.as_ref().expect("Named field to have a name.");
            let input = quote! { #ident };
            let output = mapper(ident);

            inputs.push(input);
            outputs.push(quote! {
                #ident: #output
            });
        });

        (quote! {{ #(#inputs),* }}, quote! {{ #(#outputs),* }})
    };

    match &variant.fields {
        syn::Fields::Named(fields) => gen_fields_named(fields),
        syn::Fields::Unnamed(_) => gen_fields_unnamed(variant.fields.len()),
        syn::Fields::Unit => (quote! {}, quote! {}),
    }
}

/// An enum variant (simplified).
pub(crate) struct EnumVariant {
    pub ident: syn::Ident,
    pub ty: syn::Type,
}
pub(crate) fn parse_variants(ast: &syn::DeriveInput) -> Vec<EnumVariant> {
    let mut variants = Vec::new();

    if let syn::Data::Enum(enum_data) = &ast.data {
        for variant in enum_data.variants.iter() {
            if variant.fields.len() != 1 {
                // No support for unit variants or variants with multiple fields
                panic!("Enums are only supported for one field type")
            }

            let field = variant.fields.iter().next().unwrap();

            variants.push(EnumVariant {
                ident: variant.ident.clone(),
                ty: field.ty.clone(),
            });
        }
    } else {
        panic!("Only enum can be derived")
    }

    variants
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::{Ident, TokenStream};
    use quote::quote;
    use syn::parse_quote;

    #[test]
    fn test_map_enum_variant_with_unnamed_fields() {
        // Variant from Option enum: Some(T)
        // Arrange
        let variant: syn::Variant = parse_quote! {
            Some(String)
        };

        // Simple mapper that wraps each field in a cloning operation
        let mapper = |ident: &Ident| -> TokenStream {
            quote! { #ident.clone() }
        };

        // Act
        let (inputs, outputs) = map_enum_variant(&variant, mapper);

        // Assert
        let expected_inputs = quote! { (arg_0) };
        let expected_outputs = quote! { (arg_0.clone()) };

        assert_eq!(inputs.to_string(), expected_inputs.to_string());
        assert_eq!(outputs.to_string(), expected_outputs.to_string());
    }

    #[test]
    fn test_map_enum_variant_with_named_fields() {
        // Variant from Shape enum: Rectangle { width: f32, height: f32 }
        // Arrange
        let variant: syn::Variant = parse_quote! {
            Rectangle { width: f32, height: f32 }
        };

        // Mapper that applies validation to dimensions
        let mapper = |ident: &Ident| -> TokenStream {
            quote! { validate_dimension(#ident) }
        };

        // Act
        let (inputs, outputs) = map_enum_variant(&variant, mapper);

        // Assert
        let expected_inputs = quote! { { width, height } };
        let expected_outputs = quote! { { width: validate_dimension(width), height: validate_dimension(height) } };

        assert_eq!(inputs.to_string(), expected_inputs.to_string());
        assert_eq!(outputs.to_string(), expected_outputs.to_string());
    }

    #[test]
    fn test_map_enum_variant_with_unit_variant() {
        // Variant from Option enum: None
        // Arrange
        let variant: syn::Variant = parse_quote! {
            None
        };

        // Mapper is not used for unit variants but still required
        let mapper = |ident: &Ident| -> TokenStream {
            quote! { #ident }
        };

        // Act
        let (inputs, outputs) = map_enum_variant(&variant, mapper);

        // Assert
        let expected_inputs = quote! {};
        let expected_outputs = quote! {};

        assert_eq!(inputs.to_string(), expected_inputs.to_string());
        assert_eq!(outputs.to_string(), expected_outputs.to_string());
    }
}