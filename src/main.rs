#![feature(rustc_private)]

extern crate rustc_ast_pretty;
extern crate rustc_driver;
extern crate rustc_error_codes;
extern crate rustc_errors;
extern crate rustc_hash;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

use rustc_driver::Compilation;
use rustc_interface::{interface::Compiler, Queries};
use rustc_middle::ty::TyCtxt;

use std::{env, os::unix::prelude::CommandExt, process::Command};

fn main() {
    if is_run_by_cargo() {
        if should_run_rustc_instead() {
            run_rustc();
        }
        compile_glue_crate();
    } else {
        run_with_cargo();
    }
}

fn is_run_by_cargo() -> bool {
    // If we're run by cargo, then the RUN_BY_CARGO env variable is set.
    env::var_os("RUN_BY_CARGO").is_some()
}

fn run_with_cargo() -> ! {
    let error = Command::new("cargo")
        .env("PWD", "cargo-breaking-internal")
        .env("RUN_BY_CARGO", "1")
        .env("RUSTC_WORKSPACE_WRAPPER", env::current_exe().unwrap())
        .arg("check")
        .args(["--manifest-path", "./cargo-breaking-internal/Cargo.toml"])
        .exec();

    panic!("Failed to run cargo: {}", error);
}

fn should_run_rustc_instead() -> bool {
    let first_arg = env::args().nth(3);

    !matches!(
        first_arg.as_ref().map(String::as_str),
        Some("cargo_breaking_internal")
    )
}

fn run_rustc() -> ! {
    let error = Command::new("rustc").args(env::args().skip(2)).exec();

    panic!("Failed to run rustc: {}", error);
}

fn compile_glue_crate() {
    let mut args = env::args().skip(1).collect::<Vec<_>>();

    let out = Command::new("rustc")
        .arg("--print=sysroot")
        .current_dir(".")
        .output()
        .unwrap();
    let sysroot = String::from_utf8(out.stdout).unwrap();

    args.push(format!("--sysroot={}", sysroot.trim()));

    let mut interface = CompilerInterface::new();

    rustc_driver::RunCompiler::new(args.as_slice(), &mut interface)
        .run()
        .unwrap()
}

struct CompilerInterface;

impl CompilerInterface {
    pub fn new() -> CompilerInterface {
        CompilerInterface
    }
}

impl rustc_driver::Callbacks for CompilerInterface {
    fn after_analysis<'tcx>(
        &mut self,
        _compiler: &Compiler,
        queries: &'tcx Queries<'tcx>,
    ) -> Compilation {
        queries.global_ctxt().unwrap().take().enter(|tcx| {
            dump_public_fns(&tcx);
        });

        Compilation::Stop
    }
}

fn dump_public_fns(tcx: &TyCtxt) {
    let crates = tcx.crates(()).iter();

    for krate in crates {
        let symbols = tcx
            .exported_symbols(*krate)
            .iter()
            .take(5)
            .collect::<Vec<_>>();

        dbg!(tcx.crate_name(*krate));
        dbg!(symbols);
    }
}
