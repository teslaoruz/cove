fn main() {
    slint_build::compile_with_config("ui/app.slint", slint_build::CompilerConfiguration::new().with_style("cupertino".into())).expect("Slint UI failed to compile");
}
