use wesl::Wesl;

fn main() {
    let mut wesl = Wesl::new("shaders");
    wesl.use_imports(true);
    wesl.use_sourcemap(true);
    wesl.use_lower(true);
    wesl.use_stripping(true);

    Wesl::new("shaders").build_artifact(&"package::blit".parse().unwrap(), "blit");
}
