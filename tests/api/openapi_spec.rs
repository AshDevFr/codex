//! Document-level invariants for the generated OpenAPI spec.
//!
//! These guard properties that permissive tooling (Swagger UI, the TypeScript
//! generator the web app uses) silently tolerates but that strict generators
//! reject or, worse, skip without failing. A skipped property produces a client
//! that compiles, passes its tests, and cannot see the data.

use codex::api::ApiDoc;
use serde_json::Value;
use utoipa::OpenApi;

/// Serialize the spec the same way `codex openapi` and the runtime endpoint do.
fn spec() -> Value {
    serde_json::to_value(ApiDoc::openapi()).expect("spec should serialize")
}

fn operations(spec: &Value) -> impl Iterator<Item = (String, &Value)> {
    spec["paths"]
        .as_object()
        .expect("paths object")
        .iter()
        .flat_map(|(path, item)| {
            item.as_object()
                .expect("path item object")
                .iter()
                .filter(|(method, _)| {
                    matches!(
                        method.as_str(),
                        "get" | "put" | "post" | "delete" | "options" | "head" | "patch" | "trace"
                    )
                })
                .map(move |(method, op)| (format!("{} {}", method.to_uppercase(), path), op))
        })
}

/// Walk every node in the document, yielding `(json_pointer, node)`.
fn walk(node: &Value, path: String, out: &mut Vec<(String, Value)>) {
    match node {
        Value::Object(map) => {
            out.push((path.clone(), node.clone()));
            for (key, value) in map {
                walk(value, format!("{}/{}", path, key), out);
            }
        }
        Value::Array(items) => {
            for (index, value) in items.iter().enumerate() {
                walk(value, format!("{}/{}", path, index), out);
            }
        }
        _ => {}
    }
}

fn all_nodes(spec: &Value) -> Vec<(String, Value)> {
    let mut out = Vec::new();
    walk(spec, String::new(), &mut out);
    out
}

/// Every key of a Security Requirement object must name a scheme that exists in
/// `components.securitySchemes`, otherwise the document is invalid per the
/// OpenAPI specification and strict generators refuse to parse it at all.
#[test]
fn every_referenced_security_scheme_is_defined() {
    let spec = spec();
    let defined: Vec<&str> = spec["components"]["securitySchemes"]
        .as_object()
        .expect("securitySchemes object")
        .keys()
        .map(String::as_str)
        .collect();

    let mut undefined: Vec<String> = Vec::new();
    for (operation, op) in operations(&spec) {
        let Some(requirements) = op.get("security").and_then(Value::as_array) else {
            continue;
        };
        for requirement in requirements {
            for scheme in requirement
                .as_object()
                .expect("security requirement")
                .keys()
            {
                if !defined.contains(&scheme.as_str()) {
                    undefined.push(format!("{} -> {}", operation, scheme));
                }
            }
        }
    }
    undefined.sort();
    undefined.dedup();

    assert!(
        undefined.is_empty(),
        "operations reference undefined security schemes: {:#?}",
        undefined
    );
}

/// `Option<T>` where `T` is a referenced schema renders as
/// `oneOf: [{"type": "null"}, {"$ref": ...}]`. Strict generators do not support
/// the `null` type and skip the enclosing property outright, so the document
/// must express optionality by omission from `required` instead.
#[test]
fn no_nullable_ref_unions_remain() {
    let spec = spec();

    let offenders: Vec<String> = all_nodes(&spec)
        .into_iter()
        .filter(|(_, node)| {
            node.get("oneOf")
                .and_then(Value::as_array)
                .is_some_and(|branches| {
                    branches
                        .iter()
                        .any(|branch| branch.get("type") == Some(&Value::String("null".into())))
                })
        })
        .map(|(path, _)| path)
        .collect();

    assert!(
        offenders.is_empty(),
        "nullable-ref unions remain at: {:#?}",
        offenders
    );
}

/// A multipart request body must be marked required; an optional one is skipped
/// by strict generators, leaving the operation unable to upload anything.
#[test]
fn multipart_request_bodies_are_required() {
    let spec = spec();

    let mut offenders: Vec<String> = Vec::new();
    for (operation, op) in operations(&spec) {
        let Some(body) = op.get("requestBody") else {
            continue;
        };
        let is_multipart = body["content"]
            .as_object()
            .is_some_and(|content| content.keys().any(|ct| ct.starts_with("multipart/")));
        if !is_multipart {
            continue;
        }
        if body.get("required") != Some(&Value::Bool(true)) {
            offenders.push(format!("{}: not required", operation));
        }
        let has_schema = body["content"]
            .as_object()
            .expect("content object")
            .values()
            .all(|media| media.get("schema").is_some());
        if !has_schema {
            offenders.push(format!("{}: no schema", operation));
        }
    }

    assert!(
        offenders.is_empty(),
        "multipart bodies that strict generators would skip: {:#?}",
        offenders
    );
}

/// The endpoints that used to answer `200` with a JSON `null` body now answer
/// `204 No Content`, so their `200` body is a plain schema reference.
#[test]
fn absent_value_endpoints_document_204() {
    let spec = spec();

    for (path, method) in [
        ("/api/v1/books/{book_id}/progress", "get"),
        ("/api/v1/series/{series_id}/rating", "get"),
        ("/api/v1/user/plugins/{plugin_id}/tasks", "get"),
    ] {
        let responses = &spec["paths"][path][method]["responses"];
        assert!(
            responses.get("204").is_some(),
            "{} {} should document 204 for the absent case",
            method,
            path
        );
        let ok_schema = &responses["200"]["content"]["application/json"]["schema"];
        assert!(
            ok_schema.get("$ref").is_some(),
            "{} {} 200 body should be a plain $ref, got {}",
            method,
            path,
            ok_schema
        );
    }
}
