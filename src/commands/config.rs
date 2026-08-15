//! `codex config` — inspect configuration without starting the server.
//!
//! `check` resolves the configuration exactly as `serve` would, reports every
//! environment variable that this version does not read or that changes name
//! in the next major version, and prints the result with secrets removed.
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
        /// Exit non-zero if anything at all was reported, not just errors
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
    let config = resolve_config(config_path)?;
    let findings = audit_env_with_config(&config);
    let report = build_report(config_path, &config, &findings, quiet)?;

    print!("{report}");

    if strict && !findings.is_empty() {
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

    let renames: Vec<&Finding> = findings
        .iter()
        .filter(|f| matches!(f, Finding::WillRename { .. }))
        .collect();
    let ignored: Vec<&Finding> = findings
        .iter()
        .filter(|f| matches!(f, Finding::NotYetValid { .. }))
        .collect();
    let unknown: Vec<&Finding> = findings
        .iter()
        .filter(|f| matches!(f, Finding::Unknown { .. }))
        .collect();

    if findings.is_empty() {
        writeln!(out, "\n  No environment variable problems found.")?;
    }

    if !renames.is_empty() {
        writeln!(
            out,
            "\nEnvironment variables that change name in Codex 2.0 ({}):",
            renames.len()
        )?;
        let width = renames.iter().map(|f| f.var().len()).max().unwrap_or(0);
        for finding in &renames {
            if let Finding::WillRename { var, v2_name, .. } = finding {
                writeln!(out, "  {var:<width$}  ->  {v2_name}")?;
            }
        }
        writeln!(
            out,
            "\n  These names are correct for this version. Do not rename them until you\n  \
             upgrade to Codex 2.0: this version does not read the new spelling."
        )?;
    }

    if !ignored.is_empty() {
        writeln!(
            out,
            "\nEnvironment variables that are NOT being read right now ({}):",
            ignored.len()
        )?;
        for finding in &ignored {
            if let Finding::NotYetValid { var, v1_name } = finding {
                writeln!(out, "  {var}\n      this version reads {v1_name} instead")?;
            }
        }
    }

    if !unknown.is_empty() {
        writeln!(
            out,
            "\nUnrecognized environment variables ({}):",
            unknown.len()
        )?;
        for finding in &unknown {
            if let Finding::Unknown { var, nearest } = finding {
                match nearest {
                    Some(path) => writeln!(
                        out,
                        "  {var}\n      not a Codex setting; did you mean {}?",
                        codex_config::v1_name_for(path)
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

    fn rename(var: &str, v2_name: &str, path: &str) -> Finding {
        Finding::WillRename {
            var: var.to_string(),
            v2_name: v2_name.to_string(),
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
    fn renames_are_listed_with_their_replacement() {
        let text = report(
            &[
                rename(
                    "CODEX_TASK_WORKER_COUNT",
                    "CODEX_TASK__WORKER_COUNT",
                    "task.worker_count",
                ),
                rename(
                    "CODEX_APPLICATION_PORT",
                    "CODEX_APPLICATION__PORT",
                    "application.port",
                ),
            ],
            true,
        );
        assert!(text.contains("change name in Codex 2.0 (2)"));
        assert!(text.contains("CODEX_TASK_WORKER_COUNT"));
        assert!(text.contains("CODEX_TASK__WORKER_COUNT"));
        assert!(text.contains("CODEX_APPLICATION__PORT"));
    }

    /// Renaming early silently drops the setting, so the report has to say so
    /// rather than just handing over the new name.
    #[test]
    fn the_report_warns_against_renaming_early() {
        let text = report(
            &[rename(
                "CODEX_TASK_WORKER_COUNT",
                "CODEX_TASK__WORKER_COUNT",
                "task.worker_count",
            )],
            true,
        );
        assert!(
            text.contains("Do not rename them until you"),
            "missing the do-not-rename-early warning:\n{text}"
        );
    }

    #[test]
    fn variables_not_being_read_are_called_out() {
        let text = report(
            &[Finding::NotYetValid {
                var: "CODEX_TASK__WORKER_COUNT".to_string(),
                v1_name: "CODEX_TASK_WORKER_COUNT".to_string(),
            }],
            true,
        );
        assert!(text.contains("NOT being read right now (1)"));
        assert!(text.contains("this version reads CODEX_TASK_WORKER_COUNT instead"));
    }

    #[test]
    fn unknown_variables_suggest_a_v1_name() {
        let text = report(
            &[Finding::Unknown {
                var: "CODEX_DATABASE_POSTGRES_USER".to_string(),
                nearest: Some("database.postgres.username".to_string()),
            }],
            true,
        );
        assert!(text.contains("Unrecognized environment variables (1)"));
        assert!(
            text.contains("did you mean CODEX_DATABASE_POSTGRES_USERNAME?"),
            "suggestion should use this version's spelling:\n{text}"
        );
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
        let text = report(&[], true);
        assert!(text.contains("No environment variable problems found."));
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
