mod file;

use dsp::Dsp;
use libtest_mimic::{Arguments, Failed, Trial};

fn run_test(file: file::TestFile) -> Result<(), Failed> {
    let mut dsp = Dsp::default();
    Ok(())
}

fn main() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let tests_dir = std::fs::read_dir(format!("{manifest}/zayd-tests/tests")).unwrap();

    let mut tests = vec![];
    for test in tests_dir {
        let test = test.unwrap();
        if test.file_type().unwrap().is_file() {
            let file = file::TestFile::open(test.path());
            tests.push(Trial::test(
                test.file_name().to_string_lossy().into_owned(),
                move || run_test(file),
            ));
        }
    }

    let args = Arguments::from_args();
    libtest_mimic::run(&args, tests).exit();
}
