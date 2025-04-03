use super::ConfigAnalyzer;
use crate::shared::{attribute::AttributeItem, field::FieldTypeAnalyzer};
use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub struct ConfigStructAnalyzer {
    name: Ident,
    fields_required: Vec<FieldTypeAnalyzer>,
    fields_option: Vec<FieldTypeAnalyzer>,
    fields_default: Vec<(FieldTypeAnalyzer, AttributeItem)>,
}

impl ConfigStructAnalyzer {
    pub fn new(
        name: Ident,
        fields_required: Vec<FieldTypeAnalyzer>,
        fields_option: Vec<FieldTypeAnalyzer>,
        fields_default: Vec<(FieldTypeAnalyzer, AttributeItem)>,
    ) -> Self {
        Self {
            name,
            fields_required,
            fields_option,
            fields_default,
        }
    }

    fn wrap_impl_block(&self, tokens: TokenStream) -> TokenStream {
        let name = &self.name;

        quote! {
            impl #name {
                #tokens
            }
        }
    }

    fn names(&self) -> Vec<FieldTypeAnalyzer> {
        self.fields_required.iter()
            .chain(self.fields_option.iter())
            .chain(self.fields_default.iter().map(|(field, _)| field))
            .cloned()
            .collect()
    }

    fn name_types(&self, names: &[FieldTypeAnalyzer]) -> Vec<TokenStream> {
        names.iter()
            .map(|field| {
                let name = field.ident();
                let ty = &field.field.ty;
                
                quote! {
                    #name: #ty
                }
            })
            .collect()
    }

    fn serde_struct_ident(&self) -> Ident {
        Ident::new(&format!("{}Serde", self.name), self.name.span())
    }

    fn gen_serialize_fn(
        &self,
        struct_name: &Ident,
        struct_gen: &TokenStream,
        names: &[FieldTypeAnalyzer],
    ) -> TokenStream {
        let name = &self.name;
        let names = names.iter().map(|name| {
            let name = name.ident();
            quote! { #name: self.#name.clone() }
        });

        quote! {
            impl burn::serde::Serialize for #name {

                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: burn::serde::Serializer {
                    #[derive(burn::serde::Serialize)]
                    #[serde(crate = "burn::serde")]
                    #struct_gen

                    let serde_state = #struct_name {
                        #(#names),*
                    };
                    serde_state.serialize(serializer)
                }
            }

        }
    }

    fn gen_deserialize_fn(
        &self,
        struct_name: &Ident,
        struct_gen: &TokenStream,
        names: &[FieldTypeAnalyzer],
    ) -> TokenStream {
        let name = &self.name;
        let names = names.iter().map(|name| {
            let name = name.ident();
            quote! { #name: serde_state.#name }
        });

        quote! {
            impl<'de> burn::serde::Deserialize<'de> for #name {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: burn::serde::Deserializer<'de> {
                    #[derive(burn::serde::Deserialize)]
                    #[serde(crate = "burn::serde")]
                    #struct_gen

                    let serde_state = #struct_name::deserialize(deserializer)?;
                    Ok(#name {
                        #(#names),*
                    })
                }
            }

        }
    }

    fn gen_serde_struct(&self, names: &[TokenStream]) -> TokenStream {
        let struct_name = self.serde_struct_ident();

        quote! {
            struct #struct_name {
                #(#names),*
            }

        }
    }
}

impl ConfigAnalyzer for ConfigStructAnalyzer {
    fn gen_new_fn(&self) -> TokenStream {
        let mut body = quote! {};
        let mut names = Vec::new();

        for field in self.fields_required.iter() {
            let name = field.ident();
            let ty = &field.field.ty;

            body.extend(quote! {
                #name: #name,
            });
            names.push(quote! {
                #name: #ty
            });
        }

        for field in self.fields_option.iter() {
            let name = field.ident();

            body.extend(quote! {
                #name: None,
            });
        }

        for (field, attribute) in self.fields_default.iter() {
            let name = field.ident();
            let value = &attribute.value;
            match value {
                syn::Lit::Str(value) => {
                    let stream: proc_macro2::TokenStream = value.value().parse().unwrap();

                    body.extend(quote! {
                        #name: #stream,
                    });
                }
                _ => {
                    body.extend(quote! {
                        #name: #value,
                    });
                }
            };
        }

        let body = quote! {
            /// Create a new instance of the config.
            pub fn new(
                #(#names),*
            ) -> Self {
                Self { #body }
            }
        };
        self.wrap_impl_block(body)
    }

    fn gen_builder_fns(&self) -> TokenStream {
        let mut body = quote! {};

        for (field, _) in self.fields_default.iter() {
            let name = field.ident();
            let doc = field.doc().unwrap_or_else(|| {
                quote! {
                        /// Set the default value for the field.
                }
            });
            let ty = &field.field.ty;
            let fn_name = Ident::new(&format!("with_{name}"), name.span());

            body.extend(quote! {
                #doc
                pub fn #fn_name(mut self, #name: #ty) -> Self {
                    self.#name = #name;
                    self
                }
            });
        }

        for field in self.fields_option.iter() {
            let name = field.ident();
            let ty = &field.field.ty;
            let fn_name = Ident::new(&format!("with_{name}"), name.span());

            body.extend(quote! {
                /// Set the default value for the field.
                pub fn #fn_name(mut self, #name: #ty) -> Self {
                    self.#name = #name;
                    self
                }
            });
        }

