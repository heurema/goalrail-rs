use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    fs, io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

const CLAUDE_HOME_DIRECTORY: &str = ".claude";
const GLOBAL_CONFIG_FILENAME: &str = ".claude.json";
const PROJECT_MCP_FILENAME: &str = ".mcp.json";
const PROJECT_ROOT_MARKER: &str = ".git";
const SKILLS_DIRECTORY: &str = "skills";
const SKILL_MANIFEST_FILENAME: &str = "SKILL.md";
const INSTRUCTION_FILENAMES: [&str; 3] = ["CLAUDE.md", ".claude/CLAUDE.md", "CLAUDE.local.md"];

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum InstructionScope {
    Global,
    Project,
}

impl InstructionScope {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Global => "global",
            Self::Project => "project",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InstructionSource {
    pub(crate) path: PathBuf,
    pub(crate) scope: InstructionScope,
    pub(crate) size_bytes: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct McpEvidence {
    pub(crate) user: usize,
    pub(crate) project: usize,
    pub(crate) local: usize,
}

impl McpEvidence {
    pub(crate) const fn configured(self) -> usize {
        self.user + self.project + self.local
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct SkillEvidence {
    pub(crate) personal: usize,
    pub(crate) project: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalPaths {
    pub(crate) claude_home: PathBuf,
    pub(crate) global_config: PathBuf,
    pub(crate) current_dir: PathBuf,
    pub(crate) project_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LocalEvidence {
    pub(crate) paths: LocalPaths,
    pub(crate) mcp: McpEvidence,
    pub(crate) skills: SkillEvidence,
    pub(crate) instruction_sources: Vec<InstructionSource>,
    /// The exact `projects` key that records state for the current directory.
    pub(crate) project_state_key: Option<String>,
}

/// One MCP server entry, accepted only as the documented JSON object. The
/// values themselves are discarded rather than read, because `~/.claude.json`
/// also holds credentials.
type McpServerMap = BTreeMap<String, BTreeMap<String, serde::de::IgnoredAny>>;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GlobalConfig {
    #[serde(default)]
    mcp_servers: McpServerMap,
    #[serde(default)]
    projects: BTreeMap<String, ProjectEntry>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectEntry {
    #[serde(default)]
    mcp_servers: McpServerMap,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectMcpConfig {
    #[serde(default)]
    mcp_servers: McpServerMap,
}

pub(crate) fn inspect_local_evidence() -> io::Result<LocalEvidence> {
    let paths = resolve_local_paths()?;

    inspect_local_evidence_at(paths)
}

fn resolve_local_paths() -> io::Result<LocalPaths> {
    let current_dir = env::current_dir()?;
    let (claude_home, global_config) = resolve_claude_home(
        env::var_os("CLAUDE_CONFIG_DIR"),
        env::var_os("HOME"),
        env::var_os("USERPROFILE"),
    )?;
    let project_root = find_project_root(&current_dir)?;

    Ok(LocalPaths {
        claude_home,
        global_config,
        current_dir,
        project_root,
    })
}

/// Resolve the documented Claude home and its global configuration file.
///
/// `CLAUDE_CONFIG_DIR` relocates every `~/.claude` path, including the global
/// `.claude.json` file that would otherwise be shared between tenants.
fn resolve_claude_home(
    config_dir: Option<OsString>,
    home: Option<OsString>,
    userprofile: Option<OsString>,
) -> io::Result<(PathBuf, PathBuf)> {
    if let Some(config_dir) = config_dir.filter(|value| !value.is_empty()) {
        let claude_home = PathBuf::from(config_dir);
        let global_config = claude_home.join(GLOBAL_CONFIG_FILENAME);
        return Ok((claude_home, global_config));
    }

    home.filter(|value| !value.is_empty())
        .or_else(|| userprofile.filter(|value| !value.is_empty()))
        .map(|home| {
            let home = PathBuf::from(home);
            (
                home.join(CLAUDE_HOME_DIRECTORY),
                home.join(GLOBAL_CONFIG_FILENAME),
            )
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "could not resolve the Claude home from CLAUDE_CONFIG_DIR, HOME, or USERPROFILE",
            )
        })
}

/// The nearest ancestor holding the project root marker. A directory that
/// cannot be examined fails the inspection instead of silently reading as
/// "no marker here".
fn find_project_root(current_dir: &Path) -> io::Result<Option<PathBuf>> {
    for directory in current_dir.ancestors() {
        if directory.join(PROJECT_ROOT_MARKER).try_exists()? {
            return Ok(Some(directory.to_path_buf()));
        }
    }

    Ok(None)
}

fn inspect_local_evidence_at(paths: LocalPaths) -> io::Result<LocalEvidence> {
    let directories = project_directories(&paths);
    let config = read_global_config(&paths.global_config)?;
    let project_state_key = matching_project_key(&config, &paths.current_dir);

    let mcp = McpEvidence {
        user: config.mcp_servers.len(),
        project: read_project_mcp_server_count(&project_mcp_path(&paths))?,
        local: project_state_key
            .as_deref()
            .and_then(|key| config.projects.get(key))
            .map_or(0, |entry| entry.mcp_servers.len()),
    };

    let skills = SkillEvidence {
        personal: count_skill_manifests(&paths.claude_home.join(SKILLS_DIRECTORY))?,
        project: count_project_skill_manifests(&directories)?,
    };

    Ok(LocalEvidence {
        instruction_sources: discover_instruction_sources(&paths.claude_home, &directories)?,
        project_state_key,
        paths,
        mcp,
        skills,
    })
}

/// Every directory whose project-scoped configuration applies here, ordered
/// from the project root down to the current directory.
fn project_directories(paths: &LocalPaths) -> Vec<PathBuf> {
    let Some(project_root) = paths.project_root.as_deref() else {
        return vec![paths.current_dir.clone()];
    };

    let mut directories = Vec::new();
    for directory in paths.current_dir.ancestors() {
        directories.push(directory.to_path_buf());
        if directory == project_root {
            break;
        }
    }
    directories.reverse();

    directories
}

/// Where a project-scoped MCP configuration would live. Without a project root
/// marker the current directory is the best available root, which is the same
/// fallback `project_directories` makes for skills and instructions.
fn project_mcp_path(paths: &LocalPaths) -> PathBuf {
    paths
        .project_root
        .as_deref()
        .unwrap_or(&paths.current_dir)
        .join(PROJECT_MCP_FILENAME)
}

fn read_global_config(path: &Path) -> io::Result<GlobalConfig> {
    match fs::read(path) {
        Ok(contents) => serde_json::from_slice(&contents).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to parse {}: {error}", path.display()),
            )
        }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(GlobalConfig::default()),
        Err(error) => Err(error),
    }
}

fn read_project_mcp_server_count(path: &Path) -> io::Result<usize> {
    match fs::read(path) {
        Ok(contents) => serde_json::from_slice::<ProjectMcpConfig>(&contents)
            .map(|config| config.mcp_servers.len())
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("failed to parse {}: {error}", path.display()),
                )
            }),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(error) => Err(error),
    }
}

/// The `projects` key recorded for the current directory, matching the exact
/// path first and its canonical form second.
///
/// Claude Code records local-scope state under the directory a session was
/// started in, so this deliberately keys on the current directory rather than
/// on the project root. The matched key travels with the evidence so a reader
/// never has to guess which directory the state describes.
fn matching_project_key(config: &GlobalConfig, current_dir: &Path) -> Option<String> {
    let exact = current_dir.to_string_lossy().into_owned();
    if config.projects.contains_key(&exact) {
        return Some(exact);
    }

    let canonical = fs::canonicalize(current_dir).ok()?;
    let canonical = canonical.to_string_lossy().into_owned();
    config
        .projects
        .contains_key(&canonical)
        .then_some(canonical)
}

fn count_project_skill_manifests(directories: &[PathBuf]) -> io::Result<usize> {
    let mut total = 0;
    for directory in directories {
        total +=
            count_skill_manifests(&directory.join(CLAUDE_HOME_DIRECTORY).join(SKILLS_DIRECTORY))?;
    }

    Ok(total)
}

/// Count skill directories that carry a `SKILL.md` manifest. A manifest that
/// exists but cannot be examined fails the inspection rather than quietly
/// lowering the count.
fn count_skill_manifests(skills_root: &Path) -> io::Result<usize> {
    let entries = match fs::read_dir(skills_root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(error),
    };

    let mut manifests = 0;
    for entry in entries {
        let manifest = entry?.path().join(SKILL_MANIFEST_FILENAME);
        if is_manifest_file(&manifest)? {
            manifests += 1;
        }
    }

    Ok(manifests)
}

/// Whether one candidate path is a readable `SKILL.md` file. A stray file or
/// directory in a skills root is not a manifest, an absent manifest is not a
/// skill, and a manifest that exists but cannot be examined is an error rather
/// than a silently lower count.
fn is_manifest_file(manifest: &Path) -> io::Result<bool> {
    match fs::metadata(manifest) {
        Ok(metadata) => Ok(metadata.is_file()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) if error.kind() == io::ErrorKind::NotADirectory => Ok(false),
        Err(error) => Err(error),
    }
}

fn discover_instruction_sources(
    claude_home: &Path,
    directories: &[PathBuf],
) -> io::Result<Vec<InstructionSource>> {
    let mut sources = Vec::new();
    push_instruction_source(
        &mut sources,
        &claude_home.join(INSTRUCTION_FILENAMES[0]),
        InstructionScope::Global,
    )?;

    for directory in directories {
        for filename in INSTRUCTION_FILENAMES {
            push_instruction_source(
                &mut sources,
                &directory.join(filename),
                InstructionScope::Project,
            )?;
        }
    }

    Ok(sources)
}

fn push_instruction_source(
    sources: &mut Vec<InstructionSource>,
    path: &Path,
    scope: InstructionScope,
) -> io::Result<()> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };

