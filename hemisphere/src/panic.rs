//! Thread-local panic hooks.

use std::{
    cell::RefCell,
    panic::PanicHookInfo,
    sync::atomic::{AtomicBool, Ordering},
};

use color_backtrace::{BacktracePrinter, default_output_stream};

pub type PanicHook = Box<dyn Fn(&PanicHookInfo)>;

struct Config {
    hook: Option<PanicHook>,
    print_backtrace: bool,
}

thread_local! {
    static CONFIG: RefCell<Config> = const { RefCell::new(Config { hook: None, print_backtrace: true }) };
}

fn setup() {
    static SETUP: AtomicBool = AtomicBool::new(false);
    if SETUP.load(Ordering::Acquire) {
        return;
    }

    std::panic::set_hook(Box::new(move |info| {
        CONFIG.with_borrow(|config| {
            if let Some(hook) = &config.hook {
                hook(info)
            }

            if config.print_backtrace {
                _ = BacktracePrinter::new().print_panic_info(info, &mut default_output_stream());
            }
        });
    }));

    SETUP.store(true, Ordering::Release);
}

/// Sets a thread-local panic hook.
pub fn set_hook(hook: PanicHook, print_backtrace: bool) {
    setup();
    CONFIG.set(Config {
        hook: Some(hook),
        print_backtrace,
    });
}