        self.wrap_impl_block(body)
    }

    fn gen_serde_impl(&self) -> TokenStream {
        let names = self.names();

        let struct_name = self.serde_struct_ident();
        let name_types = self.name_types(&names);
        let struct_gen = self.gen_serde_struct(&name_types);

        let serialize_gen = self.gen_serialize_fn(&struct_name, &struct_gen, &names);
        let deserialize_gen = self.gen_deserialize_fn(&struct_name, &struct_gen, &names);

        quote! {
            #serialize_gen
            #deserialize_gen
        }
    }

    fn gen_clone_impl(&self) -> TokenStream {
        let name = &self.name;
        let names = self.names().into_iter().map(|name| {
            let name = name.ident();
            quote! { #name: self.#name.clone() }
        });

        quote! {
            impl Clone for #name {
                fn clone(&self) -> Self {
                    Self {
                        #(#names),*
                    }
                }
            }

        }
    }

    fn gen_display_impl(&self) -> TokenStream {
        let name = &self.name;

        quote! {
            impl core::fmt::Display for #name {
                fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    f.write_str(&burn::config::config_to_json(self))
                }
            }
        }
    }

    fn gen_config_impl(&self) -> TokenStream {
        let name = &self.name;

        quote! {
            impl burn::config::Config for #name {
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proc_macro2::Span;
    use syn::{parse_quote, Attribute, Field};
    use crate::shared::attribute::AttributeAnalyzer;

    #[test]
    fn test_wrap_impl_block_simple_method() {
        // Arrange
        let analyzer = ConfigStructAnalyzer::new(
            Ident::new("TestConfig", Span::call_site()),
            vec![],
            vec![],
            vec![],
        );

        let method_tokens = quote! {
            fn test_method(&self) -> bool {
                true
            }
        };

        // Act
        let result = analyzer.wrap_impl_block(method_tokens);

        // Assert
        let expected = quote! {
            impl TestConfig {
                fn test_method(&self) -> bool {
                    true
                }
            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_wrap_impl_block_multiple_methods() {
        // Arrange
        let analyzer = ConfigStructAnalyzer::new(
            Ident::new("ExampleConfig", Span::call_site()),
            vec![],
            vec![],
            vec![],
        );

        let methods_tokens = quote! {
            fn method_one(&self) -> i32 { 42 }

            fn method_two(&mut self, value: String) {
                // Some implementation
            }
        };

        // Act
        let result = analyzer.wrap_impl_block(methods_tokens);

        // Assert
        let expected = quote! {
            impl ExampleConfig {
                fn method_one(&self) -> i32 { 42 }

                fn method_two(&mut self, value: String) {
                    // Some implementation
                }
            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_wrap_impl_block_empty_content() {
        // Arrange
        let analyzer = ConfigStructAnalyzer::new(
            Ident::new("EmptyConfig", Span::call_site()),
            vec![],
            vec![],
            vec![],
        );

        let empty_tokens = quote! {};

        // Act
        let result = analyzer.wrap_impl_block(empty_tokens);

        // Assert
        let expected = quote! {
            impl EmptyConfig {

            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_names_with_all_field_types() {
        // Create field analyzers with dummy fields
        let required_field: Field = parse_quote!(pub required_field: String);
        let option_field: Field = parse_quote!(pub option_field: Option<u32>);
        let default_field: Field = parse_quote!(#[config(default = "default_value")] pub default_field: &'static str);
        
        let required_analyzer = FieldTypeAnalyzer::new(required_field);
        let option_analyzer = FieldTypeAnalyzer::new(option_field);
        let default_analyzer = FieldTypeAnalyzer::new(default_field);
        
        // Create default attribute
        let default_attr: Attribute = parse_quote!(#[config(default = "default_value")]);
        let attr_item = AttributeAnalyzer::new(default_attr).item();
        
        // Create analyzer with all field types
        let analyzer = ConfigStructAnalyzer::new(
            Ident::new("TestConfig", Span::call_site()),
            vec![required_analyzer.clone()],
            vec![option_analyzer.clone()],
            vec![(default_analyzer.clone(), attr_item)],
        );
        
        // Act
        let names = analyzer.names();
        
        // Assert
        assert_eq!(names.len(), 3);
        assert_eq!(names[0].ident().to_string(), "required_field");
        assert_eq!(names[1].ident().to_string(), "option_field");
        assert_eq!(names[2].ident().to_string(), "default_field");
    }

    #[test]
    fn test_names_with_empty_collections() {
        // Create analyzer with empty collections
        let analyzer = ConfigStructAnalyzer::new(
            Ident::new("EmptyConfig", Span::call_site()),
            vec![],
            vec![],
            vec![],
        );
        
        // Act
        let names = analyzer.names();
        
        // Assert
        assert!(names.is_empty());
    }

    #[test]
    fn test_name_types_basic_fields() {
        // Arrange
        let analyzer = ConfigStructAnalyzer::new(
            Ident::new("TestConfig", Span::call_site()),
            vec![],
            vec![],
            vec![],
        );
        
        // Create test fields with different types
        let string_field: Field = parse_quote!(pub name: String);
        let int_field: Field = parse_quote!(pub count: i32);
        
        let fields = vec![
            FieldTypeAnalyzer::new(string_field),
            FieldTypeAnalyzer::new(int_field),
        ];
        
        // Act
        let result = analyzer.name_types(&fields);
        
        // Assert
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].to_string(), "name : String");
        assert_eq!(result[1].to_string(), "count : i32");
    }

    #[test]
    fn test_name_types_complex_types() {
        // Arrange
        let analyzer = ConfigStructAnalyzer::new(
            Ident::new("ComplexConfig", Span::call_site()),
            vec![],
            vec![],
            vec![],
        );
        
        // Create test fields with more complex types
        let option_field: Field = parse_quote!(pub maybe_value: Option<String>);
        let vec_field: Field = parse_quote!(pub items: Vec<i32>);
        let generic_field: Field = parse_quote!(pub data: HashMap<String, Vec<f64>>);
        
        let fields = vec![
            FieldTypeAnalyzer::new(option_field),
            FieldTypeAnalyzer::new(vec_field),
            FieldTypeAnalyzer::new(generic_field),
        ];
        
        // Act
        let result = analyzer.name_types(&fields);
        
        // Assert
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].to_string(), "maybe_value : Option < String >");
        assert_eq!(result[1].to_string(), "items : Vec < i32 >");
        assert_eq!(result[2].to_string(), "data : HashMap < String , Vec < f64 > >");
    }

    #[test]
    fn test_name_types_with_empty_input() {
        // Arrange
        let analyzer = ConfigStructAnalyzer::new(
            Ident::new("EmptyConfig", Span::call_site()),
            vec![],
            vec![],
            vec![],
        );
        
        // Act
        let result = analyzer.name_types(&[]);
        
        // Assert
        assert!(result.is_empty());
    }

    #[test]
    fn test_name_types_lifetimes_and_references() {
        // Arrange
        let analyzer = ConfigStructAnalyzer::new(
            Ident::new("RefConfig", Span::call_site()),
            vec![],
            vec![],
            vec![],
        );
        
        // Create test fields with lifetimes and references
        let ref_field: Field = parse_quote!(pub reference: &'static str);
        let ref_mut_field: Field = parse_quote!(pub mut_ref: &'a mut Vec<u8>);
        
        let fields = vec![
            FieldTypeAnalyzer::new(ref_field),
            FieldTypeAnalyzer::new(ref_mut_field),
        ];
        
        // Act
        let result = analyzer.name_types(&fields);
        
        // Assert
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].to_string(), "reference : & 'static str");
        assert_eq!(result[1].to_string(), "mut_ref : & 'a mut Vec < u8 >");
    }
}
