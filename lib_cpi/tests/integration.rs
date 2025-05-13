//! Integration tests for the CPI extension system
//! 
//! This file tests the lib_cpi functionality without relying on the #[action] macro

use lib_cpi::{
    ActionParameter, ActionDefinition, ActionResult, CpiExtension, ParamType,
    param, response, validation
};
use serde_json::{json, Value};
use std::collections::HashMap;

// Mock extension for testing
struct MockExtension {
    name: String,
    provider_type: String,
    default_settings: HashMap<String, Value>,
}

impl MockExtension {
    fn new() -> Self {
        let mut default_settings = HashMap::new();
        default_settings.insert("setting1".to_string(), json!("value1"));
        default_settings.insert("setting2".to_string(), json!(42));
        
        Self {
            name: "mock_extension".to_string(),
            provider_type: "test".to_string(),
            default_settings,
        }
    }
    
    // Regular methods without the #[action] attribute for now
    fn test_no_params(&self) -> ActionResult {
        Ok(json!({
            "success": true,
            "message": "Action executed successfully"
        }))
    }
    
    fn test_with_params(&self, name: String, count: i64) -> ActionResult {
        Ok(json!({
            "success": true,
            "message": format!("Hello, {}! Count: {}", name, count)
        }))
    }
    
    fn test_complex_return(&self, include_details: bool) -> ActionResult {
        let mut result = json!({
            "success": true,
            "timestamp": "2023-09-30T12:00:00Z", // Static timestamp for testing
            "message": "Complex data returned"
        });
        
        if include_details {
            if let Value::Object(ref mut obj) = result {
                obj.insert("details".to_string(), json!({
                    "system": std::env::consts::OS,
                    "numbers": [1, 2, 3, 4, 5],
                    "nested": {
                        "a": 1,
                        "b": "test",
                        "c": true
                    }
                }));
            }
        }
        
        Ok(result)
    }
    
    fn test_error(&self, should_fail: bool) -> ActionResult {
        if should_fail {
            Err("This action failed intentionally".to_string())
        } else {
            Ok(json!({
                "success": true,
                "message": "Action did not fail"
            }))
        }
    }
}

impl CpiExtension for MockExtension {
    fn name(&self) -> &str {
        &self.name
    }
    
    fn provider_type(&self) -> &str {
        &self.provider_type
    }
    
    fn list_actions(&self) -> Vec<String> {
        vec![
            "test_install".to_string(),
            "test_no_params".to_string(),
            "test_with_params".to_string(),
            "test_complex_return".to_string(),
            "test_error".to_string()
        ]
    }
    
    fn get_action_definition(&self, action: &str) -> Option<ActionDefinition> {
        match action {
            "test_install" => Some(ActionDefinition {
                name: "test_install".to_string(),
                description: "Test if the extension is properly installed".to_string(),
                parameters: vec![],
            }),
            "test_no_params" => Some(ActionDefinition {
                name: "test_no_params".to_string(),
                description: "Test action with no parameters".to_string(),
                parameters: vec![],
            }),
            "test_with_params" => Some(ActionDefinition {
                name: "test_with_params".to_string(),
                description: "Test action with string and integer parameters".to_string(),
                parameters: vec![
                    param!("name", "Name parameter", ParamType::String, required),
                    param!("count", "Count parameter", ParamType::Number, required),
                ],
            }),
            "test_complex_return" => Some(ActionDefinition {
                name: "test_complex_return".to_string(),
                description: "Test action with complex return value".to_string(),
                parameters: vec![
                    param!("include_details", "Include detailed information", ParamType::Boolean, optional, json!(false)),
                ],
            }),
            "test_error" => Some(ActionDefinition {
                name: "test_error".to_string(),
                description: "Test action that returns an error".to_string(),
                parameters: vec![
                    param!("should_fail", "Should the action fail?", ParamType::Boolean, required),
                ],
            }),
            _ => None,
        }
    }
    
    fn execute_action(&self, action: &str, params: &HashMap<String, Value>) -> ActionResult {
        match action {
            "test_install" => self.test_install(),
            "test_no_params" => self.test_no_params(),
            "test_with_params" => {
                let name = validation::extract_string(params, "name")?;
                let count = validation::extract_int(params, "count")?;
                self.test_with_params(name, count)
            },
            "test_complex_return" => {
                let include_details = params.get("include_details")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                self.test_complex_return(include_details)
            },
            "test_error" => {
                let should_fail = validation::extract_bool(params, "should_fail")?;
                self.test_error(should_fail)
            },
            _ => Err(format!("Unknown action: {}", action)),
        }
    }
    