    if metadata.is_file() {
        sources.push(InstructionSource {
            path: path.to_path_buf(),
            scope,
            size_bytes: metadata.len(),
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let root = env::temp_dir().join(format!(
                "goalrail-inspect-claude-{}-{name}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("fixture root should be created");

            Self { root }
        }

        fn directory(&self, relative: &str) -> PathBuf {
            let path = self.root.join(relative);
            fs::create_dir_all(&path).expect("fixture directory should be created");
            path
        }

        fn file(&self, relative: &str, contents: &str) -> PathBuf {
            let path = self.root.join(relative);
            fs::create_dir_all(path.parent().expect("fixture file should have a parent"))
                .expect("fixture parent should be created");
            fs::write(&path, contents).expect("fixture file should be written");
            path
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn config_dir_overrides_both_the_home_and_the_global_config() {
        let (home, config) = resolve_claude_home(
            Some(OsString::from("/tenant/config")),
            Some(OsString::from("/home/user")),
            None,
        )
        .expect("an explicit config directory should resolve");

        assert_eq!(home, PathBuf::from("/tenant/config"));
        assert_eq!(config, PathBuf::from("/tenant/config/.claude.json"));
    }

    #[test]
    fn falls_back_to_home_then_userprofile_and_ignores_empty_values() {
        let (home, config) = resolve_claude_home(
            Some(OsString::new()),
            Some(OsString::from("/home/user")),
            Some(OsString::from("C:\\Users\\user")),
        )
        .expect("HOME should resolve");
        assert_eq!(home, PathBuf::from("/home/user/.claude"));
        assert_eq!(config, PathBuf::from("/home/user/.claude.json"));

        let (home, config) = resolve_claude_home(
            None,
            Some(OsString::new()),
            Some(OsString::from("/profile")),
        )
        .expect("USERPROFILE should resolve");
        assert_eq!(home, PathBuf::from("/profile/.claude"));
        assert_eq!(config, PathBuf::from("/profile/.claude.json"));

        let error = resolve_claude_home(None, Some(OsString::new()), None)
            .expect_err("no home should fail closed");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn project_directories_run_from_the_root_down_to_the_current_directory() {
        let paths = LocalPaths {
            claude_home: PathBuf::from("/home/user/.claude"),
            global_config: PathBuf::from("/home/user/.claude.json"),
            current_dir: PathBuf::from("/work/repo/apps/web"),
            project_root: Some(PathBuf::from("/work/repo")),
        };

        assert_eq!(
            project_directories(&paths),
            vec![
                PathBuf::from("/work/repo"),
                PathBuf::from("/work/repo/apps"),
                PathBuf::from("/work/repo/apps/web"),
            ]
        );
    }

    #[test]
    fn project_directories_without_a_root_cover_only_the_current_directory() {
        let paths = LocalPaths {
            claude_home: PathBuf::from("/home/user/.claude"),
            global_config: PathBuf::from("/home/user/.claude.json"),
            current_dir: PathBuf::from("/work/loose"),
            project_root: None,
        };

        assert_eq!(
            project_directories(&paths),
            vec![PathBuf::from("/work/loose")]
        );
    }

    #[test]
    fn missing_configuration_is_evidence_of_zero_rather_than_failure() {
        let fixture = Fixture::new("missing-config");
        let paths = LocalPaths {
            claude_home: fixture.root.join("claude-home"),
            global_config: fixture.root.join("claude-home/.claude.json"),
            current_dir: fixture.directory("project"),
            project_root: None,
        };

        let evidence = inspect_local_evidence_at(paths).expect("absent files should not fail");

        assert_eq!(evidence.mcp, McpEvidence::default());
        assert_eq!(evidence.skills, SkillEvidence::default());
        assert!(evidence.instruction_sources.is_empty());
        assert_eq!(evidence.project_state_key, None);
    }

    #[test]
    fn a_project_mcp_file_is_read_without_a_project_root_marker() {
        let fixture = Fixture::new("mcp-without-root");
        let loose = fixture.directory("loose");
        fixture.file("loose/.mcp.json", r#"{"mcpServers": {"only": {}}}"#);
        let paths = LocalPaths {
            claude_home: fixture.root.join("claude-home"),
            global_config: fixture.root.join("claude-home/.claude.json"),
            current_dir: loose.clone(),
            project_root: None,
        };

        assert_eq!(project_mcp_path(&paths), loose.join(".mcp.json"));

        let evidence = inspect_local_evidence_at(paths).expect("evidence should be readable");

        assert_eq!(evidence.mcp.project, 1);
    }

    #[test]
    fn an_mcp_server_that_is_not_an_object_fails_closed() {
        let fixture = Fixture::new("non-object-mcp-server");
        fixture.file(
            "claude-home/.claude.json",
            r#"{"mcpServers": {"broken": null}}"#,
        );
        let paths = LocalPaths {
            claude_home: fixture.root.join("claude-home"),
            global_config: fixture.root.join("claude-home/.claude.json"),
            current_dir: fixture.directory("project"),
            project_root: None,
        };

        let error =
            inspect_local_evidence_at(paths).expect_err("a non-object server entry should fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn malformed_global_configuration_fails_closed() {
        let fixture = Fixture::new("malformed-global");
        fixture.file("claude-home/.claude.json", "{not json");
        let paths = LocalPaths {
            claude_home: fixture.root.join("claude-home"),
            global_config: fixture.root.join("claude-home/.claude.json"),
            current_dir: fixture.directory("project"),
            project_root: None,
        };

        let error = inspect_local_evidence_at(paths).expect_err("malformed JSON should fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn malformed_project_mcp_configuration_fails_closed() {
        let fixture = Fixture::new("malformed-project-mcp");
        let project = fixture.directory("project");
        fixture.directory("project/.git");
        fixture.file("project/.mcp.json", "{not json");
        let paths = LocalPaths {
            claude_home: fixture.root.join("claude-home"),
            global_config: fixture.root.join("claude-home/.claude.json"),
            current_dir: project.clone(),
            project_root: Some(project),
        };

        let error = inspect_local_evidence_at(paths).expect_err("malformed JSON should fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn counts_mcp_servers_skills_and_instructions_by_scope() {
        let fixture = Fixture::new("full-evidence");
        let project = fixture.directory("project");
        fixture.directory("project/.git");
        let nested = fixture.directory("project/apps/web");
        fixture.file(
            "claude-home/.claude.json",
            &format!(
                r#"{{
                    "mcpServers": {{ "user-one": {{}}, "user-two": {{}} }},
                    "projects": {{
                        "{}": {{ "mcpServers": {{ "local-one": {{}} }} }},
                        "{}": {{ "mcpServers": {{ "root-scope-must-not-be-counted": {{}} }} }}
                    }}
                }}"#,
                nested.display(),
                project.display()
            ),
        );
        fixture.file(
            "project/.mcp.json",
            r#"{"mcpServers": {"project-one": {}, "project-two": {}, "project-three": {}}}"#,
        );
        fixture.file("claude-home/skills/personal-one/SKILL.md", "personal");
        fixture.file("claude-home/skills/personal-two/SKILL.md", "personal");
        fixture.directory("claude-home/skills/not-a-skill");
        fixture.file("project/.claude/skills/project-one/SKILL.md", "project");
        fixture.file(
            "project/apps/web/.claude/skills/nested-one/SKILL.md",
            "nested",
        );
        fixture.file("claude-home/CLAUDE.md", "global instructions");
        let root_memory = fixture.file("project/CLAUDE.md", "project instructions");
        let root_nested = fixture.file("project/.claude/CLAUDE.md", "nested instructions");
        let root_local = fixture.file("project/CLAUDE.local.md", "local instructions");
        let leaf_memory = fixture.file("project/apps/web/CLAUDE.md", "leaf instructions");

        let paths = LocalPaths {
            claude_home: fixture.root.join("claude-home"),
            global_config: fixture.root.join("claude-home/.claude.json"),
            current_dir: nested.clone(),
            project_root: Some(project),
        };

        let evidence = inspect_local_evidence_at(paths).expect("evidence should be readable");

        assert_eq!(
            evidence.mcp,
            McpEvidence {
                user: 2,
                project: 3,
                local: 1,
            }
        );
        assert_eq!(evidence.mcp.configured(), 6);
        assert_eq!(
            evidence.skills,
            SkillEvidence {
                personal: 2,
                project: 2,
            }
        );
        assert_eq!(
            evidence.project_state_key.as_deref(),
            Some(nested.to_string_lossy().as_ref())
        );

        let discovered: Vec<_> = evidence
            .instruction_sources
            .iter()
            .map(|source| (source.path.clone(), source.scope))
            .collect();
        assert_eq!(
            discovered,
            vec![
                (
                    fixture.root.join("claude-home/CLAUDE.md"),
                    InstructionScope::Global
                ),
                (root_memory, InstructionScope::Project),
                (root_nested, InstructionScope::Project),
                (root_local, InstructionScope::Project),
                (leaf_memory, InstructionScope::Project),
            ]
        );
        assert_eq!(
            evidence.instruction_sources[0].size_bytes,
            "global instructions".len() as u64
        );
    }

    #[test]
    fn instruction_scopes_have_stable_names() {
        assert_eq!(InstructionScope::Global.as_str(), "global");
        assert_eq!(InstructionScope::Project.as_str(), "project");
    }

    #[test]
    fn the_project_root_is_the_nearest_ancestor_holding_the_marker() {
        let fixture = Fixture::new("project-root");
        let project = fixture.directory("project");
        fixture.directory("project/.git");
        let nested = fixture.directory("project/apps/web");

        assert_eq!(
            find_project_root(&nested).expect("marker discovery should succeed"),
            Some(project.clone())
        );
        assert_eq!(
            find_project_root(&project).expect("marker discovery should succeed"),
            Some(project)
        );

        let unmarked = fixture.directory("loose/inner");
        assert_eq!(
            find_project_root(&unmarked).expect("marker discovery should succeed"),
            None
        );
    }

    #[test]
    fn propagates_global_config_read_errors_other_than_absence() {
        let fixture = Fixture::new("unreadable-global");
        let directory = fixture.directory("claude-home/.claude.json");

        let error = read_global_config(&directory)
            .expect_err("a directory in place of the config file should fail");

        assert_ne!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn an_absent_project_mcp_file_counts_zero_servers() {
        let fixture = Fixture::new("absent-project-mcp");
        let absent = fixture.root.join("project/.mcp.json");

        assert_eq!(
            read_project_mcp_server_count(&absent)
                .expect("an absent project MCP file is evidence of zero"),
            0
        );
    }

    #[test]
    fn propagates_project_mcp_read_errors_other_than_absence() {
        let fixture = Fixture::new("unreadable-project-mcp");
        let directory = fixture.directory("project/.mcp.json");

        let error = read_project_mcp_server_count(&directory)
            .expect_err("a directory in place of the MCP file should fail");

        assert_ne!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn only_a_readable_manifest_file_counts_as_a_skill() {
        let fixture = Fixture::new("manifest-shapes");
        let skills = fixture.directory("claude-home/skills");
        fixture.file("claude-home/skills/real/SKILL.md", "skill");
        fixture.directory("claude-home/skills/no-manifest");
        fixture.directory("claude-home/skills/directory-manifest/SKILL.md");
        fixture.file("claude-home/skills/stray-file", "not a skill directory");

        assert!(
            is_manifest_file(&skills.join("real/SKILL.md"))
                .expect("a readable manifest should be examined")
        );
        assert!(
            !is_manifest_file(&skills.join("no-manifest/SKILL.md"))
                .expect("an absent manifest is not an error")
        );
        assert!(
            !is_manifest_file(&skills.join("directory-manifest/SKILL.md"))
                .expect("a directory is not a manifest")
        );
        assert!(
            !is_manifest_file(&skills.join("stray-file/SKILL.md"))
                .expect("a stray file in the skills root is not an error")
        );

        assert_eq!(
            count_skill_manifests(&skills).expect("the skills root should be readable"),
            1
        );
    }

    #[cfg(unix)]
    #[test]
    fn propagates_manifest_errors_other_than_absence() {
        use std::os::unix::fs::PermissionsExt;

        let fixture = Fixture::new("unreadable-manifest");
        let skill = fixture.directory("claude-home/skills/sealed");
        fixture.file("claude-home/skills/sealed/SKILL.md", "skill");
        fs::set_permissions(&skill, fs::Permissions::from_mode(0o000))
            .expect("fixture permissions should be set");

        let result = is_manifest_file(&skill.join(SKILL_MANIFEST_FILENAME));
        fs::set_permissions(&skill, fs::Permissions::from_mode(0o755))
            .expect("fixture permissions should be restored");

        let error = result.expect_err("an unreadable manifest should fail closed");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }

    #[test]
    fn propagates_skill_directory_errors_other_than_absence() {
        let fixture = Fixture::new("unreadable-skills");
        let file = fixture.file("claude-home/skills", "not a directory");

        let error = count_skill_manifests(&file)
            .expect_err("a file in place of the skills directory should fail");

        assert_ne!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn propagates_instruction_metadata_errors_other_than_absence() {
        let fixture = Fixture::new("unreadable-instructions");
        fixture.file("project/.claude", "not a directory");
        let path = fixture.root.join("project/.claude/CLAUDE.md");

        let mut sources = Vec::new();
        let error = push_instruction_source(&mut sources, &path, InstructionScope::Project)
            .expect_err("a file in place of the instruction directory should fail");

        assert_ne!(error.kind(), io::ErrorKind::NotFound);
        assert!(sources.is_empty());
    }
}
