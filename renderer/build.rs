use wesl::Wesl;

fn main() {
    let mut wesl = Wesl::new("shaders");
    wesl.use_sourcemap(true);
    wesl.set_options(wesl::CompileOptions {
        imports: true,
        condcomp: false,
        generics: false,
        strip: true,
        lower: true,
        validate: true,
        ..Default::default()
    });

    wesl.build_artifact(&"package::blit".parse().unwrap(), "blit");
    wesl.build_artifact(&"package::uber".parse().unwrap(), "uber");
}
