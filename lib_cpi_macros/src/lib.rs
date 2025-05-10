extern crate proc_macro;
use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{parse_macro_input, ItemFn, FnArg, Pat, Expr, ExprLit, Lit, parse::Parse, parse::ParseStream, Token};
use syn::punctuated::Punctuated;
use syn::{Attribute, Meta, MetaNameValue, Path, Ident};
use std::collections::HashMap;

/// Parameter attribute structure
struct ParamAttr {
    name: Option<String>,
    description: Option<String>,
    param_type: Option<String>,
    required: Option<bool>,
    default_value: Option<String>,
}

impl Parse for ParamAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut attrs = ParamAttr {
            name: None,
            description: None,
            param_type: None,
            required: None,
            default_value: None,
        };
        
        let content;
        syn::parenthesized!(content in input);
        let pairs = Punctuated::<MetaNameValue, Token![,]>::parse_terminated(&content)?;
        
        for pair in pairs {
            let name_ident = pair.path.get_ident().unwrap().to_string();
            
            if let Expr::Lit(ExprLit { lit, .. }) = pair.value {
                match name_ident.as_str() {
                    "name" => {
                        if let Lit::Str(lit_str) = lit {
                            attrs.name = Some(lit_str.value());
                        }
                    },
                    "description" => {
                        if let Lit::Str(lit_str) = lit {
                            attrs.description = Some(lit_str.value());
                        }
                    },
                    "type" => {
                        if let Lit::Str(lit_str) = lit {
                            attrs.param_type = Some(lit_str.value());
                        }
                    },
                    "required" => {
                        if let Lit::Bool(lit_bool) = lit {
                            attrs.required = Some(lit_bool.value);
                        }
                    },
                    "default" => {
                        if let Lit::Str(lit_str) = lit {
                            attrs.default_value = Some(lit_str.value());
                        }
                    },
                    _ => {}
                }
            }
        }
        
        Ok(attrs)
    }
}

/// Action attribute structure
struct ActionAttr {
    description: Option<String>,
}

impl Parse for ActionAttr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut attr = ActionAttr {
            description: None,
        };
        
        let content;
        syn::parenthesized!(content in input);
        let pairs = Punctuated::<MetaNameValue, Token![,]>::parse_terminated(&content)?;
        
        for pair in pairs {
            let name_ident = pair.path.get_ident().unwrap().to_string();
            
            if name_ident == "description" {
                if let Expr::Lit(ExprLit { lit: Lit::Str(lit_str), .. }) = pair.value {
                    attr.description = Some(lit_str.value());
                }
            }
        }
        
        Ok(attr)
    }
}

/// Stores parameter metadata during macro processing
struct ParamInfo {
    name: String,
    description: String,
    param_type: String,
    required: bool,
    default_value: Option<String>,
}

impl Default for ParamInfo {
    fn default() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            param_type: "String".to_string(),
            required: true,
            default_value: None,
        }
    }
}

/// Global storage for action metadata during compilation
thread_local! {
    static ACTION_METADATA: std::cell::RefCell<HashMap<String, (String, Vec<ParamInfo>)>> = 
        std::cell::RefCell::new(HashMap::new());
}

/// Macro to annotate extension action functions with metadata
/// 
/// Usage: #[action(description = "Description of the action")]
#[proc_macro_attribute]
pub fn action(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let fn_name = input.sig.ident.to_string();
    
    // Parse the action attribute
    let action_attr = parse_macro_input!(attr as ActionAttr);
    let description = action_attr.description
        .unwrap_or_else(|| format!("Action {}", fn_name));
    
    // Extract parameter names for metadata initialization
    let mut param_names = Vec::new();
    for arg in &input.sig.inputs {
        if let FnArg::Typed(pat_type) = arg {
            if let Pat::Ident(pat_ident) = &*pat_type.pat {
                let param_name = &pat_ident.ident;
                
                // Skip 'self' parameter
                if param_name == "self" || param_name == "&self" {
                    continue;
                }
                
                param_names.push(param_name.to_string());
            }
        }
    }
    
    // Initialize metadata for this function
    ACTION_METADATA.with(|metadata| {
        let mut map = metadata.borrow_mut();
        let params_vec = Vec::new();
        map.insert(fn_name.clone(), (description, params_vec));
        
        // Initialize with empty parameter entries
        if let Some((_, params)) = map.get_mut(&fn_name) {
            for name in param_names {
                let param_info = ParamInfo {
                    name,
                    ..Default::default()
                };
                params.push(param_info);
            }
        }
    });
    
    // Return the function unchanged
    quote! {
        #input
    }.into()
}

