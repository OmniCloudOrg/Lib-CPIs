// File: lib_cpi_macros/src/lib.rs
extern crate proc_macro;
use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{
    parse_macro_input, ItemFn, FnArg, Pat, PatIdent, PatType, 
    Ident, LitStr, ItemStruct, Expr, ExprLit, Lit,
    Meta, MetaNameValue, Token, parse::Parse, parse::ParseStream,
    Result, punctuated::Punctuated
};

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
                    if let Expr::Lit(expr_lit) = &nv.value {
                        if let Lit::Str(s) = &expr_lit.lit {
                            description = s.value();
                        }
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
                    if let Expr::Lit(expr_lit) = &nv.value {
                        if let Lit::Str(s) = &expr_lit.lit {
                            name = Some(s.value());
                        }
                    }
                },
                Meta::NameValue(nv) if nv.path.is_ident("provider_type") => {
                    if let Expr::Lit(expr_lit) = &nv.value {
                        if let Lit::Str(s) = &expr_lit.lit {
                            provider_type = Some(s.value());
                        }
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

/// Macro for defining CPI actions with rich metadata
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
    
    // Extract the description to use in quote macro
    let description = meta.description.clone();
    
    // Generate parameters for function definition
    let params = &input_fn.sig.inputs;
    
    // Generate param names for registration
    let param_names = meta.params.iter().map(|p| &p.name).collect::<Vec<_>>();
    let param_descriptions = meta.params.iter().map(|p| &p.description).collect::<Vec<_>>();
    let param_types = meta.params.iter().map(|p| &p.param_type).collect::<Vec<_>>();
    let param_requireds = meta.params.iter().map(|p| p.required).collect::<Vec<_>>();
    let param_defaults = meta.params.iter().map(|p| p.default_value.as_ref()).collect::<Vec<_>>();
    
    // Generate the metadata registration function name
    let meta_fn_name = format_ident!("{}_metadata", fn_name);
    
    // Create parameter definitions for the metadata function
    let mut param_defs = Vec::new();
    
    for (i, param) in meta.params.iter().enumerate() {
        let name = &param.name;
        let name_str = name.to_string();
        let description = &param.description;
        let param_type = &param.param_type;
        let required = param.required;
        
        let param_def = if let Some(default_val) = &param.default_value {
            quote! {
                param!(
                    #name_str, 
                    #description, 
                    ParamType::#param_type, 
                    if #required { required } else { optional },
                    #default_val
                )
            }
        } else {
            quote! {
                param!(
                    #name_str, 
                    #description, 
                    ParamType::#param_type, 
                    if #required { required } else { optional }
                )
            }
        };
        
        param_defs.push(param_def);
    }
    
    // Generate the metadata function
    let meta_fn = quote! {
        fn #meta_fn_name() -> ActionDefinition {
            ActionDefinition {
                name: #fn_name_str.to_string(),
                description: #description.to_string(),
                parameters: vec![
                    #(#param_defs),*
                ],
            }
        }
    };
    
    // Add action attribute to the function and register its metadata
    let result = quote! {
        #(#attrs)*
        #[action]
        #vis fn #fn_name(#params) #output #block
        
        #meta_fn
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
        
        // Implement CpiExtension trait
        impl CpiExtension for #struct_name {
            fn name(&self) -> &str {
                #name
            }
            
            fn provider_type(&self) -> &str {
                #provider_type
            }
            
            fn list_actions(&self) -> Vec<String> {
                // Use reflection to find all *_metadata functions
                vec![
                    // Functions will be discovered at runtime
                ]
            }
            
            fn get_action_definition(&self, action: &str) -> Option<ActionDefinition> {
                // Match the action name with the corresponding metadata function
                match action {
                    // Each action will be added here
                    _ => None,
                }
            }
            
            fn execute_action(&self, action: &str, params: &std::collections::HashMap<String, serde_json::Value>) -> ActionResult {
                // Match the action name and execute the corresponding function
                match action {
                    // Each action will be added here
                    _ => Err(format!("Action '{}' not found", action)),
                }
            }
        }
    };
    
    result.into()
}