    fn default_settings(&self) -> HashMap<String, Value> {
        self.default_settings.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_extension_metadata() {
        let extension = MockExtension::new();
        
        // Check basic properties
        assert_eq!(extension.name(), "mock_extension");
        assert_eq!(extension.provider_type(), "test");
        
        // Check default settings
        let default_settings = extension.default_settings();
        assert_eq!(default_settings.get("setting1").unwrap().as_str().unwrap(), "value1");
        assert_eq!(default_settings.get("setting2").unwrap().as_i64().unwrap(), 42);
        
        // Check action list
        let actions = extension.list_actions();
        assert!(actions.contains(&"test_install".to_string()));
        assert!(actions.contains(&"test_no_params".to_string()));
        assert!(actions.contains(&"test_with_params".to_string()));
        assert!(actions.contains(&"test_complex_return".to_string()));
        assert!(actions.contains(&"test_error".to_string()));
        
        // Check action definitions
        let test_install_def = extension.get_action_definition("test_install").unwrap();
        assert_eq!(test_install_def.name, "test_install");
        assert!(test_install_def.parameters.is_empty());
        
        let test_with_params_def = extension.get_action_definition("test_with_params").unwrap();
        assert_eq!(test_with_params_def.name, "test_with_params");
        assert_eq!(test_with_params_def.parameters.len(), 2);
        assert_eq!(test_with_params_def.parameters[0].name, "name");
        assert_eq!(test_with_params_def.parameters[1].name, "count");
        
        // Check non-existent action
        assert!(extension.get_action_definition("non_existent_action").is_none());
    }
    
    #[test]
    fn test_action_execution() {
        let extension = MockExtension::new();
        
        // Test action with no params
        let result = extension.execute_action("test_no_params", &HashMap::new()).unwrap();
        assert_eq!(result["success"], json!(true));
        assert!(result["message"].is_string());
        
        // Test action with params
        let mut params = HashMap::new();
        params.insert("name".to_string(), json!("World"));
        params.insert("count".to_string(), json!(123));
        
        let result = extension.execute_action("test_with_params", &params).unwrap();
        assert_eq!(result["success"], json!(true));
        assert_eq!(result["message"], json!("Hello, World! Count: 123"));
        
        // Test complex return value
        let mut params = HashMap::new();
        params.insert("include_details".to_string(), json!(true));
        
        let result = extension.execute_action("test_complex_return", &params).unwrap();
        assert_eq!(result["success"], json!(true));
        assert!(result["details"].is_object());
        assert!(result["details"]["system"].is_string());
        assert!(result["details"]["numbers"].is_array());
        assert!(result["details"]["nested"].is_object());
        
        // Test error return
        let mut params = HashMap::new();
        params.insert("should_fail".to_string(), json!(true));
        
        let error = extension.execute_action("test_error", &params).unwrap_err();
        assert_eq!(error, "This action failed intentionally");
        
        // Test with should_fail = false
        let mut params = HashMap::new();
        params.insert("should_fail".to_string(), json!(false));
        
        let result = extension.execute_action("test_error", &params).unwrap();
        assert_eq!(result["success"], json!(true));
    }
    
    #[test]
    fn test_parameter_validation() {
        // Test string extraction
        let mut params = HashMap::new();
        params.insert("str_param".to_string(), json!("test string"));
        
        let str_result = validation::extract_string(&params, "str_param").unwrap();
        assert_eq!(str_result, "test string");
        
        // Test int extraction
        params.insert("int_param".to_string(), json!(42));
        
        let int_result = validation::extract_int(&params, "int_param").unwrap();
        assert_eq!(int_result, 42);
        
        // Test bool extraction
        params.insert("bool_param".to_string(), json!(true));
        
        let bool_result = validation::extract_bool(&params, "bool_param").unwrap();
        assert_eq!(bool_result, true);
        
        // Test optional parameter
        let opt_result = validation::extract_string_opt(&params, "missing_param").unwrap();
        assert_eq!(opt_result, None);
        
        // Test missing required parameter
        let err = validation::extract_string(&params, "missing_param").unwrap_err();
        assert!(err.contains("not provided"));
        
        // Test type mismatch
        let err = validation::extract_string(&params, "int_param").unwrap_err();
        assert!(err.contains("must be a string"));
    }
    
    #[test]
    fn test_response_helpers() {
        // Test success with data
        let success_data = response::success(Some(json!({"key": "value"})));
        assert_eq!(success_data["success"], json!(true));
        assert_eq!(success_data["data"]["key"], json!("value"));
        
        // Test success without data
        let success_no_data = response::success(None);
        assert_eq!(success_no_data["success"], json!(true));
        assert!(!success_no_data.as_object().unwrap().contains_key("data"));
        
        // Test boolean result
        let bool_result = response::bool_result(true);
        assert_eq!(bool_result["success"], json!(true));
        assert_eq!(bool_result["result"], json!(true));
        
        // Test error
        let error_result = response::error("Something went wrong");
        assert_eq!(error_result["success"], json!(false));
        assert_eq!(error_result["error"], json!("Something went wrong"));
    }
}