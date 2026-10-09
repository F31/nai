//! Synthetic bytes test integrity checks only; these are not ONNX models.
use nai_contracts::v1::ErrorCode;
use nai_plugin_sdk::model::{ModelFile, ModelLimits, check_model_integrity};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

fn hash(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn fixture() -> Value {
    json!({
        "schema_version":1,"model_id":"test-only","version":"0.1.0",
        "license_ref":"test-not-authorization",
        "graphs":[{"id":"encoder","file":{"path":"encoder.onnx","sha256":hash(b"graph"),"license_ref":"test"},
        "external_weights":[{"path":"weights.bin","sha256":hash(b"weights"),"license_ref":"test"}],
        "inputs":[{"name":"audio","dtype":"float32","shape":[1,"samples"]}],
        "outputs":[{"name":"features","dtype":"float32","shape":[1,32]}],"opset":17}],
        "required_graph_ids":["encoder"],"preprocess_revision":"test","postprocess_revision":"test",
        "sample_rate":16000,"window_samples":640,"hop_samples":320,"lookahead_samples":320,
        "padding":"test","normalization":"test","frame_mapping":"test","fps_num":25,"fps_den":1,
        "runtime_matrix":[{"os":"windows","arch":"x86_64","ort":"test-version",
        "ort_crate":"test-version","ep":"cuda","precision":"fp32","driver":"test-driver",
        "cuda":"test-cuda","cudnn":"test-cudnn","certification_ref":"not-certified"}],
        "bom_ref":"test","golden_ref":"test"
    })
}
fn files() -> [ModelFile<'static>; 2] {
    [
        ModelFile {
            path: "encoder.onnx",
            content: b"graph",
        },
        ModelFile {
            path: "weights.bin",
            content: b"weights",
        },
    ]
}
fn check(
    value: &Value,
) -> Result<nai_plugin_sdk::model::ModelIntegrity, nai_contracts::v1::ContractError> {
    check_model_integrity(
        &serde_json::to_vec(value).unwrap(),
        &files(),
        &fixture()["runtime_matrix"][0],
        ModelLimits {
            max_files: 8,
            max_bytes: 1024,
        },
        Instant::now() + Duration::from_secs(5),
        &AtomicBool::new(false),
    )
}

#[test]
fn checks_all_content_without_granting_execution() {
    let value = fixture();
    let result = check(&value).unwrap();
    assert_eq!(result.file_count, 2);
    assert_eq!(result.content_bytes, 12);
    assert_eq!(
        result.manifest_hash,
        hash(&serde_json::to_vec(&value).unwrap())
    );
}

#[test]
fn rejects_bad_schema_and_missing_or_duplicate_graphs() {
    let mut value = fixture();
    value["required_graph_ids"] = json!(["encoder", "generator"]);
    assert_eq!(
        check(&value).unwrap_err().code,
        ErrorCode::InvalidArgument as i32
    );
    let mut value = fixture();
    let graph = value["graphs"][0].clone();
    value["graphs"].as_array_mut().unwrap().push(graph);
    assert!(check(&value).is_err());
    for field in ["license_ref", "preprocess_revision", "golden_ref"] {
        let mut value = fixture();
        value.as_object_mut().unwrap().remove(field);
        assert!(check(&value).is_err(), "{field}");
    }
    let mut value = fixture();
    value["ready"] = json!(true);
    assert!(check(&value).is_err());
}

#[test]
fn rejects_path_aliases_and_duplicate_file_declarations() {
    for path in [
        "/encoder.onnx",
        "../encoder.onnx",
        "x/../encoder.onnx",
        "./encoder.onnx",
        "x//encoder.onnx",
        "x/./encoder.onnx",
        "C:/encoder.onnx",
        "x\\encoder.onnx",
        "encoder.onnx/",
    ] {
        let mut value = fixture();
        value["graphs"][0]["file"]["path"] = json!(path);
        assert!(check(&value).is_err(), "{path}");
    }
    let mut value = fixture();
    value["graphs"][0]["external_weights"][0]["path"] = json!("encoder.onnx");
    assert!(check(&value).is_err());
}

#[test]
fn rejects_missing_extra_duplicate_and_tampered_content() {
    let value = fixture();
    let bytes = serde_json::to_vec(&value).unwrap();
    let missing = [ModelFile {
        path: "encoder.onnx",
        content: b"graph",
    }];
    let extra = [
        ModelFile {
            path: "encoder.onnx",
            content: b"graph",
        },
        ModelFile {
            path: "weights.bin",
            content: b"weights",
        },
        ModelFile {
            path: "extra.bin",
            content: b"extra",
        },
    ];
    let duplicate = [
        ModelFile {
            path: "encoder.onnx",
            content: b"graph",
        },
        ModelFile {
            path: "encoder.onnx",
            content: b"graph",
        },
    ];
    let tampered = [
        ModelFile {
            path: "encoder.onnx",
            content: b"graph",
        },
        ModelFile {
            path: "weights.bin",
            content: b"changed",
        },
    ];
    for files in [&missing[..], &extra[..], &duplicate[..], &tampered[..]] {
        let result = check_model_integrity(
            &bytes,
            files,
            &value["runtime_matrix"][0],
            ModelLimits {
                max_files: 8,
                max_bytes: 1024,
            },
            Instant::now() + Duration::from_secs(5),
            &AtomicBool::new(false),
        );
        assert_eq!(result.unwrap_err().code, ErrorCode::InvalidArgument as i32);
    }
}

#[test]
fn exact_runtime_match_has_no_cpu_fallback() {
    for field in [
        "os",
        "arch",
        "ort",
        "ort_crate",
        "ep",
        "precision",
        "driver",
        "cuda",
        "cudnn",
        "certification_ref",
    ] {
        let mut value = fixture();
        value["runtime_matrix"][0][field] = json!(match field {
            "ep" => "cpu",
            "precision" => "fp16",
            _ => "different",
        });
        assert_eq!(
            check(&value).unwrap_err().code,
            ErrorCode::VersionIncompatible as i32,
            "{field}"
        );
    }
}

#[test]
fn enforces_manifest_count_bytes_deadline_and_cancellation() {
    let value = fixture();
    let bytes = serde_json::to_vec(&value).unwrap();
    for limits in [
        ModelLimits {
            max_files: 1,
            max_bytes: 1024,
        },
        ModelLimits {
            max_files: 8,
            max_bytes: 11,
        },
    ] {
        let result = check_model_integrity(
            &bytes,
            &files(),
            &value["runtime_matrix"][0],
            limits,
            Instant::now() + Duration::from_secs(5),
            &AtomicBool::new(false),
        );
        assert_eq!(
            result.unwrap_err().code,
            ErrorCode::ResourceExhausted as i32
        );
    }
    for (deadline, cancelled, expected) in [
        (Instant::now(), false, ErrorCode::DeadlineExceeded),
        (
            Instant::now() + Duration::from_secs(5),
            true,
            ErrorCode::Cancelled,
        ),
    ] {
        let result = check_model_integrity(
            &bytes,
            &files(),
            &value["runtime_matrix"][0],
            ModelLimits {
                max_files: 8,
                max_bytes: 1024,
            },
            deadline,
            &AtomicBool::new(cancelled),
        );
        assert_eq!(result.unwrap_err().code, expected as i32);
    }
    let result = check_model_integrity(
        &vec![b' '; 65537],
        &[],
        &json!({}),
        ModelLimits {
            max_files: 8,
            max_bytes: 1024,
        },
        Instant::now() + Duration::from_secs(5),
        &AtomicBool::new(false),
    );
    assert_eq!(
        result.unwrap_err().code,
        ErrorCode::ResourceExhausted as i32
    );
    // Declared count is bounded even when no content is supplied yet.
    let result = check_model_integrity(
        &bytes,
        &[],
        &value["runtime_matrix"][0],
        ModelLimits {
            max_files: 1,
            max_bytes: 1024,
        },
        Instant::now() + Duration::from_secs(5),
        &AtomicBool::new(false),
    );
    assert_eq!(
        result.unwrap_err().code,
        ErrorCode::ResourceExhausted as i32
    );
}
