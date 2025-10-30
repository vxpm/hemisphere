#![feature(trim_prefix_suffix)]

mod file;

use dsp::Dsp;
use libtest_mimic::{Arguments, Failed, Trial};
use std::fmt::Write;

fn parse_code(mut words: &[u16]) -> Vec<dsp::Ins> {
    let mut ins = vec![];
    while !words.is_empty() {
        let opcode = dsp::ins::Opcode::new(words[0]);
        if opcode.needs_extra() {
            ins.push(dsp::Ins::with_extra(words[0], words[1]));
            words = &words[2..];
        } else {
            ins.push(dsp::Ins::new(words[0]));
            words = &words[1..];
        }
    }

    ins
}

struct Divergences {
    code: Vec<dsp::Ins>,
    regs: Vec<(dsp::Reg, u16, u16)>,
}

fn run_case(case: file::TestCase) -> Result<(), Divergences> {
    let mut dsp = Dsp::default();

    // setup
    dsp.regs = case.initial_regs();
    dsp.regs.pc = 62;
    dsp.memory.iram[62..][..case.instructions.len()].copy_from_slice(&case.instructions);
    dsp.memory.iram[62 + case.instructions.len()] = 0x21; // HALT

    // run until halt
    let code = parse_code(&case.instructions);
    while !dsp.control.halt {
        dsp.step();
    }

    // check
    let expected = case.expected_regs();
    let mut divergences = vec![];
    for i in 0..32 {
        let reg = dsp::Reg::new(i);
        let value = dsp.regs.get(reg);
        let expected = expected.get(reg);

        if value != expected {
            divergences.push((reg, value, expected));
        }
    }

    if !divergences.is_empty() {
        return Err(Divergences {
            code,
            regs: divergences,
        });
    }

    Ok(())
}

fn run_test(file: file::TestFile, quiet: bool) -> Result<(), Failed> {
    let mut failures = vec![];
    for (i, case) in file.cases.into_iter().enumerate() {
        if let Err(divergences) = run_case(case) {
            let regs = divergences
                .regs
                .iter()
                .map(|(r, v, e)| format!("{r:?}(v={v:04X}, e={e:04X}), "))
                .collect::<String>();

            let ins = divergences
                .code
                .iter()
                .map(|i| format!("{:?}\r\n", i.opcode()))
                .collect::<String>();

            failures.push(format!(
                "Case {i} failed: {}\r\n{}",
                regs.trim_suffix(", "),
                ins
            ));
        }
    }

    if !failures.is_empty() {
        if quiet {
            return Err(Failed::from(format!(
                "Failed a total of {} cases",
                failures.len()
            )));
        }

        let mut msg = format!("Failed a total of {} cases\r\n\r\n", failures.len());
        let tests_to_show = 8;

        let show = failures.iter().take(tests_to_show);
        for failure in show {
            writeln!(&mut msg, "{}", failure).unwrap();
        }

        if failures.len() > tests_to_show {
            writeln!(
                &mut msg,
                "... and {} others",
                failures.len() - tests_to_show
            )
            .unwrap();
        }

        return Err(Failed::from(msg));
    }

    Ok(())
}

fn main() {
    let manifest = env!("CARGO_MANIFEST_DIR");
    let tests_dir = std::fs::read_dir(format!("{manifest}/zayd-tests/tests")).unwrap();
    let args = Arguments::from_args();
    let env_quiet = std::env::var("QUIET").is_ok();

    let mut tests = vec![];
    for test in tests_dir {
        let test = test.unwrap();
        if test.file_type().unwrap().is_file() {
            let file = file::TestFile::open(test.path());
            tests.push(Trial::test(
                test.file_name().to_string_lossy().into_owned(),
                move || {
                    let result =
                        std::panic::catch_unwind(move || run_test(file, args.quiet || env_quiet));

                    match result {
                        Ok(r) => r,
                        Err(e) => {
                            let mut msg = "<unknown panic>".to_owned();
                            if let Some(s) = e.downcast_ref::<String>() {
                                msg = s.clone();
                            } else if let Some(s) = e.downcast_ref::<&'static str>() {
                                msg = (*s).to_owned();
                            }

                            Err(Failed::from(msg))
                        }
                    }
                },
            ));
        }
    }

    std::panic::set_hook(Box::new(move |_| ()));
    libtest_mimic::run(&args, tests).exit();
}
