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
    let config = TwigConfig::parse("[scripts.linux]\nname = \"Linux\"\n").expect("valid config");
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
