//! Copies the built web UI (web/dist) into OUT_DIR so web.rs can include_str! it.
//! Missing files become small stubs so `cargo build` works without npm.
use std::{collections::hash_map::DefaultHasher, env, fs, hash::{Hash, Hasher}, path::Path};

fn main() {
    println!("cargo:rerun-if-changed=web/dist");
    println!("cargo:rerun-if-changed=build.rs");
    let out = env::var("OUT_DIR").unwrap();
    let mut hasher = DefaultHasher::new();
    for f in ["index.html", "app.js", "app.css"] {
        let src = Path::new("web/dist").join(f);
        let dst = Path::new(&out).join(f);
        match fs::read(&src) {
            Ok(b) => fs::write(&dst, b).unwrap(),
            Err(_) => {
                let stub = match f {
                    "index.html" => "<!doctype html><meta charset=utf-8><title>Mimir</title><p>UI not built \u{2014} run npm run build in web/</p>",
                    "app.js" => "/* UI not built \u{2014} run npm run build in web/ */",
                    _ => "/* UI not built */",
                };
                fs::write(&dst, stub).unwrap();
            }
        }
        fs::read(&dst).unwrap().hash(&mut hasher);
    }
    // ETag of the embedded UI: changes exactly when the assets do.
    println!("cargo:rustc-env=MIMIR_ASSET_HASH={:016x}", hasher.finish());
}
