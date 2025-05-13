// File: lib_cpi/src/lib.rs
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionParameter {
    pub name: String,
    pub description: String,
    pub required: bool,
    pub param_type: ParamType,
    pub default_value: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ParamType {
    String,
    Number,
    Boolean,
    Object,
    Array,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Vec<ActionParameter>,
}

pub type ActionResult = Result<Value, String>;

/// Main trait that must be implemented by CPI extensions
pub trait CpiExtension: Send + Sync {
    /// Returns the name of the extension
    fn name(&self) -> &str;
    
    /// Returns the provider type
    fn provider_type(&self) -> &str;
    
    /// Returns all available actions
    fn list_actions(&self) -> Vec<String>;
    
    /// Returns the definition of a specific action
    fn get_action_definition(&self, action: &str) -> Option<ActionDefinition>;
    
    /// Executes an action with the given parameters
    fn execute_action(&self, action: &str, params: &HashMap<String, Value>) -> ActionResult;
    
    /// Optional method that returns default parameter values for the provider
    fn default_settings(&self) -> HashMap<String, Value> {
        HashMap::new()
    }
    
    /// Test if the extension is properly installed
    fn test_install(&self) -> ActionResult {
        // Default implementation returns success
        Ok(serde_json::json!({"status": "ok"}))
    }

    fn version(&self) -> String {
        // Default implementation returns a placeholder version
        "NONE".to_string()
    }
}

// Required function signature for dynamic registration
pub type GetExtensionFn = unsafe extern "C" fn() -> *mut dyn CpiExtension;

// Explicitly re-export the action macro from lib_cpi_macros
// This ensures the macro is available when users import lib_cpi
pub use lib_cpi_macros::action;

// Entry point macro that every extension DLL must implement
#[macro_export]
macro_rules! register_extension {
    ($ext_type:ty) => {
        #[no_mangle]
        pub unsafe extern "C" fn get_extension() -> *mut dyn $crate::CpiExtension {
            // Create a Box containing the extension implementation
            let extension = Box::new(<$ext_type>::new());
            // Convert the Box into a raw pointer and return it
            Box::into_raw(extension)
        }
    };
}

// Helper functions for parameter validation
pub mod validation {
    use super::*;
    
    pub fn extract_string(params: &HashMap<String, Value>, name: &str) -> Result<String, String> {
        match params.get(name) {
            Some(Value::String(s)) => Ok(s.clone()),
            Some(_) => Err(format!("Parameter '{}' must be a string", name)),
            None => Err(format!("Required parameter '{}' not provided", name)),
        }
    }
    
    pub fn extract_string_opt(params: &HashMap<String, Value>, name: &str) -> Result<Option<String>, String> {
        match params.get(name) {
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(_) => Err(format!("Parameter '{}' must be a string", name)),
            None => Ok(None),
        }
    }
    
    pub fn extract_int(params: &HashMap<String, Value>, name: &str) -> Result<i64, String> {
        match params.get(name) {
            Some(Value::Number(n)) if n.is_i64() => Ok(n.as_i64().unwrap()),
            Some(_) => Err(format!("Parameter '{}' must be an integer", name)),
            None => Err(format!("Required parameter '{}' not provided", name)),
        }
    }
    
    pub fn extract_int_opt(params: &HashMap<String, Value>, name: &str) -> Result<Option<i64>, String> {
        match params.get(name) {
            Some(Value::Number(n)) if n.is_i64() => Ok(Some(n.as_i64().unwrap())),
            Some(_) => Err(format!("Parameter '{}' must be an integer", name)),
            None => Ok(None),
        }
    }
    
    pub fn extract_float(params: &HashMap<String, Value>, name: &str) -> Result<f64, String> {
        match params.get(name) {
            Some(Value::Number(n)) => Ok(n.as_f64().unwrap()),
            Some(_) => Err(format!("Parameter '{}' must be a number", name)),
            None => Err(format!("Required parameter '{}' not provided", name)),
        }
    }
    
    pub fn extract_bool(params: &HashMap<String, Value>, name: &str) -> Result<bool, String> {
        match params.get(name) {
            Some(Value::Bool(b)) => Ok(*b),
            Some(_) => Err(format!("Parameter '{}' must be a boolean", name)),
            None => Err(format!("Required parameter '{}' not provided", name)),
        }
    }
    
    pub fn extract_json(params: &HashMap<String, Value>, name: &str) -> Result<Value, String> {
        match params.get(name) {
            Some(v) => Ok(v.clone()),
            None => Err(format!("Required parameter '{}' not provided", name)),
        }
    }
    
    pub fn validate_params(
        params: &HashMap<String, Value>, 
        required: &[&str]
    ) -> Result<(), String> {
        for &param in required {
            if !params.contains_key(param) {
                return Err(format!("Required parameter '{}' not provided", param));
            }
        }
        Ok(())
    }
}

// Helper macro to simplify creating parameter definitions
#[macro_export]
macro_rules! param {
    ($name:expr, $desc:expr, $type:expr, required) => {
        $crate::ActionParameter {
            name: $name.to_string(),
            description: $desc.to_string(),
            required: true,
            param_type: $type,
            default_value: None,
        }
    };
    ($name:expr, $desc:expr, $type:expr, optional, $default:expr) => {
        $crate::ActionParameter {
            name: $name.to_string(),
            description: $desc.to_string(),
            required: false,
            param_type: $type,
            default_value: Some($default),
        }
    };
    ($name:expr, $desc:expr, $type:expr, optional) => {
        $crate::ActionParameter {
            name: $name.to_string(),
            description: $desc.to_string(),
            required: false,
            param_type: $type,
            default_value: None,
        }
    };
}

// Helper module to create standard action responses
pub mod response {
    use serde_json::{json, Value};
    
    pub fn success(data: Option<Value>) -> Value {
        match data {
            Some(value) => json!({
                "success": true,
                "data": value,
            }),
            None => json!({
                "success": true,
            }),
        }
    }
    
    pub fn bool_result(result: bool) -> Value {
        json!({
            "success": true,
            "result": result
        })
    }
    
    pub fn error(message: impl AsRef<str>) -> Value {
        json!({
            "success": false,
            "error": message.as_ref(),
        })
    }
}