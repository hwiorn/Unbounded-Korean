use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let specs_dir = manifest_dir.join("src/specs");
    let mut entries = fs::read_dir(&specs_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "hsl"))
        .collect::<Vec<_>>();
    entries.sort();

    let mut out = String::from("pub(crate) static SPECS: &[(&str, &str)] = &[\n");
    for path in entries {
        let lang = path.file_stem().unwrap().to_string_lossy();
        let file = path.file_name().unwrap().to_string_lossy();
        out.push_str(&format!(
            "    ({lang:?}, include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/src/specs/{file}\"))),\n"
        ));
        println!("cargo:rerun-if-changed={}", path.display());
    }
    out.push_str("];\n");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::write(out_dir.join("specs_generated.rs"), out).unwrap();
}