/// Macro to define parameter metadata for actions
///
/// Usage: #[param(name = "param_name", description = "Description", type = "String", required = true, default = "value")]
#[proc_macro_attribute]
pub fn param(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let fn_name = input.sig.ident.to_string();
    
    // Parse the parameter metadata
    let param_attr = parse_macro_input!(attr as ParamAttr);
    let param_name = param_attr.name.unwrap_or_default();
    let description = param_attr.description.unwrap_or_default();
    let param_type = param_attr.param_type.unwrap_or_else(|| "String".to_string());
    let required = param_attr.required.unwrap_or(true);
    let default_value = param_attr.default_value;
    
    // Update the parameter metadata
    ACTION_METADATA.with(|metadata| {
        let mut map = metadata.borrow_mut();
        if let Some((_, params)) = map.get_mut(&fn_name) {
            for param in params.iter_mut() {
                if param.name == param_name {
                    param.description = description.clone();
                    param.param_type = param_type.clone();
                    param.required = required;
                    param.default_value = default_value.clone();
                    break;
                }
            }
        }
    });
    
    // Return the function unchanged
    quote! {
        #input
    }.into()
}

/// Macro to generate the metadata function for an action
/// This should be called after all param annotations
#[proc_macro_attribute]
pub fn generate_metadata(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let fn_name = input.sig.ident.to_string();
    let fn_ident = &input.sig.ident;
    
    // Generate metadata function name
    let meta_fn_name = format_ident!("{}_metadata", fn_name);
    
    // Generate the parameter definitions
    let param_defs = ACTION_METADATA.with(|metadata| {
        let map = metadata.borrow();
        let mut defs = Vec::new();
        
        if let Some((_, params)) = map.get(&fn_name) {
            for param in params {
                let p_name = &param.name;
                let p_desc = &param.description;
                let p_type = match param.param_type.as_str() {
                    "String" => quote! { ParamType::String },
                    "Integer" => quote! { ParamType::Integer },
                    "Boolean" => quote! { ParamType::Boolean },
                    "Object" => quote! { ParamType::Object },
                    "Array" => quote! { ParamType::Array },
                    _ => quote! { ParamType::String },
                };
                let p_required = param.required;
                
                let default_value_code = if let Some(default) = &param.default_value {
                    if param.param_type == "String" {
                        quote! { Some(json!(#default)) }
                    } else if param.param_type == "Integer" {
                        let int_value: i64 = default.parse().unwrap_or(0);
                        quote! { Some(json!(#int_value)) }
                    } else if param.param_type == "Boolean" {
                        let bool_value: bool = default.parse().unwrap_or(false);
                        quote! { Some(json!(#bool_value)) }
                    } else {
                        quote! { None }
                    }
                } else {
                    quote! { None }
                };
                
                defs.push(quote! {
                    ActionParameter {
                        name: #p_name.to_string(),
                        description: #p_desc.to_string(),
                        param_type: #p_type,
                        required: #p_required,
                        default_value: #default_value_code,
                    }
                });
            }
        }
        
        defs
    });
    
    // Get the description
    let description = ACTION_METADATA.with(|metadata| {
        let map = metadata.borrow();
        if let Some((desc, _)) = map.get(&fn_name) {
            desc.clone()
        } else {
            format!("Action {}", fn_name)
        }
    });
    
    // Final output - function + metadata function
    let result = quote! {
        #input
        
        fn #meta_fn_name() -> ActionDefinition {
            ActionDefinition {
                name: #fn_name.to_string(),
                description: #description.to_string(),
                parameters: vec![
                    #(#param_defs),*
                ],
            }
        }
    };
    
    result.into()
}

/// Macro to register action functions with the CpiExtension trait
///
/// Usage: register_actions![action1, action2, ...]
#[proc_macro]
pub fn register_actions(input: TokenStream) -> TokenStream {
    let input_str = input.to_string();
    let action_names: Vec<String> = input_str
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    
    let action_strings = action_names.iter().map(|s| s.as_str());
    let action_meta_fns = action_names.iter().map(|name| format_ident!("{}_metadata", name));
    
    let list_actions_impl = quote! {
        fn list_actions(&self) -> Vec<String> {
            vec![
                #(#action_strings.to_string()),*
            ]
        }
    };
    
    let get_action_def_match_arms = action_names.iter().zip(action_meta_fns).map(|(name, meta_fn)| {
        quote! {
            #name => Some(#meta_fn()),
        }
    });
    
    let get_action_def_impl = quote! {
        fn get_action_definition(&self, action: &str) -> Option<ActionDefinition> {
            match action {
                #(#get_action_def_match_arms)*
                _ => None,
            }
        }
    };
    
    let final_code = quote! {
        #list_actions_impl
        
        #get_action_def_impl
        
        // NOTE: The execute_action method needs to be implemented manually
    };
    
    final_code.into()
}