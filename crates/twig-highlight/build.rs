use std::{env, fs, path::PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let grammar_dir = manifest_dir.join("grammars");
    println!("cargo:rerun-if-changed={}", grammar_dir.display());

    let mut grammar_files: Vec<_> = fs::read_dir(&grammar_dir)
        .unwrap_or_else(|error| {
            panic!(
                "failed to read grammar directory {}: {error}",
                grammar_dir.display()
            )
        })
        .map(|entry| {
            entry
                .expect("failed to read grammar directory entry")
                .path()
        })
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "toml")
        })
        .collect();
    grammar_files.sort();

    let mut generated = String::from("pub const BUILTIN_GRAMMARS: &[&str] = &[\n");
    for path in grammar_files {
        println!("cargo:rerun-if-changed={}", path.display());
        generated.push_str("    include_str!(");
        generated.push_str(&format!("{:?}", path));
        generated.push_str("),\n");
    }
    generated.push_str("];\n");

    let output = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("grammars.rs");
    fs::write(output, generated).expect("failed to write generated grammar registry");
}
