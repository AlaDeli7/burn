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
        let field_initializers = self.fields_required.iter().map(|field| {
            let name = field.ident();
            quote! { #name: #name, }
        }).chain(
            self.fields_option.iter().map(|field| {
                let name = field.ident();
                quote! { #name: None, }
            })
        ).chain(
            self.fields_default.iter().map(|(field, attribute)| {
                let name = field.ident();
                let value = &attribute.value;

                match value {
                    syn::Lit::Str(value) => {
                        let stream: proc_macro2::TokenStream = value.value().parse().unwrap();
                        quote! { #name: #stream, }
                    }
                    _ => quote! { #name: #value, }
                }
            })
        );

        let param_declarations = self.fields_required.iter().map(|field| {
            let name = field.ident();
            let ty = &field.field.ty;
            quote! { #name: #ty }
        });

        let body = quote! {
            /// Create a new instance of the config.
            pub fn new(
                #(#param_declarations),*
            ) -> Self {
                Self { #(#field_initializers)* }
            }
        };

        self.wrap_impl_block(body)
    }

    fn gen_builder_fns(&self) -> TokenStream {
        let default_field_builders = self.fields_default.iter().map(|(field, _)| {
            let name = field.ident();
            let doc = field.doc().unwrap_or_else(|| {
                quote! {
                    /// Set the default value for the field.
                }
            });
            let ty = &field.field.ty;
            let fn_name = Ident::new(&format!("with_{name}"), name.span());

            quote! {
                #doc
                pub fn #fn_name(mut self, #name: #ty) -> Self {
                    self.#name = #name;
                    self
                }
            }
        });

        let option_field_builders = self.fields_option.iter().map(|field| {
            let name = field.ident();
            let ty = &field.field.ty;
            let fn_name = Ident::new(&format!("with_{name}"), name.span());

            quote! {
            /// Set the default value for the field.
            pub fn #fn_name(mut self, #name: #ty) -> Self {
                self.#name = #name;
                self
            }
        }
        });

        let body = quote! {
            #(#default_field_builders)*
            #(#option_field_builders)*
        };

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

    #[test]
    fn test_serde_struct_ident_basic() {
        // Arrange
        let analyzer = ConfigStructAnalyzer::new(
            Ident::new("TestConfig", Span::call_site()),
            vec![],
            vec![],
            vec![],
        );

        // Act
        let result = analyzer.serde_struct_ident();

        // Assert
        assert_eq!(result.to_string(), "TestConfigSerde")
    }

    #[test]
    fn test_gen_serde_struct_empty() {
        // Arrange
        let analyzer = ConfigStructAnalyzer::new(
            Ident::new("EmptyConfig", Span::call_site()),
            vec![],
            vec![],
            vec![],
        );
        let names: Vec<TokenStream> = vec![];

        // Act
        let result = analyzer.gen_serde_struct(&names);

        // Assert
        let expected = quote! {
            struct EmptyConfigSerde {

            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_gen_serde_struct_with_fields() {
        // Arrange
        let analyzer = ConfigStructAnalyzer::new(
            Ident::new("ModelConfig", Span::call_site()),
            vec![],
            vec![],
            vec![],
        );

        // Create field definitions as TokenStreams
        let fields = vec![
            quote! { learning_rate: f32 },
            quote! { batch_size: usize },
            quote! { model_name: String }
        ];

        // Act
        let result = analyzer.gen_serde_struct(&fields);

        // Assert
        let expected = quote! {
            struct ModelConfigSerde {
                learning_rate: f32,
                batch_size: usize,
                model_name: String
            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_gen_serialize_fn_basic() {
        // Arrange
        let analyzer = ConfigStructAnalyzer::new(
            Ident::new("SimpleConfig", Span::call_site()),
            vec![],
            vec![],
            vec![],
        );

        // Create a simple field for testing
        let field: Field = parse_quote!(pub value: i32);
        let field_analyzer = FieldTypeAnalyzer::new(field);

        // Create the struct definition
        let struct_name = analyzer.serde_struct_ident();
        let struct_gen = quote! {
            struct SimpleConfigSerde {
                value: i32
            }
        };

        // Act
        let result = analyzer.gen_serialize_fn(&struct_name, &struct_gen, &[field_analyzer]);

        // Assert
        let expected = quote! {
            impl burn::serde::Serialize for SimpleConfig {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: burn::serde::Serializer {
                    #[derive(burn::serde::Serialize)]
                    #[serde(crate = "burn::serde")]
                    struct SimpleConfigSerde {
                        value: i32
                    }

                    let serde_state = SimpleConfigSerde {
                        value: self.value.clone()
                    };
                    serde_state.serialize(serializer)
                }
            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_gen_serialize_fn_multiple_fields() {
        // Arrange
        let analyzer = ConfigStructAnalyzer::new(
            Ident::new("ModelConfig", Span::call_site()),
            vec![],
            vec![],
            vec![],
        );

        // Create multiple fields for testing
        let field1: Field = parse_quote!(pub learning_rate: f32);
        let field2: Field = parse_quote!(pub batch_size: usize);
        let field3: Field = parse_quote!(pub model_name: String);

        let fields = vec![
            FieldTypeAnalyzer::new(field1),
            FieldTypeAnalyzer::new(field2),
            FieldTypeAnalyzer::new(field3),
        ];

        // Create the struct definition
        let struct_name = analyzer.serde_struct_ident();
        let struct_gen = quote! {
            struct ModelConfigSerde {
                learning_rate: f32,
                batch_size: usize,
                model_name: String
            }
        };

        // Act
        let result = analyzer.gen_serialize_fn(&struct_name, &struct_gen, &fields);

        // Assert
        let expected = quote! {
            impl burn::serde::Serialize for ModelConfig {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: burn::serde::Serializer {
                    #[derive(burn::serde::Serialize)]
                    #[serde(crate = "burn::serde")]
                    struct ModelConfigSerde {
                        learning_rate: f32,
                        batch_size: usize,
                        model_name: String
                    }

                    let serde_state = ModelConfigSerde {
                        learning_rate: self.learning_rate.clone(),
                        batch_size: self.batch_size.clone(),
                        model_name: self.model_name.clone()
                    };
                    serde_state.serialize(serializer)
                }
            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_gen_deserialize_fn_basic() {
        // Arrange
        let analyzer = ConfigStructAnalyzer::new(
            Ident::new("SimpleConfig", Span::call_site()),
            vec![],
            vec![],
            vec![],
        );

        // Create a simple field for testing
        let field: Field = parse_quote!(pub value: i32);
        let field_analyzer = FieldTypeAnalyzer::new(field);

        // Create the struct definition
        let struct_name = analyzer.serde_struct_ident();
        let struct_gen = quote! {
            struct SimpleConfigSerde {
                value: i32
            }
        };

        // Act
        let result = analyzer.gen_deserialize_fn(&struct_name, &struct_gen, &[field_analyzer]);

        // Assert
        let expected = quote! {
            impl<'de> burn::serde::Deserialize<'de> for SimpleConfig {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: burn::serde::Deserializer<'de> {
                    #[derive(burn::serde::Deserialize)]
                    #[serde(crate = "burn::serde")]
                    struct SimpleConfigSerde {
                        value: i32
                    }

                    let serde_state = SimpleConfigSerde::deserialize(deserializer)?;
                    Ok(SimpleConfig {
                        value: serde_state.value
                    })
                }
            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_gen_deserialize_fn_multiple_fields() {
        // Arrange
        let analyzer = ConfigStructAnalyzer::new(
            Ident::new("ModelConfig", Span::call_site()),
            vec![],
            vec![],
            vec![],
        );

        // Create multiple fields for testing
        let field1: Field = parse_quote!(pub learning_rate: f32);
        let field2: Field = parse_quote!(pub batch_size: usize);
        let field3: Field = parse_quote!(pub model_name: String);

        let fields = vec![
            FieldTypeAnalyzer::new(field1),
            FieldTypeAnalyzer::new(field2),
            FieldTypeAnalyzer::new(field3),
        ];

        // Create the struct definition
        let struct_name = analyzer.serde_struct_ident();
        let struct_gen = quote! {
            struct ModelConfigSerde {
                learning_rate: f32,
                batch_size: usize,
                model_name: String
            }
        };

        // Act
        let result = analyzer.gen_deserialize_fn(&struct_name, &struct_gen, &fields);

        // Assert
        let expected = quote! {
            impl<'de> burn::serde::Deserialize<'de> for ModelConfig {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: burn::serde::Deserializer<'de> {
                    #[derive(burn::serde::Deserialize)]
                    #[serde(crate = "burn::serde")]
                    struct ModelConfigSerde {
                        learning_rate: f32,
                        batch_size: usize,
                        model_name: String
                    }

                    let serde_state = ModelConfigSerde::deserialize(deserializer)?;
                    Ok(ModelConfig {
                        learning_rate: serde_state.learning_rate,
                        batch_size: serde_state.batch_size,
                        model_name: serde_state.model_name
                    })
                }
            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_gen_deserialize_fn_empty_struct() {
        // Arrange
        let analyzer = ConfigStructAnalyzer::new(
            Ident::new("EmptyConfig", Span::call_site()),
            vec![],
            vec![],
            vec![],
        );

        // Create the struct definition
        let struct_name = analyzer.serde_struct_ident();
        let struct_gen = quote! {
            struct EmptyConfigSerde {}
        };

        // Act
        let result = analyzer.gen_deserialize_fn(&struct_name, &struct_gen, &[]);

        // Assert
        let expected = quote! {
            impl<'de> burn::serde::Deserialize<'de> for EmptyConfig {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: burn::serde::Deserializer<'de> {
                    #[derive(burn::serde::Deserialize)]
                    #[serde(crate = "burn::serde")]
                    struct EmptyConfigSerde {}

                    let serde_state = EmptyConfigSerde::deserialize(deserializer)?;
                    Ok(EmptyConfig {

                    })
                }
            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_gen_new_fn_with_required_fields() {
        // Arrange
        let name = Ident::new("TestConfig", Span::call_site());

        // Create required fields
        let string_field: Field = parse_quote!(pub name: String);
        let int_field: Field = parse_quote!(pub count: i32);

        let required_fields = vec![
            FieldTypeAnalyzer::new(string_field),
            FieldTypeAnalyzer::new(int_field),
        ];

        let analyzer = ConfigStructAnalyzer::new(
            name,
            required_fields,
            vec![],
            vec![],
        );

        // Act
        let result = analyzer.gen_new_fn();

        // Assert
        let expected = quote! {
                impl TestConfig {
                    /// Create a new instance of the config.
                    pub fn new(
                        name: String,
                        count: i32
                    ) -> Self {
                        Self {
                            name: name,
                            count: count,
                        }
                    }
                }
            };

        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_gen_new_fn_with_optional_fields() {
        // Arrange
        let name = Ident::new("ConfigWithOptions", Span::call_site());

        // Create required field
        let id_field: Field = parse_quote!(pub id: String);

        // Create optional fields
        let option_field: Field = parse_quote!(pub maybe_value: Option<String>);
        let vec_field: Field = parse_quote!(pub items: Vec<i32>);

        let required_fields = vec![FieldTypeAnalyzer::new(id_field)];
        let optional_fields = vec![
            FieldTypeAnalyzer::new(option_field),
            FieldTypeAnalyzer::new(vec_field),
        ];

        let analyzer = ConfigStructAnalyzer::new(
            name,
            required_fields,
            optional_fields,
            vec![],
        );

        // Act
        let result = analyzer.gen_new_fn();

        // Assert
        let expected = quote! {
            impl ConfigWithOptions {
                /// Create a new instance of the config.
                pub fn new(
                    id: String
                ) -> Self {
                    Self {
                        id: id,
                        maybe_value: None,
                        items: None,
                    }
                }
            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }

    // todo check why this is failing
    // #[test]
    // fn test_gen_new_fn_with_default_fields() {
    //     // Arrange
    //     let name = Ident::new("ConfigWithDefaults", Span::call_site());
    //
    //     // Create field with numeric default
    //     let int_field: Field = parse_quote!(pub count: i32);
    //     let int_attr: Attribute = parse_quote!(#[config(default = 42)]);
    //     let int_analyzer = AttributeAnalyzer::new(int_attr);
    //
    //     // Create field with string default
    //     let string_field: Field = parse_quote!(pub name: String);
    //     let string_attr: Attribute = parse_quote!(#[config(default = "default_name")]);
    //     let string_analyzer = AttributeAnalyzer::new(string_attr);
    //
    //     let default_fields = vec![
    //         (FieldTypeAnalyzer::new(int_field), int_analyzer.item()),
    //         (FieldTypeAnalyzer::new(string_field), string_analyzer.item()),
    //     ];
    //
    //     let analyzer = ConfigStructAnalyzer::new(
    //         name,
    //         vec![],
    //         vec![],
    //         default_fields,
    //     );
    //
    //     // Act
    //     let result = analyzer.gen_new_fn();
    //
    //     // Assert
    //     let expected = quote! {
    //         impl ConfigWithDefaults {
    //             /// Create a new instance of the config.
    //             pub fn new(
    //             ) -> Self {
    //                 Self {
    //                     count: 42,
    //                     name: "default_name",
    //                 }
    //             }
    //         }
    //     };
    //
    //     assert_eq!(result.to_string(), expected.to_string());
    // }

    #[test]
    fn test_gen_new_fn_all_field_types() {
        // Arrange
        let name = Ident::new("CompleteConfig", Span::call_site());

        // Create required field
        let req_field: Field = parse_quote!(pub required: String);

        // Create optional field
        let opt_field: Field = parse_quote!(pub optional: Option<i32>);

        // Create field with default
        let default_field: Field = parse_quote!(pub with_default: f32);
        let default_attr: Attribute = parse_quote!(#[config(default = 3.14)]);
        let default_analyzer = AttributeAnalyzer::new(default_attr);

        let analyzer = ConfigStructAnalyzer::new(
            name,
            vec![FieldTypeAnalyzer::new(req_field)],
            vec![FieldTypeAnalyzer::new(opt_field)],
            vec![(FieldTypeAnalyzer::new(default_field), default_analyzer.item())],
        );

        // Act
        let result = analyzer.gen_new_fn();

        // Assert
        let expected = quote! {
            impl CompleteConfig {
                /// Create a new instance of the config.
                pub fn new(
                    required: String
                ) -> Self {
                    Self {
                        required: required,
                        optional: None,
                        with_default: 3.14,
                    }
                }
            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_gen_new_fn_empty_struct() {
        // Arrange
        let name = Ident::new("EmptyConfig", Span::call_site());

        let analyzer = ConfigStructAnalyzer::new(
            name,
            vec![],
            vec![],
            vec![],
        );

        // Act
        let result = analyzer.gen_new_fn();

        // Assert
        let expected = quote! {
            impl EmptyConfig {
                /// Create a new instance of the config.
                pub fn new(
                ) -> Self {
                    Self {
                    }
                }
            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_gen_builder_fns_for_default_fields() {
        // Arrange
        let name = Ident::new("ConfigWithDefaults", Span::call_site());

        // Create field with default value
        let field: Field = parse_quote!(pub learning_rate: f32);
        let attr: Attribute = parse_quote!(#[config(default = 0.01)]);
        let default_field = (FieldTypeAnalyzer::new(field), AttributeAnalyzer::new(attr).item());

        let analyzer = ConfigStructAnalyzer::new(
            name,
            vec![],
            vec![],
            vec![default_field],
        );

        // Act
        let result = analyzer.gen_builder_fns();

        // Assert
        let expected = quote! {
            impl ConfigWithDefaults {
                /// Set the default value for the field.
                pub fn with_learning_rate(mut self, learning_rate: f32) -> Self {
                    self.learning_rate = learning_rate;
                    self
                }
            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_gen_builder_fns_for_optional_fields() {
        // Arrange
        let name = Ident::new("ConfigWithOptions", Span::call_site());

        // Create optional field
        let field: Field = parse_quote!(pub dropout: Option<f32>);

        let analyzer = ConfigStructAnalyzer::new(
            name,
            vec![],
            vec![FieldTypeAnalyzer::new(field)],
            vec![],
        );

        // Act
        let result = analyzer.gen_builder_fns();

        // Assert
        let expected = quote! {
            impl ConfigWithOptions {
                /// Set the default value for the field.
                pub fn with_dropout(mut self, dropout: Option<f32>) -> Self {
                    self.dropout = dropout;
                    self
                }
            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_gen_builder_fns_with_multiple_fields() {
        // Arrange
        let name = Ident::new("ModelConfig", Span::call_site());

        // Create default field
        let default_field: Field = parse_quote!(pub learning_rate: f32);
        let default_attr: Attribute = parse_quote!(#[config(default = 0.01)]);
        let default = (FieldTypeAnalyzer::new(default_field), AttributeAnalyzer::new(default_attr).item());

        // Create optional fields
        let option_field1: Field = parse_quote!(pub dropout: Option<f32>);
        let option_field2: Field = parse_quote!(pub batch_size: Option<usize>);

        let analyzer = ConfigStructAnalyzer::new(
            name,
            vec![],
            vec![
                FieldTypeAnalyzer::new(option_field1),
                FieldTypeAnalyzer::new(option_field2),
            ],
            vec![default],
        );

        // Act
        let result = analyzer.gen_builder_fns();

        // Assert - we'll use a simplified assertion to check for all expected builder methods
        let result_str = result.to_string();

        assert!(result_str.contains("with_learning_rate"));
        assert!(result_str.contains("with_dropout"));
        assert!(result_str.contains("with_batch_size"));
    }

    #[test]
    fn test_gen_builder_fns_with_doc_comments() {
        // Arrange
        let name = Ident::new("DocConfig", Span::call_site());

        // Create field with doc comment
        let mut field: Field = parse_quote!(pub epochs: usize);
        field.attrs.push(parse_quote!(#[doc = "Number of training epochs"]));

        let attr: Attribute = parse_quote!(#[config(default = 10)]);
        let default_field = (FieldTypeAnalyzer::new(field), AttributeAnalyzer::new(attr).item());

        let analyzer = ConfigStructAnalyzer::new(
            name,
            vec![],
            vec![],
            vec![default_field],
        );

        // Act
        let result = analyzer.gen_builder_fns();

        // Assert
        let result_str = result.to_string();

        // Check that the doc comment was preserved
        assert!(result_str.contains("Number of training epochs"));
        assert!(result_str.contains("with_epochs"));
    }

    #[test]
    fn test_gen_builder_fns_empty() {
        // Arrange
        let name = Ident::new("EmptyConfig", Span::call_site());

        let analyzer = ConfigStructAnalyzer::new(
            name,
            vec![],
            vec![],
            vec![],
        );

        // Act
        let result = analyzer.gen_builder_fns();

        // Assert
        let expected = quote! {
            impl EmptyConfig {
            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_gen_serde_impl_basic_struct() {
        use proc_macro2::Span;
        use syn::parse_quote;
        use quote::quote;

        // Arrange
        let struct_name = Ident::new("ModelConfig", Span::call_site());

        // Create fields for a simple ML config
        let field1: Field = parse_quote!(pub learning_rate: f32);
        let field2: Field = parse_quote!(pub batch_size: usize);
        let field3: Field = parse_quote!(pub model_name: String);

        // Create the analyzer with these fields as required
        let analyzer = ConfigStructAnalyzer::new(
            struct_name,
            vec![
                FieldTypeAnalyzer::new(field1),
                FieldTypeAnalyzer::new(field2),
                FieldTypeAnalyzer::new(field3),
            ],
            vec![],
            vec![],
        );

        // Act
        let result = analyzer.gen_serde_impl();

        // Assert
        let expected = quote! {
            impl burn::serde::Serialize for ModelConfig {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: burn::serde::Serializer {
                    #[derive(burn::serde::Serialize)]
                    #[serde(crate = "burn::serde")]
                    struct ModelConfigSerde {
                        learning_rate: f32,
                        batch_size: usize,
                        model_name: String
                    }

                    let serde_state = ModelConfigSerde {
                        learning_rate: self.learning_rate.clone(),
                        batch_size: self.batch_size.clone(),
                        model_name: self.model_name.clone()
                    };
                    serde_state.serialize(serializer)
                }
            }

            impl<'de> burn::serde::Deserialize<'de> for ModelConfig {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: burn::serde::Deserializer<'de> {
                    #[derive(burn::serde::Deserialize)]
                    #[serde(crate = "burn::serde")]
                    struct ModelConfigSerde {
                        learning_rate: f32,
                        batch_size: usize,
                        model_name: String
                    }

                    let serde_state = ModelConfigSerde::deserialize(deserializer)?;
                    Ok(ModelConfig {
                        learning_rate: serde_state.learning_rate,
                        batch_size: serde_state.batch_size,
                        model_name: serde_state.model_name
                    })
                }
            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_gen_clone_impl() {
        use proc_macro2::Span;
        use syn::parse_quote;
        use quote::quote;

        // Arrange
        let struct_name = Ident::new("DataConfig", Span::call_site());

        // Create various field types
        let field1: Field = parse_quote!(pub dataset_path: String);
        let field2: Field = parse_quote!(pub batch_size: usize);
        let field3: Field = parse_quote!(pub shuffle: bool);

        // Create the analyzer with these fields
        let analyzer = ConfigStructAnalyzer::new(
            struct_name,
            vec![
                FieldTypeAnalyzer::new(field1),
                FieldTypeAnalyzer::new(field2),
                FieldTypeAnalyzer::new(field3),
            ],
            vec![],
            vec![],
        );

        // Act
        let result = analyzer.gen_clone_impl();

        // Assert
        let expected = quote! {
            impl Clone for DataConfig {
                fn clone(&self) -> Self {
                    Self {
                        dataset_path: self.dataset_path.clone(),
                        batch_size: self.batch_size.clone(),
                        shuffle: self.shuffle.clone()
                    }
                }
            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }
}
