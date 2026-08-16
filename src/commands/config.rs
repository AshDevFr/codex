//! `codex config` — inspect configuration without starting the server.
//!
//! `check` resolves the configuration the way `serve` does, reports every
//! `CODEX_` variable that is not read, and prints the result with secrets
//! removed. Unlike `serve` it lists every problem rather than stopping at the
//! first, which is the whole point of running it before a deployment.
//!
//! It opens no database connection and writes nothing, so it is safe to run as
//! a Kubernetes initContainer ahead of the app container, or as a one-off Job
//! before scheduling an upgrade.

use anyhow::Result;
use clap::Subcommand;
use std::path::{Path, PathBuf};

use codex_cli_common::resolve_config;
use codex_config::{Config, Finding, audit_env_with_config, redacted_yaml, write_starter_config};

/// Configuration subcommands
#[derive(Subcommand, Debug)]
pub enum ConfigSubcommand {
    /// Validate environment variables and print the resolved configuration
    Check {
        /// Also fail on warnings, not just on settings that are no longer read
        #[arg(long)]
        strict: bool,

        /// Print only findings, omitting the resolved configuration
        #[arg(short, long)]
        quiet: bool,
    },

    /// Write a commented starter configuration file
    Init {
        /// Replace an existing file
        #[arg(long)]
        force: bool,
    },
}

pub fn config_command(config_path: PathBuf, command: ConfigSubcommand) -> Result<()> {
    match command {
        ConfigSubcommand::Check { strict, quiet } => check(&config_path, strict, quiet),
        ConfigSubcommand::Init { force } => {
            write_starter_config(&config_path, force)?;
            println!("Wrote a starter configuration to {}", config_path.display());
            println!("Edit it, then run `codex config check` to validate.");
            Ok(())
        }
    }
}

fn check(config_path: &Path, strict: bool, quiet: bool) -> Result<()> {
    // `resolve` rather than `load`: the point is to list every problem, and
    // `load` refuses to start on the first one.
    //
    // A value of the wrong type fails resolution outright, so there is no
    // config to audit against. Report it and carry on with the name checks
    // rather than returning, because a mistyped value and a misspelled
    // variable are exactly the pair an operator wants to see together.
    let (config, type_error) = match resolve_config(config_path) {
        Ok(config) => (config, None),
        Err(error) => (Config::default(), Some(format!("{error:#}"))),
    };

    let findings = audit_env_with_config(&config);
    let report = build_report(
        config_path,
        &config,
        &findings,
        type_error.as_deref(),
        // The resolved config is the defaults when resolution failed, so
        // printing it would be a lie.
        quiet || type_error.is_some(),
    )?;

    print!("{report}");

    // Match what the server will do: it refuses to start on either of these.
    let failed = type_error.is_some()
        || findings.iter().any(Finding::is_fatal)
        || (strict && !findings.is_empty());
    if failed {
        std::process::exit(1);
    }
    Ok(())
}

