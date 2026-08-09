use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

const OVERRIDE_FILENAME: &str = "AGENTS.override.md";
const DEFAULT_FILENAME: &str = "AGENTS.md";
const DEFAULT_MAX_BYTES: u64 = 32 * 1024;
const DEFAULT_PROJECT_ROOT_MARKER: &str = ".git";
const SEPARATOR_BYTES: u64 = 2;

#[derive(Debug, Default, Deserialize)]
struct AgentConfig {
    project_doc_max_bytes: Option<u64>,
    project_doc_fallback_filenames: Option<Vec<String>>,
    project_root_markers: Option<Vec<String>>,
    #[serde(default)]
    projects: BTreeMap<String, ProjectTrustConfig>,
}

#[derive(Debug, Deserialize)]
struct ProjectTrustConfig {
    #[serde(default)]
    trust_level: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentDiscoveryOptions {
    pub(crate) codex_home: PathBuf,
    pub(crate) project_root: Option<PathBuf>,
    pub(crate) current_dir: PathBuf,
    pub(crate) fallback_filenames: Vec<String>,
    pub(crate) max_bytes: u64,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum InstructionScope {
    Global,
    Project,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ProjectTrust {
    Trusted,
    Untrusted,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstructionSource {
    pub(crate) path: PathBuf,
    pub(crate) scope: InstructionScope,
    pub(crate) size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedAgentDiscovery {
    pub(crate) options: AgentDiscoveryOptions,
    pub(crate) project_root_markers: Vec<String>,
    pub(crate) config_path: PathBuf,
    pub(crate) project_trust: Option<ProjectTrust>,
    pub(crate) project_config_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentInspection {
    pub(crate) sources: Vec<InstructionSource>,
    pub(crate) discovery: ResolvedAgentDiscovery,
}

pub(crate) fn resolve_agent_discovery() -> io::Result<ResolvedAgentDiscovery> {
    let current_dir = env::current_dir()?;
    let codex_home = resolve_codex_home(
        env::var_os("CODEX_HOME"),
        env::var_os("HOME"),
        env::var_os("USERPROFILE"),
    )?;

    resolve_agent_discovery_at(codex_home, current_dir)
}

fn resolve_codex_home(
    codex_home: Option<OsString>,
    home: Option<OsString>,
    userprofile: Option<OsString>,
) -> io::Result<PathBuf> {
    codex_home
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            home.filter(|value| !value.is_empty())
                .or_else(|| userprofile.filter(|value| !value.is_empty()))
                .map(|home| PathBuf::from(home).join(".codex"))
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "could not resolve CODEX_HOME from CODEX_HOME, HOME, or USERPROFILE",
            )
        })
}

pub(crate) fn inspect_current_agents() -> io::Result<AgentInspection> {
    let discovery = resolve_agent_discovery()?;
    let sources = discover_agents(&discovery.options)?;

    Ok(AgentInspection { sources, discovery })
}

fn resolve_agent_discovery_at(
    codex_home: PathBuf,
    current_dir: PathBuf,
) -> io::Result<ResolvedAgentDiscovery> {
    let config_path = codex_home.join("config.toml");
    let config = read_agent_config(&config_path)?.unwrap_or_default();

    let project_root_markers = config
        .project_root_markers
        .unwrap_or_else(|| vec![DEFAULT_PROJECT_ROOT_MARKER.to_owned()]);
    let project_root = find_project_root(&current_dir, &project_root_markers)?;
    let project_trust = project_root
        .as_deref()
        .and_then(|root| find_project_trust(root, &config.projects));
    let mut fallback_filenames = config.project_doc_fallback_filenames.unwrap_or_default();
    let mut max_bytes = config.project_doc_max_bytes.unwrap_or(DEFAULT_MAX_BYTES);
    let mut project_config_paths = Vec::new();

    if project_trust == Some(ProjectTrust::Trusted) {
        for directory in project_directories(project_root.as_deref(), &current_dir)? {
            let project_config_path = directory.join(".codex/config.toml");
            let Some(project_config) = read_agent_config(&project_config_path)? else {
                continue;
            };

            if let Some(value) = project_config.project_doc_fallback_filenames {
                fallback_filenames = value;
            }
            if let Some(value) = project_config.project_doc_max_bytes {
                max_bytes = value;
            }
            project_config_paths.push(project_config_path);
        }
    }

    Ok(ResolvedAgentDiscovery {
        options: AgentDiscoveryOptions {
            codex_home,
            project_root,
            current_dir,
            fallback_filenames,
            max_bytes,
        },
        project_root_markers,
        config_path,
        project_trust,
        project_config_paths,
    })
}

fn read_agent_config(path: &Path) -> io::Result<Option<AgentConfig>> {
    match fs::read_to_string(path) {
        Ok(contents) => toml::from_str::<AgentConfig>(&contents)
            .map(Some)
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("failed to parse {}: {error}", path.display()),
                )
            }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn find_project_trust(
    project_root: &Path,
    projects: &BTreeMap<String, ProjectTrustConfig>,
) -> Option<ProjectTrust> {
    let configured = projects
        .get(&project_root.to_string_lossy().into_owned())
        .and_then(parse_project_trust);

    if configured.is_some() {
        return configured;
    }

    fs::canonicalize(project_root)
        .ok()
        .and_then(|path| projects.get(&path.to_string_lossy().into_owned()))
        .and_then(parse_project_trust)
}

fn parse_project_trust(project: &ProjectTrustConfig) -> Option<ProjectTrust> {
    match project.trust_level.as_deref() {
        Some("trusted") => Some(ProjectTrust::Trusted),
        Some("untrusted") => Some(ProjectTrust::Untrusted),
        _ => None,
    }
}

fn find_project_root(current_dir: &Path, markers: &[String]) -> io::Result<Option<PathBuf>> {
    if markers.is_empty() {
        return Ok(Some(current_dir.to_path_buf()));
    }

    for directory in current_dir.ancestors() {
        for marker in markers {
            if directory.join(marker).try_exists()? {
                return Ok(Some(directory.to_path_buf()));
            }
        }
    }

    Ok(None)
}

pub(crate) fn discover_agents(
    options: &AgentDiscoveryOptions,
) -> io::Result<Vec<InstructionSource>> {
    let mut sources = Vec::new();
    let mut combined_size = 0_u64;

    if let Some((path, size_bytes)) = first_instruction_file(&options.codex_home, &[])?
        && !add_source(
            &mut sources,
            &mut combined_size,
            options.max_bytes,
            path,
            InstructionScope::Global,
            size_bytes,
        )
    {
        return Ok(sources);
    }

    for directory in project_directories(options.project_root.as_deref(), &options.current_dir)? {
        let Some((path, size_bytes)) =
            first_instruction_file(&directory, &options.fallback_filenames)?
        else {
            continue;
        };

        if !add_source(
            &mut sources,
            &mut combined_size,
            options.max_bytes,
            path,
            InstructionScope::Project,
            size_bytes,
        ) {
            break;
        }
    }

    Ok(sources)
}

fn first_instruction_file(
    directory: &Path,
    fallback_filenames: &[String],
) -> io::Result<Option<(PathBuf, u64)>> {
    let filenames = [OVERRIDE_FILENAME, DEFAULT_FILENAME]
        .into_iter()
        .chain(fallback_filenames.iter().map(String::as_str));

    for filename in filenames {
        let path = directory.join(filename);
        match fs::metadata(&path) {
            Ok(metadata) if metadata.is_file() && metadata.len() > 0 => {
                return Ok(Some((path, metadata.len())));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }

    Ok(None)
}

fn project_directories(
    project_root: Option<&Path>,
    current_dir: &Path,
) -> io::Result<Vec<PathBuf>> {
    let Some(project_root) = project_root else {
        return Ok(vec![current_dir.to_path_buf()]);
    };

    let relative = current_dir.strip_prefix(project_root).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "current directory is outside the project root",
        )
    })?;

    let mut directories = vec![project_root.to_path_buf()];
    let mut directory = project_root.to_path_buf();

    for component in relative.components() {
        directory.push(component.as_os_str());
        directories.push(directory.clone());
    }

    Ok(directories)
}

fn add_source(
    sources: &mut Vec<InstructionSource>,
    combined_size: &mut u64,
    max_bytes: u64,
    path: PathBuf,
    scope: InstructionScope,
    size_bytes: u64,
) -> bool {
    let separator_size = if sources.is_empty() {
        0
    } else {
        SEPARATOR_BYTES
    };
    let next_size = combined_size
        .saturating_add(separator_size)
        .saturating_add(size_bytes);

    if next_size > max_bytes {
        return false;
    }

    sources.push(InstructionSource {
        path,
        scope,
        size_bytes,
    });
    *combined_size = next_size;
    true
}

#[cfg(test)]
mod tests {
    use std::{
        process,
        sync::atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    static NEXT_TEST_DIR: AtomicUsize = AtomicUsize::new(0);

    struct TestDirectory {
        path: PathBuf,
    }

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_TEST_DIR.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("goalrail-agents-test-{}-{sequence}", process::id()));
            fs::create_dir_all(&path).expect("test directory should be created");
            Self { path }
        }

        fn directory(&self, relative: &str) -> PathBuf {
            let path = self.path.join(relative);
            fs::create_dir_all(&path).expect("fixture directory should be created");
            path
        }

        fn file(&self, relative: &str, contents: &str) -> PathBuf {
            let path = self.path.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("fixture parent should be created");
            }
            fs::write(&path, contents).expect("fixture file should be written");
            path
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn resolves_codex_home_from_non_empty_environment_values() {
        assert_eq!(
            resolve_codex_home(
                Some(OsString::from("/explicit")),
                Some(OsString::from("/home")),
                Some(OsString::from("/profile")),
            )
            .expect("explicit CODEX_HOME should resolve"),
            PathBuf::from("/explicit")
        );
        assert_eq!(
            resolve_codex_home(
                Some(OsString::new()),
                Some(OsString::from("/home")),
                Some(OsString::from("/profile")),
            )
            .expect("HOME should be the first fallback"),
            PathBuf::from("/home/.codex")
        );
        assert_eq!(
            resolve_codex_home(
                Some(OsString::new()),
                Some(OsString::new()),
                Some(OsString::from("/profile")),
            )
            .expect("USERPROFILE should be the final fallback"),
            PathBuf::from("/profile/.codex")
        );

        let error = resolve_codex_home(
            Some(OsString::new()),
            Some(OsString::new()),
            Some(OsString::new()),
        )
        .expect_err("empty environment values must not resolve");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn resolves_discovery_from_user_config() {
        let tree = TestDirectory::new();
        let codex_home = tree.directory("codex-home");
        let project_root = tree.directory("repo");
        let current_dir = tree.directory("repo/crates/app");

        tree.file(
            "codex-home/config.toml",
            r#"
project_doc_max_bytes = 4096
project_doc_fallback_filenames = ["TEAM_GUIDE.md"]
project_root_markers = [".hg"]
"#,
        );
        tree.file("repo/.hg", "marker");

        let resolved = resolve_agent_discovery_at(codex_home.clone(), current_dir.clone())
            .expect("configuration should resolve");

        assert_eq!(resolved.config_path, codex_home.join("config.toml"));
        assert_eq!(resolved.project_root_markers, vec![".hg"]);
        assert_eq!(resolved.options.codex_home, codex_home);
        assert_eq!(resolved.options.project_root, Some(project_root));
        assert_eq!(resolved.options.current_dir, current_dir);
        assert_eq!(resolved.options.fallback_filenames, vec!["TEAM_GUIDE.md"]);
        assert_eq!(resolved.options.max_bytes, 4096);
        assert_eq!(resolved.project_trust, None);
        assert!(resolved.project_config_paths.is_empty());
    }

    #[test]
    fn resolves_defaults_when_user_config_is_missing() {
        let tree = TestDirectory::new();
        let codex_home = tree.directory("codex-home");
        let project_root = tree.directory("repo");
        let current_dir = tree.directory("repo/crates/app");

        tree.directory("repo/.git");

        let resolved =
            resolve_agent_discovery_at(codex_home, current_dir).expect("defaults should resolve");

        assert_eq!(resolved.project_root_markers, vec![".git"]);
        assert_eq!(resolved.options.project_root, Some(project_root));
        assert!(resolved.options.fallback_filenames.is_empty());
        assert_eq!(resolved.options.max_bytes, 32 * 1024);
        assert_eq!(resolved.project_trust, None);
        assert!(resolved.project_config_paths.is_empty());
    }

    #[test]
    fn overlays_trusted_project_config_from_root_to_current_directory() {
        let tree = TestDirectory::new();
        let codex_home = tree.directory("codex-home");
        let project_root = tree.directory("repo");
        let current_dir = tree.directory("repo/crates/app");

        tree.directory("repo/.git");
        tree.file(
            "codex-home/config.toml",
            &format!(
                r#"
project_doc_max_bytes = 8192
project_doc_fallback_filenames = ["USER.md"]

[projects."{}"]
trust_level = "trusted"
"#,
                project_root.display()
            ),
        );
        let root_config = tree.file(
            "repo/.codex/config.toml",
            r#"
project_doc_max_bytes = 4096
project_doc_fallback_filenames = ["ROOT.md"]
"#,
        );
        let crates_config = tree.file(
            "repo/crates/.codex/config.toml",
            r#"project_doc_fallback_filenames = ["CRATE.md"]"#,
        );
        let current_config = tree.file(
            "repo/crates/app/.codex/config.toml",
            "project_doc_max_bytes = 1024",
        );

        let resolved = resolve_agent_discovery_at(codex_home, current_dir)
            .expect("trusted project layers should resolve");

        assert_eq!(resolved.project_trust, Some(ProjectTrust::Trusted));
        assert_eq!(resolved.options.max_bytes, 1024);
        assert_eq!(resolved.options.fallback_filenames, vec!["CRATE.md"]);
        assert_eq!(
            resolved.project_config_paths,
            vec![root_config, crates_config, current_config]
        );
    }

    #[test]
    fn ignores_project_config_when_project_is_untrusted() {
        let tree = TestDirectory::new();
        let codex_home = tree.directory("codex-home");
        let project_root = tree.directory("repo");
        let current_dir = tree.directory("repo/crates/app");

        tree.directory("repo/.git");
        tree.file(
            "codex-home/config.toml",
            &format!(
                r#"
project_doc_max_bytes = 8192
project_doc_fallback_filenames = ["USER.md"]

[projects."{}"]
trust_level = "untrusted"
"#,
                project_root.display()
            ),
        );
        tree.file(
            "repo/.codex/config.toml",
            r#"
project_doc_max_bytes = 1024
project_doc_fallback_filenames = ["PROJECT.md"]
"#,
        );

        let resolved = resolve_agent_discovery_at(codex_home, current_dir)
            .expect("untrusted project should use only user settings");

        assert_eq!(resolved.project_trust, Some(ProjectTrust::Untrusted));
        assert_eq!(resolved.options.max_bytes, 8192);
        assert_eq!(resolved.options.fallback_filenames, vec!["USER.md"]);
        assert!(resolved.project_config_paths.is_empty());
    }

    #[test]
    fn tolerates_unknown_or_missing_project_trust_values() {
        let tree = TestDirectory::new();
        let codex_home = tree.directory("codex-home");
        let project_root = tree.directory("repo");
        let current_dir = tree.directory("repo/crates/app");

        tree.directory("repo/.git");
        tree.file(
            "codex-home/config.toml",
            &format!(
                r#"
project_doc_max_bytes = 8192
project_doc_fallback_filenames = ["USER.md"]

[projects."{}"]
trust_level = "future-value"

[projects."/unrelated"]
note = "no trust level"
"#,
                project_root.display()
            ),
        );
        tree.file(
            "repo/.codex/config.toml",
            r#"
project_doc_max_bytes = 1024
project_doc_fallback_filenames = ["PROJECT.md"]
"#,
        );

        let resolved = resolve_agent_discovery_at(codex_home, current_dir)
            .expect("unknown trust values should not break discovery");

        assert_eq!(resolved.project_trust, None);
        assert_eq!(resolved.options.max_bytes, 8192);
        assert_eq!(resolved.options.fallback_filenames, vec!["USER.md"]);
        assert!(resolved.project_config_paths.is_empty());
    }

    #[test]
    fn discovers_global_and_project_sources_in_precedence_order() {
        let tree = TestDirectory::new();
        let codex_home = tree.directory("codex-home");
        let project_root = tree.directory("repo");
        let current_dir = tree.directory("repo/crates/app");

        tree.file("codex-home/AGENTS.md", "global default");
        let global_override = tree.file("codex-home/AGENTS.override.md", "global override");
        let root_agents = tree.file("repo/AGENTS.md", "root");
        tree.file("repo/crates/AGENTS.md", "crates default");
        let crates_override = tree.file("repo/crates/AGENTS.override.md", "crates override");
        let current_agents = tree.file("repo/crates/app/AGENTS.md", "current");
        tree.file("repo/crates/app/deeper/AGENTS.md", "too deep");

        let sources = discover_agents(&AgentDiscoveryOptions {
            codex_home,
            project_root: Some(project_root),
            current_dir,
            fallback_filenames: Vec::new(),
            max_bytes: 32 * 1024,
        })
        .expect("discovery should succeed");

        let paths: Vec<_> = sources.iter().map(|source| &source.path).collect();
        assert_eq!(
            paths,
            vec![
                &global_override,
                &root_agents,
                &crates_override,
                &current_agents
            ]
        );
        assert_eq!(sources[0].scope, InstructionScope::Global);
        assert!(
            sources[1..]
                .iter()
                .all(|source| source.scope == InstructionScope::Project)
        );
    }

    #[test]
    fn skips_empty_files_and_uses_configured_fallback() {
        let tree = TestDirectory::new();
        let codex_home = tree.directory("codex-home");
        let project_root = tree.directory("repo");

        tree.file("codex-home/AGENTS.md", "");
        tree.file("repo/AGENTS.override.md", "");
        tree.file("repo/AGENTS.md", "");
        let fallback = tree.file("repo/TEAM_GUIDE.md", "fallback");

        let sources = discover_agents(&AgentDiscoveryOptions {
            codex_home,
            project_root: Some(project_root.clone()),
            current_dir: project_root,
            fallback_filenames: vec!["TEAM_GUIDE.md".to_owned()],
            max_bytes: 32 * 1024,
        })
        .expect("discovery should succeed");

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].path, fallback);
    }

