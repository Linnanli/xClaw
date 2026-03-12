#!/bin/bash
# scripts/scaffold-tdd-module.sh
# Usage: ./scripts/scaffold-tdd-module.sh <module_path.rs>

MODULE_PATH=$1

if [ -z "$MODULE_PATH" ]; then
    echo "Usage: $0 <module_path.rs>"
    exit 1
fi

DIR=$(dirname "$MODULE_PATH")
mkdir -p "$DIR"

echo "Creating TDD-ready module at $MODULE_PATH..."

cat > "$MODULE_PATH" <<EOL
use anyhow::Result;
use thiserror::Error;
use tracing::{info, instrument};

#[derive(Debug, Error)]
pub enum ModuleError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Internal error")]
    Internal(#[from] std::io::Error),
}

/// Core function description
#[instrument]
pub fn process_data(input: &str) -> Result<String, ModuleError> {
    // TODO: Implement logic
    if input.is_empty() {
        return Err(ModuleError::InvalidInput("Input cannot be empty".to_string()));
    }
    
    info!("Processing data");
    Ok(input.to_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn test_process_data_happy_path() {
        // Arrange
        let input = "hello";
        
        // Act
        let result = process_data(input);
        
        // Assert
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "HELLO");
    }

    #[test]
    fn test_process_data_empty_input() {
        // Arrange
        let input = "";
        
        // Act
        let result = process_data(input);
        
        // Assert
        assert!(result.is_err());
        match result.unwrap_err() {
            ModuleError::InvalidInput(msg) => assert_eq!(msg, "Input cannot be empty"),
            _ => panic!("Wrong error type"),
        }
    }

    proptest! {
        #[test]
        fn test_process_data_does_not_crash(s in "\\PC*") {
            let _ = process_data(&s);
        }
    }
}
EOL

echo "Module created."