/// Render the whole report as text.
///
/// Split out from [`check`] so the formatting is testable without a process
/// boundary or a live environment.
fn build_report(
    config_path: &Path,
    config: &Config,
    findings: &[Finding],
    type_error: Option<&str>,
    quiet: bool,
) -> Result<String> {
    use std::fmt::Write as _;
    let mut out = String::new();

    writeln!(out, "Codex configuration check")?;
    writeln!(
        out,
        "  config file: {} ({})",
        config_path.display(),
        if config_path.exists() {
            "found"
        } else {
            "not found, using defaults"
        }
    )?;

    let legacy: Vec<&Finding> = findings
        .iter()
        .filter(|f| matches!(f, Finding::Legacy { .. }))
        .collect();
    let removed: Vec<&Finding> = findings
        .iter()
        .filter(|f| matches!(f, Finding::Removed { .. }))
        .collect();
    let unknown: Vec<&Finding> = findings
        .iter()
        .filter(|f| matches!(f, Finding::Unknown { .. }))
        .collect();

    if let Some(error) = type_error {
        writeln!(out, "\nERROR: a value could not be parsed:\n")?;
        for line in error.lines() {
            writeln!(out, "  {line}")?;
        }
        writeln!(out, "\n  Values are typed. Booleans are `true`/`false`,")?;
        writeln!(
            out,
            "  lists are `[a, b]`, maps are `{{key=value, key=value}}`."
        )?;
        writeln!(
            out,
            "  Parsing stops at the first bad value, so fix this one and run again."
        )?;
    }

    if findings.is_empty() && type_error.is_none() {
        writeln!(out, "\n  No environment variable problems found.")?;
    }

    if !legacy.is_empty() {
        writeln!(
            out,
            "\nERROR: environment variables that are no longer read ({}):",
            legacy.len()
        )?;
        let width = legacy.iter().map(|f| f.var().len()).max().unwrap_or(0);
        for finding in &legacy {
            if let Finding::Legacy {
                var, replacement, ..
            } = finding
            {
                writeln!(out, "  {var:<width$}  ->  {replacement}")?;
            }
        }
        writeln!(
            out,
            "\n  Nesting levels are separated by `__` since Codex 2.0."
        )?;
        writeln!(out, "  A single `_` still separates words inside one key.")?;
    }

    if !removed.is_empty() {
        writeln!(
            out,
            "\nERROR: environment variables that were replaced ({}):",
            removed.len()
        )?;
        for finding in &removed {
            if let Finding::Removed {
                var,
                replacement,
                note,
            } = finding
            {
                writeln!(out, "  {var}\n      use {replacement}  ({note})")?;
            }
        }
    }

    if !unknown.is_empty() {
        writeln!(
            out,
            "\nWarning: unrecognized environment variables ({}):",
            unknown.len()
        )?;
        for finding in &unknown {
            if let Finding::Unknown { var, nearest } = finding {
                match nearest {
                    Some(path) => writeln!(
                        out,
                        "  {var}\n      not a Codex setting; did you mean {}?",
                        codex_config::v2_name_for(path)
                    )?,
                    None => writeln!(out, "  {var}\n      not a Codex setting; ignored")?,
                }
            }
        }
    }

    if !quiet {
        writeln!(out, "\nResolved configuration (secrets redacted):\n")?;
        for line in redacted_yaml(config)?.lines() {
            writeln!(out, "  {line}")?;
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy(var: &str, replacement: &str, path: &str) -> Finding {
        Finding::Legacy {
            var: var.to_string(),
            replacement: replacement.to_string(),
            path: path.to_string(),
        }
    }

    fn report(findings: &[Finding], quiet: bool) -> String {
        build_report(
            Path::new("config/codex.yaml"),
            &Config::default(),
            findings,
            None,
            quiet,
        )
        .unwrap()
    }

    #[test]
    fn legacy_names_are_listed_with_their_replacement() {
        let text = report(
            &[
                legacy(
                    "CODEX_TASK_WORKER_COUNT",
                    "CODEX_TASK__WORKER_COUNT",
                    "task.worker_count",
                ),
                legacy(
                    "CODEX_APPLICATION_PORT",
                    "CODEX_APPLICATION__PORT",
                    "application.port",
                ),
            ],
            true,
        );
        assert!(text.contains("no longer read (2)"));
        assert!(text.contains("CODEX_TASK__WORKER_COUNT"));
        assert!(text.contains("CODEX_APPLICATION__PORT"));
        assert!(text.contains("separated by `__`"));
    }

    /// The two inverted replacements are the ones worth reading carefully, so
    /// the note has to reach the report.
    #[test]
    fn replaced_variables_carry_their_note() {
        let text = report(
            &[Finding::Removed {
                var: "CODEX_DISABLE_WORKERS".to_string(),
                replacement: "CODEX_TASK__RUN_IN_PROCESS".to_string(),
                note: "INVERTED: `DISABLE_WORKERS=true` becomes `RUN_IN_PROCESS=false`".to_string(),
            }],
            true,
        );
        assert!(text.contains("were replaced (1)"));
        assert!(text.contains("CODEX_TASK__RUN_IN_PROCESS"));
        assert!(text.contains("INVERTED"));
    }

    /// Unknown names are advisory; the heading must not read as an error.
    #[test]
    fn unknown_variables_are_a_warning_with_a_suggestion() {
        let text = report(
            &[Finding::Unknown {
                var: "CODEX_DATABASE_POSTGRES_USER".to_string(),
                nearest: Some("database.postgres.username".to_string()),
            }],
            true,
        );
        assert!(text.contains("Warning: unrecognized"));
        assert!(text.contains("did you mean CODEX_DATABASE__POSTGRES__USERNAME?"));
    }

    #[test]
    fn unknown_variables_without_a_match_say_so() {
        let text = report(
            &[Finding::Unknown {
                var: "CODEX_SOMETHING_ELSE".to_string(),
                nearest: None,
            }],
            true,
        );
        assert!(text.contains("not a Codex setting; ignored"));
    }

    /// A mistyped value must not hide the misspelled variable next to it.
    #[test]
    fn a_type_error_is_reported_alongside_name_findings() {
        let text = build_report(
            Path::new("config/codex.yaml"),
            &Config::default(),
            &[legacy(
                "CODEX_TASK_WORKER_COUNT",
                "CODEX_TASK__WORKER_COUNT",
                "task.worker_count",
            )],
            Some(
                "invalid type: found string \"x\", expected a boolean for key \"API.CORS_ENABLED\"",
            ),
            true,
        )
        .unwrap();

        assert!(text.contains("a value could not be parsed"));
        assert!(text.contains("API.CORS_ENABLED"));
        assert!(
            text.contains("`[a, b]`"),
            "should show the list syntax: {text}"
        );
        assert!(
            text.contains("CODEX_TASK__WORKER_COUNT"),
            "the name finding must still appear: {text}"
        );
    }

    #[test]
    fn a_clean_environment_reports_nothing_to_fix() {
        assert!(report(&[], true).contains("No environment variable problems found."));
    }

    #[test]
    fn the_resolved_config_is_printed_unless_quiet() {
        assert!(report(&[], false).contains("Resolved configuration"));
        assert!(!report(&[], true).contains("Resolved configuration"));
    }

    #[test]
    fn the_printed_config_hides_secrets() {
        let mut config = Config::default();
        config.auth.jwt_secret = "a-real-signing-secret".to_string();
        let text = build_report(Path::new("config/codex.yaml"), &config, &[], None, false).unwrap();

        assert!(!text.contains("a-real-signing-secret"));
        assert!(text.contains(codex_config::REDACTED));
    }

    #[test]
    fn a_missing_config_file_is_reported_not_created() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.yaml");

        let text = build_report(&missing, &Config::default(), &[], None, true).unwrap();

        assert!(text.contains("not found, using defaults"));
        assert!(!missing.exists(), "check must not create the config file");
    }
}