    #[test]
    fn checks_only_current_directory_without_project_root() {
        let tree = TestDirectory::new();
        let codex_home = tree.directory("codex-home");
        let current_dir = tree.directory("parent/current");

        tree.file("parent/AGENTS.md", "parent");
        let current_agents = tree.file("parent/current/AGENTS.md", "current");

        let sources = discover_agents(&AgentDiscoveryOptions {
            codex_home,
            project_root: None,
            current_dir,
            fallback_filenames: Vec::new(),
            max_bytes: 32 * 1024,
        })
        .expect("discovery should succeed");

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].path, current_agents);
    }

    #[test]
    fn stops_before_exceeding_combined_size_limit() {
        let tree = TestDirectory::new();
        let codex_home = tree.directory("codex-home");
        let project_root = tree.directory("repo");

        let global = tree.file("codex-home/AGENTS.md", "1234");
        tree.file("repo/AGENTS.md", "5678");

        let sources = discover_agents(&AgentDiscoveryOptions {
            codex_home,
            project_root: Some(project_root.clone()),
            current_dir: project_root,
            fallback_filenames: Vec::new(),
            max_bytes: 9,
        })
        .expect("discovery should succeed");

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].path, global);
        assert_eq!(sources[0].size_bytes, 4);
    }

    #[test]
    fn accepts_a_source_at_the_exact_combined_size_limit() {
        let mut sources = Vec::new();
        let mut combined_size = 0;

        assert!(add_source(
            &mut sources,
            &mut combined_size,
            4,
            PathBuf::from("AGENTS.md"),
            InstructionScope::Project,
            4,
        ));
        assert_eq!(combined_size, 4);
        assert_eq!(sources.len(), 1);
    }

    #[test]
    fn propagates_non_not_found_config_read_errors() {
        let tree = TestDirectory::new();
        let directory = tree.directory("config.toml");

        let error = read_agent_config(&directory).expect_err("a directory is not a config file");

        assert_ne!(error.kind(), io::ErrorKind::NotFound);
    }

    #[cfg(unix)]
    #[test]
    fn propagates_non_not_found_instruction_metadata_errors() {
        use std::os::unix::fs::symlink;

        let tree = TestDirectory::new();
        let directory = tree.directory("repo");
        symlink("AGENTS.override.md", directory.join("AGENTS.override.md"))
            .expect("self-referential fixture symlink should be created");

        let error = first_instruction_file(&directory, &[])
            .expect_err("a symlink loop must be reported instead of ignored");

        assert_ne!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn rejects_current_directory_outside_project_root() {
        let tree = TestDirectory::new();
        let codex_home = tree.directory("codex-home");
        let project_root = tree.directory("repo");
        let current_dir = tree.directory("other");

        let error = discover_agents(&AgentDiscoveryOptions {
            codex_home,
            project_root: Some(project_root),
            current_dir,
            fallback_filenames: Vec::new(),
            max_bytes: 32 * 1024,
        })
        .expect_err("discovery should reject unrelated paths");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
