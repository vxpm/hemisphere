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

    wesl.build_artifact(&"package::blit_vec4f".parse().unwrap(), "blit_vec4f");
    wesl.build_artifact(&"package::blit_f32".parse().unwrap(), "blit_f32");
}
