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

// File: lib_cpi/src/macros/mod.rs

use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{
    parse_macro_input, ItemFn, Ident, LitStr, 
    parse::{Parse, ParseStream},
    Result, Expr, punctuated::Punctuated, Token, 
    ItemStruct, Meta, Lit, MetaNameValue
};

/// Parameter definition for a CPI action
struct ParamDef {
    name: Ident,
    description: String,
    param_type: Ident,
    required: bool,
    default_value: Option<Expr>,
}

impl Parse for ParamDef {
    fn parse(input: ParseStream) -> Result<Self> {
        let content;
        syn::parenthesized!(content in input);
        let name: Ident = content.parse()?;
        content.parse::<Token![,]>()?;
        
        // Parse description as a string literal
        let desc_lit: LitStr = content.parse()?;
        let description = desc_lit.value();
        content.parse::<Token![,]>()?;
        
        // Parse parameter type
        let param_type: Ident = content.parse()?;
        content.parse::<Token![,]>()?;
        
        // Parse required/optional flag
        let required_ident: Ident = content.parse()?;
        let required = required_ident == "required";
        
        // Parse default value if present
        let default_value = if content.peek(Token![,]) {
            content.parse::<Token![,]>()?;
            Some(content.parse()?)
        } else {
            None
        };
        
        Ok(ParamDef {
            name,
            description,
            param_type,
            required,
            default_value,
        })
    }
}

/// CPI Action metadata
struct CpiActionMeta {
    description: String,
    params: Vec<ParamDef>,
}

impl Parse for CpiActionMeta {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut description = String::new();
        let mut params = Vec::new();
        
        // Parse attribute arguments like description = "...", param(...), ...
        let attrs = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;
        
        for attr in attrs {
            match attr {
                Meta::NameValue(nv) if nv.path.is_ident("description") => {
                    if let Lit::Str(s) = nv.value {
                        description = s.value();
                    }
                },
                Meta::List(list) if list.path.is_ident("param") => {
                    let param: ParamDef = syn::parse2(list.tokens)?;
                    params.push(param);
                },
                _ => return Err(syn::Error::new_spanned(attr, "Expected 'description' or 'param'")),
            }
        }
        
        Ok(CpiActionMeta {
            description,
            params,
        })
    }
}

/// Extension metadata
struct CpiExtensionMeta {
    name: String,
    provider_type: String,
}

impl Parse for CpiExtensionMeta {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut name = None;
        let mut provider_type = None;
        
        // Parse attribute arguments like name = "...", provider_type = "..."
        let attrs = Punctuated::<Meta, Token![,]>::parse_terminated(input)?;
        
        for attr in attrs {
            match attr {
                Meta::NameValue(nv) if nv.path.is_ident("name") => {
                    if let Lit::Str(s) = nv.value {
                        name = Some(s.value());
                    }
                },
                Meta::NameValue(nv) if nv.path.is_ident("provider_type") => {
                    if let Lit::Str(s) = nv.value {
                        provider_type = Some(s.value());
                    }
                },
                _ => return Err(syn::Error::new_spanned(attr, "Expected 'name' or 'provider_type'")),
            }
        }
        
        Ok(CpiExtensionMeta {
            name: name.unwrap_or_default(),
            provider_type: provider_type.unwrap_or_else(|| "command".to_string()),
        })
    }
}

