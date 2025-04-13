use crate::shared::enum_variant::map_enum_variant;

use super::ConfigAnalyzer;
use proc_macro2::{Ident, TokenStream};
use quote::quote;

pub struct ConfigEnumAnalyzer {
    name: Ident,
    data: syn::DataEnum,
}

impl ConfigEnumAnalyzer {
    pub fn new(name: Ident, data: syn::DataEnum) -> Self {
        Self { name, data }
    }

    fn serde_enum_ident(&self) -> Ident {
        Ident::new(&format!("{}Serde", self.name), self.name.span())
    }

    fn gen_serde_enum(&self) -> TokenStream {
        let enum_name = self.serde_enum_ident();
        let data = &self.data.variants;

        quote! {
            #[derive(burn::serde::Serialize, burn::serde::Deserialize)]
            #[serde(crate = "burn::serde")]
            enum #enum_name {
                #data
            }

        }
    }

    fn gen_serialize_fn(&self) -> TokenStream {
        let enum_name = self.serde_enum_ident();
        let variants = self.data.variants.iter().map(|variant| {
            let variant_name = &variant.ident;
            let (inputs, outputs) = map_enum_variant(variant, |ident| quote! { #ident.clone() });

            quote! { Self::#variant_name #inputs => #enum_name::#variant_name #outputs }
        });

        let name = &self.name;

        quote! {
            impl burn::serde::Serialize for #name {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: burn::serde::Serializer {
                    let serde_state = match self {
                        #(#variants),*
                    };
                    serde_state.serialize(serializer)
                }
            }

        }
    }

    fn gen_deserialize_fn(&self) -> TokenStream {
        let enum_name = self.serde_enum_ident();
        let variants = self.data.variants.iter().map(|variant| {
            let variant_name = &variant.ident;
            let (inputs, outputs) = map_enum_variant(variant, |ident| quote! { #ident.clone() });

            quote! { #enum_name::#variant_name #inputs => Self::#variant_name #outputs }
        });
        let name = &self.name;

        quote! {
            impl<'de> burn::serde::Deserialize<'de> for #name {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: burn::serde::Deserializer<'de> {
                    let serde_state = #enum_name::deserialize(deserializer)?;
                    Ok(match serde_state {
                        #(#variants),*
                    })
                }
            }

        }
    }
}

impl ConfigAnalyzer for ConfigEnumAnalyzer {
    fn gen_serde_impl(&self) -> TokenStream {
        let struct_gen = self.gen_serde_enum();
        let serialize_gen = self.gen_serialize_fn();
        let deserialize_gen = self.gen_deserialize_fn();

        quote! {
            #struct_gen
            #serialize_gen
            #deserialize_gen
        }
    }

    fn gen_clone_impl(&self) -> TokenStream {
        let variants = self.data.variants.iter().map(|variant| {
            let variant_name = &variant.ident;
            let (inputs, outputs) = map_enum_variant(variant, |ident| quote! { #ident.clone() });

            quote! { Self::#variant_name #inputs => Self::#variant_name #outputs }
        });
        let name = &self.name;

        quote! {
            impl Clone for #name {
                fn clone(&self) -> Self {
                    match self {
                        #(#variants),*
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
    use syn::{parse_quote, DeriveInput, Data};

    #[test]
    fn test_serde_enum_ident() {
        // Create a DeriveInput for an enum using parse_quote
        let input: DeriveInput = parse_quote! {
            enum DeviceType {
                CPU,
                GPU,
                TPU
            }
        };

        // Extract the DataEnum from DeriveInput
        let enum_data = match &input.data {
            Data::Enum(data) => data.clone(),
            _ => panic!("Expected enum data"),
        };

        // Create the analyzer
        let enum_name = input.ident.clone();
        let analyzer = ConfigEnumAnalyzer::new(enum_name, enum_data);

        // Act - Get the generated serde enum identifier
        let result = analyzer.serde_enum_ident();

        // Assert - Verify the name is correctly formed and preserves span
        assert_eq!(result.to_string(), "DeviceTypeSerde");
    }

    #[test]
    fn test_gen_serde_enum() {
        // Create a DeriveInput for an enum with different variant types
        let input: DeriveInput = parse_quote! {
            enum DeviceType {
                CPU,
                GPU(String),
                TPU { cores: u32, vendor: String }
            }
        };

        // Extract the DataEnum from DeriveInput
        let enum_data = match &input.data {
            Data::Enum(data) => data.clone(),
            _ => panic!("Expected enum data"),
        };

        // Create the analyzer with the enum name and data
        let enum_name = input.ident.clone();
        let analyzer = ConfigEnumAnalyzer::new(enum_name, enum_data);

        // Act - Generate the serde enum
        let result = analyzer.gen_serde_enum();

        // Assert - Verify the generated code structure
        let expected = quote! {
            #[derive(burn::serde::Serialize, burn::serde::Deserialize)]
            #[serde(crate = "burn::serde")]
            enum DeviceTypeSerde {
                CPU,
                GPU(String),
                TPU { cores: u32, vendor: String }
            }
        };

        assert_eq!(result.to_string(), expected.to_string());
    }

    #[test]
    fn test_gen_serialize_fn() {
        // Arrange - Create an enum with different variant types
        let input: DeriveInput = parse_quote! {
            enum Shape {
                Circle(f32),
                Rectangle { width: f32, height: f32 },
                Point
            }
        };

        // Arrange - Extract the enum data
        let enum_data = match &input.data {
            Data::Enum(data) => data.clone(),
            _ => panic!("Expected enum data"),
        };

        // Arrange - Create the analyzer
        let analyzer = ConfigEnumAnalyzer::new(input.ident.clone(), enum_data);

        // Act - Generate the serialization implementation
        let serialize_impl = analyzer.gen_serialize_fn();

        // Arrange - Define expected implementation
        let expected = quote! {
            impl burn::serde::Serialize for Shape {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: burn::serde::Serializer {
                    let serde_state = match self {
                        Self::Circle(arg_0) => ShapeSerde::Circle(arg_0.clone()),
                        Self::Rectangle { width, height } => ShapeSerde::Rectangle { width: width.clone(), height: height.clone() },
                        Self::Point => ShapeSerde::Point
                    };
                    serde_state.serialize(serializer)
                }
            }
        };

        // Assert - Verify generated code matches expected
        assert_eq!(serialize_impl.to_string(), expected.to_string());
    }

    #[test]
    fn test_gen_deserialize_fn() {
        // Arrange - Create an enum with different variant types
        let input: DeriveInput = parse_quote! {
            enum Shape {
                Circle(f32),
                Rectangle { width: f32, height: f32 },
                Point
            }
        };

        // Arrange - Extract the enum data
        let Data::Enum(enum_data) = input.data.clone() else {
            panic!("Expected enum data")
        };

        // Arrange - Create the analyzer
        let analyzer = ConfigEnumAnalyzer::new(input.ident.clone(), enum_data);

        // Act - Generate the deserialization implementation
        let deserialize_impl = analyzer.gen_deserialize_fn();

        // Arrange - Define expected implementation
        let expected = quote! {
            impl<'de> burn::serde::Deserialize<'de> for Shape {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: burn::serde::Deserializer<'de> {
                    let serde_state = ShapeSerde::deserialize(deserializer)?;
                    Ok(match serde_state {
                        ShapeSerde::Circle(arg_0) => Self::Circle(arg_0.clone()),
                        ShapeSerde::Rectangle { width, height } => Self::Rectangle { width: width.clone(), height: height.clone() },
                        ShapeSerde::Point => Self::Point
                    })
                }
            }
        };

        // Assert - Verify generated code matches expected
        assert_eq!(deserialize_impl.to_string(), expected.to_string());
    }

    #[test]
    fn test_gen_serde_impl() {
        // Arrange - Create an enum with different variant types
        let input: DeriveInput = parse_quote! {
            enum Device {
                CPU,
                GPU(String)
            }
        };

        // Arrange - Extract the enum data
        let Data::Enum(enum_data) = input.data.clone() else {
            panic!("Expected enum data")
        };

        // Arrange - Create the analyzer
        let analyzer = ConfigEnumAnalyzer::new(input.ident.clone(), enum_data);

        // Act - Generate the combined serde implementation
        let actual = analyzer.gen_serde_impl();

        // Arrange - Define expected implementation
        let expected = quote! {
            #[derive(burn::serde::Serialize, burn::serde::Deserialize)]
            #[serde(crate = "burn::serde")]
            enum DeviceSerde {
                CPU,
                GPU(String)
            }

            impl burn::serde::Serialize for Device {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: burn::serde::Serializer {
                    let serde_state = match self {
                        Self::CPU => DeviceSerde::CPU,
                        Self::GPU(arg_0) => DeviceSerde::GPU(arg_0.clone())
                    };
                    serde_state.serialize(serializer)
                }
            }

            impl<'de> burn::serde::Deserialize<'de> for Device {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: burn::serde::Deserializer<'de> {
                    let serde_state = DeviceSerde::deserialize(deserializer)?;
                    Ok(match serde_state {
                        DeviceSerde::CPU => Self::CPU,
                        DeviceSerde::GPU(arg_0) => Self::GPU(arg_0.clone())
                    })
                }
            }
        };

        // Assert - Verify generated code matches expected
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn test_gen_clone_impl() {
        // Arrange - Create an enum with different variant types
        let input: DeriveInput = parse_quote! {
            enum Message {
                Text(String),
                Command { name: String, args: Vec<String> },
                Quit
            }
        };

        // Arrange - Extract the enum data
        let Data::Enum(enum_data) = input.data.clone() else {
            panic!("Expected enum data")
        };

        // Arrange - Create the analyzer
        let analyzer = ConfigEnumAnalyzer::new(input.ident.clone(), enum_data);

        // Act - Generate the Clone implementation
        let actual = analyzer.gen_clone_impl();

        // Arrange - Define expected implementation
        let expected = quote! {
            impl Clone for Message {
                fn clone(&self) -> Self {
                    match self {
                        Self::Text(arg_0) => Self::Text(arg_0.clone()),
                        Self::Command { name, args } => Self::Command { name: name.clone(), args: args.clone() },
                        Self::Quit => Self::Quit
                    }
                }
            }
        };

        // Assert - Verify generated code matches expected
        assert_eq!(
            actual.to_string(),
            expected.to_string()
        );
    }
}