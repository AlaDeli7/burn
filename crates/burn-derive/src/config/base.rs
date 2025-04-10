use super::ConfigAnalyzerFactory;
use quote::quote;

pub(crate) fn derive_impl(item: &syn::DeriveInput) -> proc_macro::TokenStream {
    let input = derive_impl_code(&item);
    proc_macro::TokenStream::from(input)
}

// Goal : Avoid direct use of proc_macro APIs in the testable logic
fn derive_impl_code(item: &syn::DeriveInput) -> proc_macro2::TokenStream {
    let factory = ConfigAnalyzerFactory::new();
    let analyzer = factory.create_analyzer(item);

    let constructor = analyzer.gen_new_fn();
    let builders = analyzer.gen_builder_fns();
    let serde = analyzer.gen_serde_impl();
    let clone = analyzer.gen_clone_impl();
    let display = analyzer.gen_display_impl();
    let config_impl = analyzer.gen_config_impl();

    quote! {
        #config_impl
        #constructor
        #builders
        #serde
        #clone
        #display
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;
    use syn::parse_quote;

    #[test]
    fn test_derive_impl_for_struct() {
        // Arrange - Create a struct
        let struct_input = parse_quote! {
            // #[derive(Config)]
            pub struct NetworkConfig {
                #[config(default = "0.001")]
                pub learning_rate: f32,
                pub batch_size: usize,
                pub momentum: Option<f32>,
            }
        };

        // Arrange - Create expected output
        let expected = quote! {
            impl burn::config::Config for NetworkConfig {
            }

            impl NetworkConfig {
                /// Create a new instance of the config.
                pub fn new(
                    batch_size: usize
                ) -> Self {
                    Self {
                        batch_size: batch_size,
                        momentum: None,
                        learning_rate: 0.001,
                    }
                }
            }

            impl NetworkConfig {
                /// Set the default value for the field.
                pub fn with_learning_rate(mut self, learning_rate: f32) -> Self {
                    self.learning_rate = learning_rate;
                    self
                }

                /// Set the default value for the field.
                pub fn with_momentum(mut self, momentum: Option<f32>) -> Self {
                    self.momentum = momentum;
                    self
                }
            }

            impl burn::serde::Serialize for NetworkConfig {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: burn::serde::Serializer {
                    #[derive(burn::serde::Serialize)]
                    #[serde(crate = "burn::serde")]
                    struct NetworkConfigSerde {
                        batch_size: usize,
                        momentum: Option<f32>,
                        learning_rate: f32
                    }

                    let serde_state = NetworkConfigSerde {
                        batch_size: self.batch_size.clone(),
                        momentum: self.momentum.clone(),
                        learning_rate: self.learning_rate.clone()
                    };
                    serde_state.serialize(serializer)
                }
            }

            impl<'de> burn::serde::Deserialize<'de> for NetworkConfig {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: burn::serde::Deserializer<'de> {
                    #[derive(burn::serde::Deserialize)]
                    #[serde(crate = "burn::serde")]
                    struct NetworkConfigSerde {
                        batch_size: usize,
                        momentum: Option<f32>,
                        learning_rate: f32
                    }

                    let serde_state = NetworkConfigSerde::deserialize(deserializer)?;
                    Ok(NetworkConfig {
                        batch_size: serde_state.batch_size,
                        momentum: serde_state.momentum,
                        learning_rate: serde_state.learning_rate
                    })
                }
            }

            impl Clone for NetworkConfig {
                fn clone(&self) -> Self {
                    Self {
                        batch_size: self.batch_size.clone(),
                        momentum: self.momentum.clone(),
                        learning_rate: self.learning_rate.clone()
                    }
                }
            }

            impl core::fmt::Display for NetworkConfig {
                fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    f.write_str(&burn::config::config_to_json(self))
                }
            }
        };

        // Act -
        let actual = derive_impl_code(&struct_input);

        // Assert - Compare the string representations
        assert_eq!(actual.to_string(), expected.to_string());
    }
}
