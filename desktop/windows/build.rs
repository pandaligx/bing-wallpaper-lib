use std::{env, fs, path::PathBuf};

fn main() {
    let icon = fs::canonicalize("../../ico/icon.ico").expect("workspace icon");
    let version = env!("CARGO_PKG_VERSION");
    let resource = fs::read_to_string("../../ico/icon.rc")
        .expect("workspace resource template")
        .replace(
            "\"icon.ico\"",
            &format!(
                "\"{}\"",
                icon.display()
                    .to_string()
                    .trim_start_matches(r"\\?\")
                    .replace('\\', "/")
            ),
        )
        .replace("0,2,37,0", &format!("{},0", version.replace('.', ",")))
        .replace("0.2.37.0", &format!("{version}.0"));
    let rc = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory")).join("app.rc");
    fs::write(&rc, resource).expect("write versioned Windows resource");
    println!("cargo:rerun-if-changed=../../ico/icon.rc");
    println!("cargo:rerun-if-changed=../../ico/icon.ico");
    embed_resource::compile(rc, embed_resource::NONE)
        .manifest_optional()
        .expect("compile Windows resources");
}
