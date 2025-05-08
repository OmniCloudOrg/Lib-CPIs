// File: lib_cpi_macros/src/lib.rs
extern crate proc_macro;
use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{parse_macro_input, ItemFn, FnArg, Pat};

/// Macro to annotate extension action functions
/// 
/// Usage: #[action("Description of the action")]
/// 
/// This will register the function as an action and generate metadata about its parameters
#[proc_macro_attribute]
pub fn action(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the input function
    let input = parse_macro_input!(item as ItemFn);
    let fn_name = &input.sig.ident;
    let fn_vis = &input.vis;
    
    // Parse the attribute parameters (description)
    let attr_str = attr.to_string();
    let description = if attr_str.is_empty() {
        format!("Action {}", fn_name)
    } else {
        attr_str.trim_matches('"').to_string()
    };
    
    // Generate the metadata registration function name
    let meta_fn_name = format_ident!("{}_metadata", fn_name);
    
    // Extract parameter names for metadata
    let mut param_defs = Vec::new();
    
    for arg in &input.sig.inputs {
        if let FnArg::Typed(pat_type) = arg {
            if let Pat::Ident(pat_ident) = &*pat_type.pat {
                let param_name = &pat_ident.ident;
                
                // Skip 'self' parameter
                if param_name == "self" {
                    continue;
                }
                
                let param_name_str = param_name.to_string();
                
                // Create a parameter definition tokens
                param_defs.push(quote! {
                    ActionParameter {
                        name: #param_name_str.to_string(),
                        description: format!("Parameter {}", #param_name_str),
                        required: true,
                        param_type: ParamType::String, // Default to string for simplicity
                        default_value: None,
                    }
                });
            }
        }
    }
    
    // Generate the metadata function
    let meta_fn = if param_defs.is_empty() {
        quote! {
            fn #meta_fn_name() -> ActionDefinition {
                ActionDefinition {
                    name: #fn_name.to_string(),
                    description: #description.to_string(),
                    parameters: vec![],
                }
            }
        }
    } else {
        quote! {
            fn #meta_fn_name() -> ActionDefinition {
                ActionDefinition {
                    name: #fn_name.to_string(),
                    description: #description.to_string(),
                    parameters: vec![
                        #(#param_defs),*
                    ],
                }
            }
        }
    };
    
    // Generate the expanded output
    let result = quote! {
        #input
        
        #meta_fn
    };
    
    result.into()
}