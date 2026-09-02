//! rust-embed requires `webui/dist` at compile time. CI and a fresh
//! `cargo test` do not run vite, so write a stub index.html when missing.

fn main() {
    let dist = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../webui/dist");
    let index = dist.join("index.html");
    println!("cargo:rerun-if-changed=../webui/dist/index.html");
    if index.exists() {
        return;
    }
    std::fs::create_dir_all(&dist).expect("create webui/dist for rust-embed");
    std::fs::write(
        &index,
        "<!doctype html><title>wishket radar</title><div id=\"app\"></div>\n",
    )
    .expect("write webui/dist placeholder");
}
