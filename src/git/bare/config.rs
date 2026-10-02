//! The `.twig.toml` presentation configuration: parsing, ignore patterns, and
//! the optional `[paper]` and `[scripts]` sections.

use std::collections::{BTreeMap, HashMap};

use serde::Deserialize;

use super::handle::RepoHandle;

/// Config filenames, tried in order. `.twig.toml` is the current name and
/// `.twig` the historic short form; `.fig.toml` and `fig.toml` are read for
/// repositories created before the rename that never updated their config file.
pub(super) const CONFIG_FILENAMES: [&str; 4] = [".twig.toml", ".twig", ".fig.toml", "fig.toml"];

/// Presentation configuration from `.twig.toml`
#[derive(Debug, Deserialize, Default, Clone)]
pub struct PresentConfig {
    pub files: Vec<String>,
    #[serde(flatten)]
    pub template_vars: HashMap<String, String>,
}

/// One downloadable script inside a `[scripts]` group.
#[derive(Debug, Deserialize, Default, Clone)]
pub struct ScriptEntry {
    /// Display name. Empty means "use the file's base name".
    #[serde(default)]
    pub name: String,
    /// Repository-relative path, served raw for `curl`.
    pub path: String,
    /// Interpreter the run command pipes into. Defaults to `bash`.
    #[serde(default = "default_script_shell")]
    pub shell: String,
}

fn default_script_shell() -> String {
    "bash".to_string()
}

/// One `[scripts.<group>]` table: the group's display name, its script
/// entries, and the groups nested inside it — nesting in TOML is what builds
/// the hierarchy the Scripts tab renders.
#[derive(Debug, Deserialize, Default, Clone)]
pub struct ScriptGroupConfig {
    /// Display name. Empty means "use the group's key".
    #[serde(default)]
    pub name: String,
    /// The scripts in this group, in configured order.
    #[serde(default)]
    pub scripts: Vec<ScriptEntry>,
    /// `[scripts.<group>.<subgroup>]` tables.
    #[serde(flatten)]
    pub groups: BTreeMap<String, ScriptGroupConfig>,
}

/// Scripts configuration from `[scripts]` in `.twig.toml`.
///
/// The section is optional; the Scripts tab shows only when it names a group
/// that carries at least one script.
#[derive(Debug, Deserialize, Default, Clone)]
pub struct ScriptsConfig {
    #[serde(flatten)]
    pub groups: BTreeMap<String, ScriptGroupConfig>,
}

/// A script group flattened out of the `[scripts]` tree, carrying the
/// slash-joined key path that identifies it in URLs.
#[derive(Debug, Clone)]
pub struct ScriptGroupNode {
    /// Slash-joined group keys, the URL path under `/scripts`.
    pub key: String,
    /// `name` from the group, else its last key segment.
    pub label: String,
    /// The group's own scripts, in configured order.
    pub scripts: Vec<ScriptEntry>,
    /// Keys of the groups directly nested inside this one.
    pub children: Vec<String>,
}

impl ScriptEntry {
    /// The label shown in the UI: the configured name, else the file base name.
    pub fn label(&self) -> &str {
        let trimmed = self.name.trim();
        if trimmed.is_empty() {
            self.path.rsplit('/').next().unwrap_or(&self.path)
        } else {
            trimmed
        }
    }
}

/// Depth-first walk of the `[scripts]` tree, alphabetical within each level.
/// Groups with no scripts and no usable descendants are pruned; the remaining
/// ones are collected in pre-order so a parent always precedes its children.
fn collect_script_groups(
    groups: &BTreeMap<String, ScriptGroupConfig>,
    prefix: &str,
    nodes: &mut Vec<ScriptGroupNode>,
) {
    for (key, group) in groups {
        let group_key = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}/{key}")
        };
        let scripts: Vec<ScriptEntry> = group
            .scripts
            .iter()
            .filter(|entry| !entry.path.trim().is_empty())
            .cloned()
            .collect();

        // Children are collected directly after the marker, so slicing
        // `nodes[marker..]` yields exactly this group's descendants.
        let marker = nodes.len();
        collect_script_groups(&group.groups, &group_key, nodes);
        let children = nodes[marker..]
            .iter()
            .map(|node| node.key.clone())
            .collect::<Vec<_>>();

        if scripts.is_empty() && children.is_empty() {
            continue;
        }

        let label = group.name.trim();
        let label = if label.is_empty() {
            key.as_str()
        } else {
            label
        };
        nodes.insert(
            marker,
            ScriptGroupNode {
                key: group_key,
                label: label.to_string(),
                scripts,
                children,
            },
        );
    }
}

