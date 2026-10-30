use std::{env, fs, path::Path};
fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for path in entries {
        assert!(!path.is_symlink(), "payload must not contain symlinks");
        if path.is_dir() {
            walk(root, &path, out);
        } else {
            let name = path.file_name().unwrap().to_str().unwrap();
            if ["gpui-kit.d.ts", "jsconfig.json"].contains(&name) || name.ends_with(".error") {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/");
            out.push(format!("({rel:?}, include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/payload/{rel}\"))),"));
        }
    }
}
fn main() {
    println!("cargo:rerun-if-changed=payload");
    let root = std::path::PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("payload");
    let mut out = vec![];
    walk(&root, &root, &mut out);
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("payload.rs"),
        format!(
            "pub const PAYLOAD: &[(&str, &[u8])] = &[\n{}\n];",
            out.join("\n")
        ),
    )
    .unwrap();
}
