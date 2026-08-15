use std::process::ExitCode;

use clap::{Parser, Subcommand};
use gr_inspect_claude::{
    InspectionOutcome as ClaudeInspectionOutcome, inspect_claude as run_claude_inspection,
};
use gr_inspect_codex::{
    InspectionOutcome, SkillsInspectionOutcome, inspect_codex as run_codex_inspection,
    inspect_codex_plugins as run_codex_plugins_inspection,
    inspect_codex_skill_actions as run_codex_skill_actions_inspection,
    inspect_codex_skills as run_codex_skills_inspection,
    inspect_codex_updates as run_codex_updates_inspection,
};
use gr_inspect_core::Verdict;

#[derive(Debug, Parser)]
#[command(name = "gr", version, about = "Goalrail command-line interface")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Inspect {
        #[command(subcommand)]
        target: InspectTarget,
    },
}

#[derive(Debug, Subcommand)]
enum InspectTarget {
    Codex {
        #[arg(long)]
        json: bool,
        #[command(subcommand)]
        section: Option<CodexSection>,
    },
    Claude {
        #[arg(long)]
        json: bool,
    },
}

#[derive(Debug, Subcommand)]
enum CodexSection {
    Skills {
        #[arg(long)]
        json: bool,
        #[arg(long)]
        actionable: bool,
    },
    Plugins {
        #[arg(long)]
        json: bool,
    },
    Updates {
        #[arg(long)]
        json: bool,
    },
}

fn main() -> ExitCode {
    match Cli::parse().command {
        Command::Inspect {
            target:
                InspectTarget::Codex {
                    json,
                    section: None,
                },
        } => inspect_codex(json),
        Command::Inspect {
            target:
                InspectTarget::Codex {
                    json: outer_json,
                    section:
                        Some(CodexSection::Skills {
                            json: inner_json,
                            actionable,
                        }),
                },
        } => inspect_codex_skills(outer_json || inner_json, actionable),
        Command::Inspect {
            target:
                InspectTarget::Codex {
                    json: outer_json,
                    section: Some(CodexSection::Plugins { json: inner_json }),
                },
        } => inspect_codex_plugins(outer_json || inner_json),
        Command::Inspect {
            target:
                InspectTarget::Codex {
                    json: outer_json,
                    section: Some(CodexSection::Updates { json: inner_json }),
                },
        } => inspect_codex_updates(outer_json || inner_json),
        Command::Inspect {
            target: InspectTarget::Claude { json },
        } => inspect_claude(json),
    }
}

fn inspect_claude(json: bool) -> ExitCode {
    let outcome = run_claude_inspection();
    let verdict = render_claude_outcome(json, &outcome);

    ExitCode::from(verdict.exit_code())
}

fn render_claude_outcome(json: bool, outcome: &ClaudeInspectionOutcome) -> Verdict {
    if json {
        match outcome.to_pretty_json() {
            Ok(output) => println!("{output}"),
            Err(error) => {
                eprintln!("gr inspect claude: failed to serialize JSON report: {error}");
                return Verdict::Incomplete;
            }
        }
    } else {
        let output = outcome.to_human();
        if outcome.is_failure() {
            eprintln!("gr inspect claude: {output}");
        } else {
            print!("{output}");
        }
    }

    outcome.verdict()
}

fn inspect_codex_plugins(json: bool) -> ExitCode {
    let outcome = run_codex_plugins_inspection();
    let verdict = render_outcome("gr inspect codex plugins", json, &outcome);

    ExitCode::from(verdict.exit_code())
}

fn inspect_codex_updates(json: bool) -> ExitCode {
    let outcome = run_codex_updates_inspection();
    let verdict = render_outcome("gr inspect codex updates", json, &outcome);

    ExitCode::from(verdict.exit_code())
}

fn inspect_codex_skills(json: bool, actionable: bool) -> ExitCode {
    let outcome = if actionable {
        run_codex_skill_actions_inspection()
    } else {
        run_codex_skills_inspection()
    };
    let verdict = render_skills_outcome(json, &outcome);

    ExitCode::from(verdict.exit_code())
}

fn inspect_codex(json: bool) -> ExitCode {
    let outcome = run_codex_inspection();
    let verdict = render_outcome("gr inspect codex", json, &outcome);

    ExitCode::from(verdict.exit_code())
}

fn render_skills_outcome(json: bool, outcome: &SkillsInspectionOutcome) -> Verdict {
    if json {
        match outcome.to_pretty_json() {
            Ok(output) => println!("{output}"),
            Err(error) => {
                eprintln!("gr inspect codex skills: failed to serialize JSON report: {error}");
                return Verdict::Incomplete;
            }
        }
    } else {
        let output = outcome.to_human();
        if outcome.is_failure() {
            eprintln!("gr inspect codex skills: {output}");
        } else {
            print!("{output}");
        }
    }

    outcome.verdict()
}

