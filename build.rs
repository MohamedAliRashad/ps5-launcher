fn main() {
    slint_build::compile_with_config(
        "ui/app.slint",
        slint_build::CompilerConfiguration::new().embed_resources(slint_build::EmbedResourcesKind::EmbedFiles),
    )
    .expect("Slint UI failed to compile");

    // The PS4 RuTracker snapshot is bundled when it has been collected; until then the
    // launcher simply has no PS4 catalog (an empty file here).
    let source = std::path::Path::new("assets/rutracker/ps4-topics.json");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("ps4-topics.json");
    std::fs::write(&out, std::fs::read(source).unwrap_or_default()).expect("could not stage the PS4 catalog");
    println!("cargo:rerun-if-changed=assets/rutracker/ps4-topics.json");

    // The PKG extractor's write guard (src/pkgguard.c), embedded and preloaded at run time.
    let guard = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("pkgguard.so");
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".into());
    let status = std::process::Command::new(&cc)
        .args(["-shared", "-fPIC", "-O2", "-Wall", "-Wextra", "-Werror", "-o"])
        .arg(&guard)
        .arg("src/pkgguard.c")
        .args(["-ldl", "-lpthread"])
        .status()
        .unwrap_or_else(|e| panic!("could not run the C compiler ({cc}): {e}"));
    assert!(status.success(), "building src/pkgguard.c failed");
    println!("cargo:rerun-if-changed=src/pkgguard.c");
    println!("cargo:rerun-if-changed=build.rs");
}
