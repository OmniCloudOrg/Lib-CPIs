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

// File: lib_cpi/src/macros.rs
use proc_macro::TokenStream;
use quote::{quote, format_ident};
use syn::{
    parse_macro_input, parse_quote, Attribute, AttributeArgs, Block, Expr, ExprLit, 
    FnArg, Ident, ItemFn, ItemImpl, ItemStruct, Lit, LitStr, Meta, NestedMeta, 
    Pat, PatIdent, PatType, Path, Type, parse::Parse, parse::ParseStream, 
    punctuated::Punctuated, token::Comma, Result
};
use proc_macro2::{Span, TokenStream as TokenStream2};

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
        content.parse::<Comma>()?;
        
        // Parse description as a string literal
        let desc_lit: LitStr = content.parse()?;
        let description = desc_lit.value();
        content.parse::<Comma>()?;
        
        // Parse parameter type
        let param_type: Ident = content.parse()?;
        content.parse::<Comma>()?;
        
        // Parse required/optional flag
        let required_ident: Ident = content.parse()?;
        let required = required_ident == "required";
        
        // Parse default value if present
        let default_value = if content.peek(Comma) {
            content.parse::<Comma>()?;
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
        
        while !input.is_empty() {
            let lookahead = input.lookahead1();
            
            if lookahead.peek(Ident) {
                let ident: Ident = input.parse()?;
                
                if ident == "description" {
                    input.parse::<syn::Token![=]>()?;
                    let desc_lit: LitStr = input.parse()?;
                    description = desc_lit.value();
                    
                    if input.peek(Comma) {
                        input.parse::<Comma>()?;
                    }
                } else if ident == "param" {
                    let param: ParamDef = input.parse()?;
                    params.push(param);
                    
                    if input.peek(Comma) {
                        input.parse::<Comma>()?;
                    }
                } else {
                    return Err(input.error("Expected 'description' or 'param'"));
                }
            } else {
                return Err(lookahead.error());
            }
        }
        
        Ok(CpiActionMeta {
            description,
            params,
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
    
    // Generate action registration code
    // This will be collected and used by the cpi_extension macro
    let action_registration = quote! {
        actions.insert(
            #fn_name_str.to_string(),
            ActionDefinition {
                name: #fn_name_str.to_string(),
                description: #meta.description.to_string(),
                parameters: vec![
                    #(param!(
                        stringify!(#meta.params.name), 
                        #meta.params.description, 
                        ParamType::#meta.params.param_type, 
                        #meta.params.required
                        #(, #meta.params.default_value)*
                    )),*
                ],
            }
        );
        
        action_handlers.insert(
            #fn_name_str.to_string(),
            |slf, params| {
                // Extract parameters
                #(
                    let #meta.params.name = if #meta.params.required {
                        if stringify!(#meta.params.param_type) == "String" {
                            validation::extract_string(params, stringify!(#meta.params.name))?
                        } else {
                            validation::extract_int(params, stringify!(#meta.params.name))?
                        }
                    } else {
                        if stringify!(#meta.params.param_type) == "String" {
                            validation::extract_string_opt(params, stringify!(#meta.params.name))?.unwrap_or_else(|| {
                                #meta.params.default_value.unwrap_or_default()
                            })
                        } else {
                            validation::extract_int_opt(params, stringify!(#meta.params.name))?.unwrap_or_else(|| {
                                #meta.params.default_value.unwrap_or_default()
                            })
                        }
                    };
                )*
                
                // Call the actual method
                slf.#fn_name(#(#meta.params.name),*)
            }
        );
    };
    
    // Add action attribute to the function
    let result = quote! {
        #(#attrs)*
        #[action]
        #vis fn #fn_name(#params) #output #block
        
        // This metadata will be collected by the cpi_extension macro
        const _: () = {
            #[doc(hidden)]
            #[export_name = concat!("__cpi_action_", stringify!(#fn_name))]
            pub extern "C" fn __register_action() -> &'static str {
                stringify!(#action_registration)
            }
        };
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
    let args = parse_macro_input!(attr as AttributeArgs);
    
    let struct_name = &input_struct.ident;
    let struct_vis = &input_struct.vis;
    
    // Extract extension name and provider type from attributes
    let mut name = None;
    let mut provider_type = None;
    
    for arg in args {
        if let NestedMeta::Meta(Meta::NameValue(nv)) = arg {
            if nv.path.is_ident("name") {
                if let Expr::Lit(ExprLit { lit: Lit::Str(lit_str), .. }) = nv.lit {
                    name = Some(lit_str.value());
                }
            } else if nv.path.is_ident("provider_type") {
                if let Expr::Lit(ExprLit { lit: Lit::Str(lit_str), .. }) = nv.lit {
                    provider_type = Some(lit_str.value());
                }
            }
        }
    }
    
    let name = name.unwrap_or_else(|| struct_name.to_string().to_lowercase());
    let provider_type = provider_type.unwrap_or_else(|| "command".to_string());
    
    // Generate implementation of CpiExtension trait
    let cpi_extension_impl = quote! {
        impl CpiExtension for #struct_name {
            fn name(&self) -> &str {
                #name
            }
            
            fn provider_type(&self) -> &str {
                #provider_type
            }
            
            fn list_actions(&self) -> Vec<String> {
                // Collect all actions registered with #[cpi_action]
                self.registered_actions.keys().cloned().collect()
            }
            
            fn get_action_definition(&self, action: &str) -> Option<ActionDefinition> {
                self.registered_actions.get(action).cloned()
            }
            
            fn execute_action(&self, action: &str, params: &HashMap<String, Value>) -> ActionResult {
                if let Some(handler) = self.action_handlers.get(action) {
                    handler(self, params)
                } else {
                    Err(format!("Action '{}' not found", action))
                }
            }
        }
    };
    
    // Add fields for action registration to the struct
    let mut struct_fields = input_struct.fields.clone();
    
    // Add fields for registered_actions and action_handlers
    if let syn::Fields::Named(ref mut named_fields) = struct_fields {
        named_fields.named.push(parse_quote! {
            registered_actions: std::collections::HashMap<String, lib_cpi::ActionDefinition>
        });
        
        named_fields.named.push(parse_quote! {
            action_handlers: std::collections::HashMap<String, fn(&Self, &std::collections::HashMap<String, serde_json::Value>) -> lib_cpi::ActionResult>
        });
    }
    
    // Generate code to initialize action registrations in new() method
    let initialization_code = quote! {
        // Initialize action registrations
        let mut registered_actions = std::collections::HashMap::new();
        let mut action_handlers = std::collections::HashMap::new();
        
        // This will collect all actions registered with #[cpi_action]
        // The static metadata from each function will be integrated here
        
        registered_actions,
        action_handlers,
    };
    
    // Generate the final output
    let result = quote! {
        #struct_vis struct #struct_name #struct_fields
        
        #cpi_extension_impl
    };
    
    result.into()
}