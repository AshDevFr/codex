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

/// The OIDC login body carries an optional native redirect target. It must stay
/// optional, because the web app posts the endpoint with no payload at all, and
/// it must still be a plain `$ref` so strict generators can see the schema.
#[test]
fn oidc_login_body_is_optional_and_a_plain_ref() {
    let spec = spec();
    let body = &spec["paths"]["/api/v1/auth/oidc/{provider}/login"]["post"]["requestBody"];

    assert_ne!(
        body.get("required"),
        Some(&Value::Bool(true)),
        "a required body would force clients to send one; the web app sends none"
    );

    let schema = &body["content"]["application/json"]["schema"];
    assert_eq!(
        schema.get("$ref").and_then(Value::as_str),
        Some("#/components/schemas/OidcLoginRequest"),
        "body schema should be a plain $ref, got {schema}"
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

/// A generic DTO named through a `pub type` alias loses its type argument and
/// collapses to the base component, which then holds whichever instantiation the
/// schema registry happened to render into that slot. Four series endpoints were
/// documented as returning books this way, and the document generated without a
/// warning: the alias `SeriesListResponse` emitted `$ref: PaginatedResponse`, and
/// the bare `PaginatedResponse` carried `data: [BookDto]`.
///
/// The signature is a component whose name is the bare prefix of one or more
/// `Base_Argument` components in the same document. Referencing the bare form is
/// never what the handler means, so a `body =` position must name the generic
/// inline (`PaginatedResponse<SeriesDto>`) rather than through an alias.
#[test]
fn no_operation_references_an_unparameterised_generic_wrapper() {
    let spec = spec();
    let schemas = spec["components"]["schemas"]
        .as_object()
        .expect("schemas object");

    // Bases that the registry has rendered at least one concrete instantiation for.
    let parameterised: Vec<&str> = schemas
        .keys()
        .filter_map(|name| name.split_once('_').map(|(base, _)| base))
        .collect();

    let mut collapsed: Vec<String> = Vec::new();
    for (operation, op) in operations(&spec) {
        for (_, node) in all_nodes(op) {
            let Some(reference) = node.get("$ref").and_then(Value::as_str) else {
                continue;
            };
            let Some(name) = reference.strip_prefix("#/components/schemas/") else {
                continue;
            };
            if parameterised.contains(&name) {
                collapsed.push(format!("{} -> {}", operation, name));
            }
        }
    }
    collapsed.sort();
    collapsed.dedup();

    assert!(
        collapsed.is_empty(),
        "operations reference a generic wrapper with its type argument discarded, \
         so they document whichever instantiation the registry rendered rather than \
         their own: {:#?}",
        collapsed
    );
}

/// A parameter documented as `in: path` but absent from the path template
/// describes a call that cannot be constructed, and a `{placeholder}` with no
/// declared parameter describes one that cannot be filled in. Both halves broke
/// when four handlers annotated an `IntoParams` struct they never extracted:
/// `page` and `pageSize` rendered as path parameters of a path that has no such
/// segments.
#[test]
fn path_parameters_match_their_path_template() {
    let spec = spec();
    let mut mismatches: Vec<String> = Vec::new();

    for (path, item) in spec["paths"].as_object().expect("paths object") {
        let template: Vec<&str> = path
            .split('/')
            .filter_map(|segment| segment.strip_prefix('{')?.strip_suffix('}'))
            .collect();

        for (method, op) in item.as_object().expect("path item object") {
            if !matches!(
                method.as_str(),
                "get" | "put" | "post" | "delete" | "options" | "head" | "patch" | "trace"
            ) {
                continue;
            }
            let declared: Vec<&str> = op
                .get("parameters")
                .and_then(Value::as_array)
                .map(|params| {
                    params
                        .iter()
                        .filter(|p| p.get("in").and_then(Value::as_str) == Some("path"))
                        .filter_map(|p| p.get("name").and_then(Value::as_str))
                        .collect()
                })
                .unwrap_or_default();

            let operation = format!("{} {}", method.to_uppercase(), path);
            for name in &declared {
                if !template.contains(name) {
                    mismatches.push(format!(
                        "{}: declares path parameter `{}` that the template does not contain",
                        operation, name
                    ));
                }
            }
            for name in &template {
                if !declared.contains(name) {
                    mismatches.push(format!(
                        "{}: template contains `{{{}}}` with no declared path parameter",
                        operation, name
                    ));
                }
            }
        }
    }
    mismatches.sort();

    assert!(
        mismatches.is_empty(),
        "path parameters and path templates disagree: {:#?}",
        mismatches
    );
}

/// The filter grammar is the primary query interface: `POST /series/list` and
/// `POST /books/list` route all filtering through `condition`. Both fields once
/// carried `#[schema(value_type = Option<Object>)]`, which erased a fully
/// modelled recursive grammar to a bare untyped object, so a generated client got
/// a freeform dictionary for the one field that carries the actual query and the
/// grammar was discoverable only by reading the Rust source.
///
/// The condition schemas are recursive, which is legal OpenAPI and is what the
/// override was presumably working around. Asserting the `$ref` keeps that
/// workaround from being reintroduced quietly.
#[test]
fn filter_conditions_reference_their_grammar() {
    let spec = spec();

    for (request, condition) in [
        ("SeriesListRequest", "SeriesCondition"),
        ("BookListRequest", "BookCondition"),
    ] {
        let property = &spec["components"]["schemas"][request]["properties"]["condition"];
        assert_eq!(
            property.get("$ref").and_then(Value::as_str),
            Some(format!("#/components/schemas/{}", condition).as_str()),
            "{}.condition should reference {} rather than erasing the grammar, got {}",
            request,
            condition,
            property
        );
        assert!(
            spec["components"]["schemas"]
                .get(condition)
                .is_some_and(|schema| schema.get("oneOf").is_some()),
            "{} should be modelled as a oneOf over its combinators and predicates",
            condition
        );
    }
}