/// Paper (long-form reading) configuration from `.twig.toml`.
///
/// The section is optional; a repository only offers the Paper tab when
/// `[paper]` is present and its directory holds at least one Markdown page.
#[derive(Debug, Deserialize, Default, Clone)]
pub struct PaperConfig {
    /// Directory holding the paper's Markdown pages, read in sorted order.
    #[serde(default = "default_paper_dir")]
    pub dir: String,
}

impl PaperConfig {
    /// Whether the configured directory is usable as a page source.
    pub fn is_configured(&self) -> bool {
        !self.dir.trim().trim_matches('/').is_empty()
    }
}

fn default_paper_dir() -> String {
    "paper".to_string()
}

/// Configuration from `.twig.toml` file in repository
#[derive(Debug, Deserialize, Default)]
pub struct TwigConfig {
    #[serde(default)]
    pub ignore_for_view: Vec<String>,
    /// Tabs to display. If empty or not present, all tabs are shown.
    #[serde(default)]
    pub tabs: Vec<String>,
    /// Whether the repository can be deleted from the UI.
    #[serde(default)]
    pub deleteable: bool,
    /// Whether the repository is private (read operations require authentication).
    #[serde(default)]
    pub private: bool,
    /// Presentation configuration.
    #[serde(default)]
    pub present: PresentConfig,
    /// Paper (long-form reading) configuration. Present only when `[paper]`
    /// appears in the configuration file.
    #[serde(default)]
    pub paper: Option<PaperConfig>,
    /// Scripts configuration. Present only when `[scripts]` appears in the
    /// configuration file; the Scripts tab shows when it holds a group.
    #[serde(default)]
    pub scripts: Option<ScriptsConfig>,
}

/// A repository's parsed configuration alongside its source.
#[derive(Default)]
pub struct TwigConfigWithRaw {
    pub config: TwigConfig,
    pub raw: Option<String>,
    pub filename: Option<String>,
    /// The parse error, when `raw` is present but invalid. `config` then holds
    /// defaults so the rest of the UI keeps working around the bad file.
    pub error: Option<String>,
}

impl TwigConfigWithRaw {
    /// Parses `content` and records a human-readable error instead of silently
    /// discarding it, so the UI can point at the offending line.
    pub(super) fn from_source(content: &str, filename: &str) -> Self {
        match TwigConfig::parse(content) {
            Ok(config) => Self {
                config,
                raw: Some(content.to_string()),
                filename: Some(filename.to_string()),
                error: None,
            },
            Err(error) => Self {
                config: TwigConfig::default(),
                raw: Some(content.to_string()),
                filename: Some(filename.to_string()),
                error: Some(error.to_string()),
            },
        }
    }
}

impl TwigConfig {
    /// Load config from `.twig.toml` file in the repository
    /// Falls back to `.twig`, `.fig.toml` and `fig.toml` for backwards compatibility
    /// Opens and closes the repo each time — prefer `RepoHandle::load_config_with_raw` when possible
    pub fn load(root: &str, namespace: &str, repo: &str) -> Self {
        let Ok(handle) = RepoHandle::open(root, namespace, repo) else {
            return Self::default();
        };
        handle.load_config_with_raw().config
    }

    /// Parse config from TOML content. The error is returned rather than
    /// swallowed so callers can surface it.
    fn parse(content: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(content)
    }

    /// Check if a file path matches any of the ignore patterns
    pub fn should_ignore(&self, file_path: &str) -> bool {
        let file_path = file_path.trim_start_matches("./");

        for pattern in &self.ignore_for_view {
            // Check if the file path starts with the pattern (for folder patterns)
            // or matches exactly (for file patterns)
            if pattern.ends_with('/') {
                // Folder pattern (e.g., "skills/")
                let pattern_prefix = pattern.trim_end_matches('/');
                if file_path.starts_with(pattern_prefix)
                    && (file_path.len() == pattern_prefix.len()
                        || file_path[pattern_prefix.len()..].starts_with('/'))
                {
                    return true;
                }
            } else if pattern.contains('/') {
                // Path pattern with subdirectories (e.g., "docs/temp")
                if file_path.starts_with(pattern)
                    && (file_path.len() == pattern.len()
                        || file_path[pattern.len()..].starts_with('/'))
                {
                    return true;
                }
            } else {
                // Simple file or folder name pattern
                // Check if any path component matches
                if file_path.split('/').any(|component| component == pattern) {
                    return true;
                }
            }
        }
        false
    }