/// Macro for defining CPI actions
/// 
/// # Example
/// 
/// ```
/// #[cpi_action(
///     description = "Test if VirtualBox is properly installed"
/// )]
/// fn test_install(&self) -> ActionResult {
///     // Implementation
/// }
/// 
/// #[cpi_action(
///     description = "Create a new virtual machine",
///     param(worker_name, "Name of the VM to create", String, required),
///     param(os_type, "Operating system type", String, optional, "Ubuntu_64"),
///     param(memory_mb, "Memory in MB", Integer, optional, 2048),
///     param(cpu_count, "Number of CPUs", Integer, optional, 2)
/// )]
/// fn create_worker(&self, worker_name: String, os_type: String, memory_mb: i64, cpu_count: i64) -> ActionResult {
///     // Implementation
/// }
/// ```
#[proc_macro_attribute]
pub fn cpi_action(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    let meta = parse_macro_input!(attr as CpiActionMeta);
    
    let fn_name = &input_fn.sig.ident;
    let fn_name_str = fn_name.to_string();
    let vis = &input_fn.vis;
    let attrs = &input_fn.attrs;
    let block = &input_fn.block;
    let output = &input_fn.sig.output;
    
    // Generate parameters for function definition
    let params = &input_fn.sig.inputs;
    
    // Generate param names for registration
    let param_names = meta.params.iter().map(|p| &p.name).collect::<Vec<_>>();
    let param_descriptions = meta.params.iter().map(|p| &p.description).collect::<Vec<_>>();
    let param_types = meta.params.iter().map(|p| &p.param_type).collect::<Vec<_>>();
    let param_requireds = meta.params.iter().map(|p| p.required).collect::<Vec<_>>();
    let param_defaults = meta.params.iter().map(|p| p.default_value.as_ref()).collect::<Vec<_>>();
    
    // Add action attribute to the function and register its metadata
    let result = quote! {
        #(#attrs)*
        #[action]
        #vis fn #fn_name(#params) #output #block
        
        // This metadata will be collected by the cpi_extension macro
        #[doc(hidden)]
        #[export_name = concat!("__cpi_action_", stringify!(#fn_name))]
        pub extern "C" fn __register_action() {
            // Register action definition and handler
            ACTION_REGISTRY.with(|registry| {
                let mut registry = registry.borrow_mut();
                
                // Add action definition
                registry.definitions.insert(
                    #fn_name_str.to_string(),
                    ActionDefinition {
                        name: #fn_name_str.to_string(),
                        description: #meta.description.to_string(),
                        parameters: vec![
                            #(
                                param!(
                                    stringify!(#param_names), 
                                    #param_descriptions, 
                                    ParamType::#param_types, 
                                    if #param_requireds { required } else { optional }
                                    #(, if let Some(default) = #param_defaults { default } else { panic!("Default value expected") })*
                                )
                            ),*
                        ],
                    }
                );
                
                // Add action handler
                registry.handlers.insert(
                    #fn_name_str.to_string(),
                    |extension, params| {
                        // Extract parameters based on their types
                        #(
                            let #param_names = if #param_requireds {
                                if stringify!(#param_types) == "String" {
                                    validation::extract_string(params, stringify!(#param_names))?
                                } else {
                                    validation::extract_int(params, stringify!(#param_names))?
                                }
                            } else {
                                if stringify!(#param_types) == "String" {
                                    validation::extract_string_opt(params, stringify!(#param_names))?.unwrap_or_else(|| {
                                        // Here we would use the default value, but for simplicity we'll use a placeholder
                                        "default".to_string()
                                    })
                                } else {
                                    validation::extract_int_opt(params, stringify!(#param_names))?.unwrap_or(0)
                                }
                            };
                        )*
                        
                        // Call the actual method
                        extension.#fn_name(#(#param_names),*)
                    }
                );
            });
        }
    };
    
    result.into()
}

/// Macro for implementing CPI extension
/// 
/// # Example
/// 
/// ```
/// #[cpi_extension(
///     name = "virtualbox",
///     provider_type = "command"
/// )]
/// pub struct VirtualBoxExtension {
///     default_settings: HashMap<String, Value>,
/// }
/// ```
#[proc_macro_attribute]
pub fn cpi_extension(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_struct = parse_macro_input!(item as ItemStruct);
    let meta = parse_macro_input!(attr as CpiExtensionMeta);
    
    let struct_name = &input_struct.ident;
    let name = &meta.name;
    let provider_type = &meta.provider_type;
    
    // Generate implementation of CpiExtension trait
    let result = quote! {
        // Define the original struct
        #input_struct
        
        // Define a thread_local registry to store action definitions and handlers
        thread_local! {
            static ACTION_REGISTRY: std::cell::RefCell<ActionRegistry<#struct_name>> = std::cell::RefCell::new(ActionRegistry::new());
        }
        
        // Define the registry structure
        struct ActionRegistry<T> {
            definitions: std::collections::HashMap<String, ActionDefinition>,
            handlers: std::collections::HashMap<String, fn(&T, &std::collections::HashMap<String, serde_json::Value>) -> ActionResult>,
        }
        
        impl<T> ActionRegistry<T> {
            fn new() -> Self {
                Self {
                    definitions: std::collections::HashMap::new(),
                    handlers: std::collections::HashMap::new(),
                }
            }
        }
        
        // Implement CpiExtension trait
        impl CpiExtension for #struct_name {
            fn name(&self) -> &str {
                #name
            }
            
            fn provider_type(&self) -> &str {
                #provider_type
            }
            
            fn list_actions(&self) -> Vec<String> {
                ACTION_REGISTRY.with(|registry| {
                    registry.borrow().definitions.keys().cloned().collect()
                })
            }
            
            fn get_action_definition(&self, action: &str) -> Option<ActionDefinition> {
                ACTION_REGISTRY.with(|registry| {
                    registry.borrow().definitions.get(action).cloned()
                })
            }
            
            fn execute_action(&self, action: &str, params: &std::collections::HashMap<String, serde_json::Value>) -> ActionResult {
                ACTION_REGISTRY.with(|registry| {
                    let registry = registry.borrow();
                    if let Some(handler) = registry.handlers.get(action) {
                        handler(self, params)
                    } else {
                        Err(format!("Action '{}' not found", action))
                    }
                })
            }
        }
    };
    
    result.into()
}