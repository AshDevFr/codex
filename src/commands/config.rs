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
use codex_config::{Config, Finding, audit_env_with_config, redacted_yaml};

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
}

pub fn config_command(config_path: PathBuf, command: ConfigSubcommand) -> Result<()> {
    match command {
        ConfigSubcommand::Check { strict, quiet } => check(&config_path, strict, quiet),
    }
}

fn check(config_path: &Path, strict: bool, quiet: bool) -> Result<()> {
    // `resolve` rather than `load`: the point is to list every problem,
    // and `load` refuses to start on the first one.
    let config = resolve_config(config_path)?;
    let findings = audit_env_with_config(&config);
    let report = build_report(config_path, &config, &findings, quiet)?;

    print!("{report}");

    // Match what the server will do: a fatal finding stops it, so `check`
    // fails too. `--strict` additionally fails on warnings.
    let failed = findings.iter().any(Finding::is_fatal) || (strict && !findings.is_empty());
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

    if findings.is_empty() {
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
        let text = build_report(Path::new("config/codex.yaml"), &config, &[], false).unwrap();

        assert!(!text.contains("a-real-signing-secret"));
        assert!(text.contains(codex_config::REDACTED));
    }

    #[test]
    fn a_missing_config_file_is_reported_not_created() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nope.yaml");

        let text = build_report(&missing, &Config::default(), &[], true).unwrap();

        assert!(text.contains("not found, using defaults"));
        assert!(!missing.exists(), "check must not create the config file");
    }
}
