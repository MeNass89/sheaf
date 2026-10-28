fn main() {
    // The interface's French (Preferences ▸ Interface ▸ Language; iLovePDF's and Acrobat's interface
    // language): gettext catalogues bundled into the binary, chosen at run time, nothing loaded
    // from disk. Messages carry no context, so one entry serves every screen.
    let config = slint_build::CompilerConfiguration::new()
        .with_bundled_translations("../sheaf-app/translations")
        .with_default_translation_context(slint_build::DefaultTranslationContext::None);
    println!("cargo:rerun-if-changed=../sheaf-app/translations");
    slint_build::compile_with_config("../sheaf-app/ui/main.slint", config).unwrap();
}
