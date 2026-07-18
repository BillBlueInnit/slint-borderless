fn main() {
    let library_paths = std::collections::HashMap::from([(
        slint_borderless::SLINT_LIBRARY_NAME.to_owned(),
        slint_borderless::slint_library_path(),
    )]);
    let config = slint_build::CompilerConfiguration::new().with_library_paths(library_paths);
    slint_build::compile_with_config("ui/app.slint", config).expect("failed to compile Slint UI");
}
