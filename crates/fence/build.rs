use std::{env, fs, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=data");
    let root = env::var("CARGO_MANIFEST_DIR").unwrap();
    let mut code = String::from("pub const BUILTIN: &[(&str, &str, &str)] = &[\n");
    for group in ["profiles", "agents", "services"] {
        let mut paths: Vec<_> = fs::read_dir(Path::new(&root).join("data").join(group))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        paths.sort();
        for path in paths {
            if path.extension().is_some_and(|e| e == "toml") {
                let name = path.file_stem().unwrap().to_str().unwrap();
                code.push_str(&format!(
                    "({group:?}, {name:?}, include_str!({:?})),\n",
                    path.to_str().unwrap()
                ));
            }
        }
    }
    code.push_str("];");
    fs::write(
        Path::new(&env::var("OUT_DIR").unwrap()).join("data.rs"),
        code,
    )
    .unwrap();
}