fn render_outcome(command: &str, json: bool, outcome: &InspectionOutcome) -> Verdict {
    if json {
        match outcome.to_pretty_json() {
            Ok(output) => println!("{output}"),
            Err(error) => {
                eprintln!("{command}: failed to serialize JSON report: {error}");
                return Verdict::Incomplete;
            }
        }
    } else {
        let output = outcome.to_human();
        if outcome.is_failure() {
            eprintln!("{command}: {output}");
        } else {
            print!("{output}");
        }
    }

    outcome.verdict()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_codex_inspection_with_json_output() {
        let cli = Cli::try_parse_from(["gr", "inspect", "codex", "--json"])
            .expect("command should parse");

        assert!(matches!(
            cli.command,
            Command::Inspect {
                target: InspectTarget::Codex {
                    json: true,
                    section: None
                }
            }
        ));
    }

    #[test]
    fn parses_skills_drilldown_with_json_output() {
        let cli = Cli::try_parse_from(["gr", "inspect", "codex", "skills", "--json"])
            .expect("command should parse");

        assert!(matches!(
            cli.command,
            Command::Inspect {
                target: InspectTarget::Codex {
                    section: Some(CodexSection::Skills {
                        json: true,
                        actionable: false,
                    }),
                    ..
                }
            }
        ));
    }

    #[test]
    fn parses_skills_drilldown_with_outer_json_output() {
        let cli = Cli::try_parse_from(["gr", "inspect", "codex", "--json", "skills"])
            .expect("command should parse");

        assert!(matches!(
            cli.command,
            Command::Inspect {
                target: InspectTarget::Codex {
                    json: true,
                    section: Some(CodexSection::Skills {
                        json: false,
                        actionable: false,
                    }),
                }
            }
        ));
    }

    #[test]
    fn parses_actionable_skills_drilldown() {
        let cli =
            Cli::try_parse_from(["gr", "inspect", "codex", "skills", "--actionable", "--json"])
                .expect("command should parse");

        assert!(matches!(
            cli.command,
            Command::Inspect {
                target: InspectTarget::Codex {
                    section: Some(CodexSection::Skills {
                        json: true,
                        actionable: true,
                    }),
                    ..
                }
            }
        ));
    }

    #[test]
    fn parses_plugins_drilldown_with_inner_json_output() {
        let cli = Cli::try_parse_from(["gr", "inspect", "codex", "plugins", "--json"])
            .expect("command should parse");

        assert!(matches!(
            cli.command,
            Command::Inspect {
                target: InspectTarget::Codex {
                    json: false,
                    section: Some(CodexSection::Plugins { json: true }),
                }
            }
        ));
    }

    #[test]
    fn parses_claude_inspection_with_and_without_json_output() {
        let cli = Cli::try_parse_from(["gr", "inspect", "claude", "--json"])
            .expect("command should parse");

        assert!(matches!(
            cli.command,
            Command::Inspect {
                target: InspectTarget::Claude { json: true }
            }
        ));

        let cli = Cli::try_parse_from(["gr", "inspect", "claude"]).expect("command should parse");

        assert!(matches!(
            cli.command,
            Command::Inspect {
                target: InspectTarget::Claude { json: false }
            }
        ));
    }

    #[test]
    fn parses_plugins_drilldown_with_outer_json_output() {
        let cli = Cli::try_parse_from(["gr", "inspect", "codex", "--json", "plugins"])
            .expect("command should parse");

        assert!(matches!(
            cli.command,
            Command::Inspect {
                target: InspectTarget::Codex {
                    json: true,
                    section: Some(CodexSection::Plugins { json: false }),
                }
            }
        ));
    }

    #[test]
    fn parses_updates_drilldown_with_inner_and_outer_json_output() {
        let inner = Cli::try_parse_from(["gr", "inspect", "codex", "updates", "--json"])
            .expect("command should parse");
        assert!(matches!(
            inner.command,
            Command::Inspect {
                target: InspectTarget::Codex {
                    json: false,
                    section: Some(CodexSection::Updates { json: true }),
                }
            }
        ));

        let outer = Cli::try_parse_from(["gr", "inspect", "codex", "--json", "updates"])
            .expect("command should parse");
        assert!(matches!(
            outer.command,
            Command::Inspect {
                target: InspectTarget::Codex {
                    json: true,
                    section: Some(CodexSection::Updates { json: false }),
                }
            }
        ));
    }
}