    /// Every configured script group, pre-order and alphabetical within each
    /// level. Empty when the repository configures no scripts, which is what
    /// keeps the Scripts tab hidden.
    pub fn script_groups(&self) -> Vec<ScriptGroupNode> {
        self.scripts.as_ref().map_or_else(Vec::new, |scripts| {
            let mut nodes = Vec::new();
            collect_script_groups(&scripts.groups, "", &mut nodes);
            nodes
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_twig_config_parse() {
        let toml_content = r#"
ignore_for_view = ["skills/", "temp", "drafts/"]
deleteable = true
"#;
        let config = TwigConfig::parse(toml_content).expect("valid config");
        assert_eq!(config.ignore_for_view.len(), 3);
        assert!(config.deleteable);
        assert!(!config.private);
    }

    #[test]
    fn test_present_config_parse() {
        let toml_content = r#"
[present]
files = ["slides/intro.md"]
author = "Jane Doe"
"#;
        let config = TwigConfig::parse(toml_content).expect("valid config");
        assert!(
            config
                .present
                .files
                .contains(&"slides/intro.md".to_string())
        );
        assert_eq!(
            config.present.template_vars.get("author").unwrap(),
            "Jane Doe"
        );
    }

    #[test]
    fn test_paper_config_parse() {
        let config = TwigConfig::parse("[paper]\ndir = \"manuscript\"\n").expect("valid config");
        let paper = config.paper.expect("paper section should be present");
        assert_eq!(paper.dir, "manuscript");
        assert!(paper.is_configured());

        // An empty section falls back to the default directory.
        let config = TwigConfig::parse("[paper]\n").expect("valid config");
        assert_eq!(config.paper.expect("paper section").dir, "paper");

        // Without the section the Paper tab stays off.
        assert!(
            TwigConfig::parse("private = true")
                .expect("valid config")
                .paper
                .is_none()
        );
    }

    #[test]
    fn test_paper_config_empty_dir_is_not_configured() {
        let config = TwigConfig::parse("[paper]\ndir = \"  \"\n").expect("valid config");
        assert!(!config.paper.expect("paper section").is_configured());
    }

    #[test]
    fn test_scripts_config_parses_groups_and_entries() {
        let config = TwigConfig::parse(
            r#"
[scripts.linux]
name = "Linux"
scripts = [
    { name = "Install", path = "scripts/linux/install.sh" },
]

[scripts.linux.maintenance]
scripts = [{ name = "Cleanup", path = "scripts/cleanup.sh" }]

[scripts.macos]
scripts = [{ path = "scripts/macos/setup.sh" }]
"#,
        )
        .expect("valid config");

        let nodes = config.script_groups();
        let keys = nodes
            .iter()
            .map(|node| node.key.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            keys,
            vec!["linux", "linux/maintenance", "macos"],
            "groups flatten pre-order, alphabetical within each level"
        );

        let linux = &nodes[0];
        assert_eq!(linux.label, "Linux");
        assert_eq!(linux.scripts.len(), 1);
        assert_eq!(linux.scripts[0].name, "Install");
        assert_eq!(linux.scripts[0].shell, "bash", "bash is the default shell");
        assert_eq!(linux.children, vec!["linux/maintenance"]);

        let maintenance = &nodes[1];
        assert_eq!(maintenance.label, "maintenance", "key is the fallback name");
        assert!(maintenance.children.is_empty());

        assert_eq!(
            nodes[2].scripts[0].label(),
            "setup.sh",
            "an entry without a name falls back to its file base name"
        );
    }

    #[test]
    fn test_scripts_config_absent_or_empty_yields_no_groups() {
        // Without the section the Scripts tab stays off entirely.
        assert!(
            TwigConfig::parse("private = true")
                .expect("valid config")
                .script_groups()
                .is_empty()
        );

        // A group with no scripts and no children is pruned.
        let config =
            TwigConfig::parse("[scripts.linux]\nname = \"Linux\"\n").expect("valid config");
        assert!(config.script_groups().is_empty());

        // An entry missing its path is a broken config, reported as a parse
        // error rather than silently dropped.
        let parsed = TwigConfigWithRaw::from_source(
            "[scripts.linux]\nscripts = [{ name = \"Broken\" }]\n",
            ".twig.toml",
        );
        assert!(parsed.error.is_some(), "a pathless entry must be reported");

        // But an empty child does not prune a parent that has its own scripts.
        let config = TwigConfig::parse(
            "[scripts.linux]\nscripts = [{ path = \"a.sh\" }]\n[scripts.linux.empty]\n",
        )
        .expect("valid config");
        let nodes = config.script_groups();
        assert_eq!(nodes.len(), 1);
        assert!(nodes[0].children.is_empty());
    }

    #[test]
    fn test_config_parse_errors_are_reported_not_swallowed() {
        let parsed = TwigConfigWithRaw::from_source("not = = valid\n", ".twig.toml");
        assert!(parsed.error.is_some(), "the parse error must be kept");
        assert!(
            parsed.config.paper.is_none() && !parsed.config.deleteable,
            "a broken file falls back to defaults so the UI keeps rendering"
        );
        assert_eq!(parsed.filename.as_deref(), Some(".twig.toml"));
        assert_eq!(parsed.raw.as_deref(), Some("not = = valid\n"));

        let parsed = TwigConfigWithRaw::from_source("private = true\n", ".twig.toml");
        assert!(parsed.error.is_none());
        assert!(parsed.config.private);

        let parsed = TwigConfigWithRaw::default();
        assert!(parsed.error.is_none() && parsed.raw.is_none());
    }

    #[test]
    fn test_twig_config_should_ignore_folder() {
        let config = TwigConfig {
            ignore_for_view: vec!["skills".to_string()],
            tabs: vec![],
            deleteable: false,
            private: false,
            present: PresentConfig::default(),
            paper: None,
            scripts: None,
        };

        // Should ignore files in the skills folder
        assert!(config.should_ignore("skills/README.md"));
        assert!(!config.should_ignore("src/main.rs"));
    }

    #[test]
    fn test_twig_config_should_ignore_with_slash_pattern() {
        let config = TwigConfig {
            ignore_for_view: vec!["docs/temp".to_string()],
            tabs: vec![],
            deleteable: false,
            private: false,
            present: PresentConfig::default(),
            paper: None,
            scripts: None,
        };

        assert!(config.should_ignore("docs/temp/file.md"));
        assert!(!config.should_ignore("docs/other.md"));
        assert!(!config.should_ignore("docs/temporary/file.md"));
    }

    #[test]
    fn test_twig_config_should_ignore_multiple_patterns() {
        let config = TwigConfig {
            ignore_for_view: vec!["node_modules/".to_string(), "temp".to_string()],
            tabs: vec![],
            deleteable: false,
            private: false,
            present: PresentConfig::default(),
            paper: None,
            scripts: None,
        };

        assert!(config.should_ignore("node_modules/package.json"));
        assert!(config.should_ignore("temp/notes.md"));
        assert!(config.should_ignore("src/temp/data.txt"));
        assert!(!config.should_ignore("README.md"));
    }

    #[test]
    fn test_twig_config_empty() {
        let config = TwigConfig::default();
        assert!(config.ignore_for_view.is_empty());
        assert!(!config.should_ignore("anything.txt"));
    }

    #[test]
    fn test_twig_config_should_ignore_folder_with_trailing_slash() {
        let config = TwigConfig {
            ignore_for_view: vec!["build/".to_string()],
            tabs: vec![],
            deleteable: false,
            private: false,
            present: PresentConfig::default(),
            paper: None,
            scripts: None,
        };

        assert!(config.should_ignore("build/output.js"));
        assert!(config.should_ignore("build"));
        assert!(!config.should_ignore("builder/file.js"));
    }

    #[test]
    fn test_twig_config_should_ignore_exact_file() {
        let config = TwigConfig {
            ignore_for_view: vec!["SECRET.md".to_string()],
            tabs: vec![],
            deleteable: false,
            private: false,
            present: PresentConfig::default(),
            paper: None,
            scripts: None,
        };

        assert!(config.should_ignore("SECRET.md"));
        assert!(config.should_ignore("docs/SECRET.md"));
        assert!(!config.should_ignore("secret.md"));
    }

    #[test]
    fn test_twig_config_toml_format_parsing() {
        let toml_content = r#"
ignore_for_view = ["temp", "scratch/"]
deleteable = false
"#;
        let config = TwigConfig::parse(toml_content).expect("valid config");
        assert_eq!(config.ignore_for_view, vec!["temp", "scratch/"]);
        assert!(!config.deleteable);
    }

    #[test]
    fn test_twig_config_parse_private() {
        let toml_content = r"
private = true
";
        let config = TwigConfig::parse(toml_content).expect("valid config");
        assert!(config.private);

        let default_config = TwigConfig::parse("").expect("empty config is valid");
        assert!(!default_config.private);
    }
}
