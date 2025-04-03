use syn::{Attribute, Meta};

pub struct AttributeAnalyzer {
    attr: Attribute,
}

#[derive(Clone)]
pub struct AttributeItem {
    pub value: syn::Lit,
}

impl AttributeAnalyzer {
    pub fn new(attr: Attribute) -> Self {
        Self { attr }
    }

    pub fn item(&self) -> AttributeItem {
        let value = match &self.attr.meta {
            Meta::List(val) => val.parse_args::<syn::MetaNameValue>().unwrap(),
            Meta::NameValue(meta) => meta.clone(),
            Meta::Path(_) => panic!("Path meta unsupported"),
        };

        let lit = match value.value {
            syn::Expr::Lit(lit) => lit.lit,
            _ => panic!("Only literal is supported"),
        };

        AttributeItem { value: lit }
    }

    pub fn has_name(&self, name: &str) -> bool {
        Self::path_syn_name(self.attr.path()) == name
    }

    fn path_syn_name(path: &syn::Path) -> String {
        let length = path.segments.len();
        let mut name = String::new();
        for (i, segment) in path.segments.iter().enumerate() {
            if i == length - 1 {
                name += segment.ident.to_string().as_str();
            } else {
                let tmp = segment.ident.to_string() + "::";
                name += tmp.as_str();
            }
        }
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn test_attribute_has_name() {
        // Test for config attribute
        let attr: Attribute = parse_quote!(#[config(default = "42")]);
        let analyzer = AttributeAnalyzer::new(attr);

        assert!(analyzer.has_name("config"));
        assert!(!analyzer.has_name("module"));
        assert!(!analyzer.has_name("something_else"));
    }

    #[test]
    fn test_attribute_item_extraction() {
        // Test for default value extraction
        let attr: Attribute = parse_quote!(#[config(default = "42")]);
        let analyzer = AttributeAnalyzer::new(attr);
        let item = analyzer.item();

        if let syn::Lit::Str(lit_str) = &item.value {
            assert_eq!(lit_str.value(), "42");
        } else {
            panic!("Expected string literal");
        }

        // Test with numeric literal
        let attr: Attribute = parse_quote!(#[config(default = 3.14)]);
        let analyzer = AttributeAnalyzer::new(attr);
        let item = analyzer.item();

        if let syn::Lit::Float(lit_float) = &item.value {
            assert_eq!(lit_float.base10_parse::<f32>().unwrap(), 3.14);
        } else {
            panic!("Expected float literal");
        }

        // Test with boolean literal
        let attr: Attribute = parse_quote!(#[config(default = true)]);
        let analyzer = AttributeAnalyzer::new(attr);
        let item = analyzer.item();

        if let syn::Lit::Bool(lit_bool) = &item.value {
            assert_eq!(lit_bool.value, true);
        } else {
            panic!("Expected boolean literal");
        }
    }

    #[test]
    fn test_path_syn_name() {
        // Simple path
        let path: syn::Path = parse_quote!(config);
        assert_eq!(AttributeAnalyzer::path_syn_name(&path), "config");

        // Nested path
        let path: syn::Path = parse_quote!(burn::config);
        assert_eq!(AttributeAnalyzer::path_syn_name(&path), "burn::config");

        // Multiple segments
        let path: syn::Path = parse_quote!(burn::module::config);
        assert_eq!(AttributeAnalyzer::path_syn_name(&path), "burn::module::config");
    }

    #[test]
    #[should_panic(expected = "Path meta unsupported")]
    fn test_item_with_path_meta() {
        // This should panic as per the implementation
        let attr: Attribute = parse_quote!(#[config]);
        let analyzer = AttributeAnalyzer::new(attr);
        analyzer.item(); // Should panic
    }

    #[test]
    #[should_panic(expected = "Only literal is supported")]
    fn test_item_with_non_literal() {
        // This should panic as per the implementation
        let attr: Attribute = parse_quote!(#[config(default = 1 + 2)]);
        let analyzer = AttributeAnalyzer::new(attr);
        analyzer.item(); // Should panic because 1 + 2 is an expression, not a literal
    }
}

