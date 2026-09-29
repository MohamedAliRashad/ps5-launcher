fn main() {
    slint_build::compile_with_config(
        "ui/app.slint",
        slint_build::CompilerConfiguration::new().embed_resources(slint_build::EmbedResourcesKind::EmbedFiles),
    )
    .expect("Slint UI failed to compile");
}
