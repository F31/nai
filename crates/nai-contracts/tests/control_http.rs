use nai_contracts::control_http::SCHEMAS_JSON;
use serde_json::{Value, json};

#[test]
fn generated_control_schema_flattens_inheritance_and_closes_commands() {
    let mut schema: Value = serde_json::from_str(SCHEMAS_JSON).unwrap();
    schema["$ref"] = json!("#/$defs/ModeCommand");
    let validator = jsonschema::validator_for(&schema).unwrap();
    let command = json!({"command_id":"test","expected_state_revision":"0","ttl_ms":1000,"session_id":"s","generation_epoch":"0","device_id":"d","seq":"1","mode":"assisted"});
    assert!(validator.is_valid(&command));
    for field in ["device_id", "seq", "mode", "command_id"] {
        let mut missing = command.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(!validator.is_valid(&missing), "{field}");
    }
    let mut unknown = command;
    unknown["credential_plaintext"] = json!("forbidden");
    assert!(!validator.is_valid(&unknown));
}
