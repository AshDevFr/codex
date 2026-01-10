use anyhow::Result;
use std::path::PathBuf;
use utoipa::OpenApi;

use crate::api::docs::ApiDoc;

/// Export OpenAPI specification to a file
///
/// This command generates the OpenAPI 3.0 specification from the
/// utoipa annotations in the codebase and writes it to the specified file.
///
/// # Arguments
/// * `output` - Path to write the OpenAPI spec (default: openapi.json)
/// * `format` - Output format: json or yaml (default: json)
///
/// # Example
/// ```bash
/// codex openapi --output web/openapi.json
/// codex openapi --output openapi.yaml --format yaml
/// ```
pub fn openapi_command(output: PathBuf, format: OpenApiFormat) -> Result<()> {
    let spec = ApiDoc::openapi();

    let content = match format {
        OpenApiFormat::Json => spec.to_pretty_json()?,
        OpenApiFormat::Yaml => spec.to_yaml()?,
    };

    std::fs::write(&output, content)?;

    println!("OpenAPI specification written to: {}", output.display());
    println!("Format: {:?}", format);

    // Print some stats
    println!("Endpoints: {}", spec.paths.paths.len());
    if let Some(components) = &spec.components {
        println!("Schemas: {}", components.schemas.len());
    }

    Ok(())
}

/// Output format for OpenAPI specification
#[derive(Debug, Clone, Copy, Default, clap::ValueEnum)]
pub enum OpenApiFormat {
    /// JSON format (default)
    #[default]
    Json,
    /// YAML format
    Yaml,
}
