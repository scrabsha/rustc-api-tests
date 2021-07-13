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
use rustc_hir::{
    def::{DefKind, Res},
    def_id::{CrateNum, DefId},
};
use rustc_interface::{interface::Compiler, Queries};
use rustc_middle::{
    middle::cstore::ExternCrateSource,
    ty::{TyCtxt, Visibility},
};

use std::{
    collections::HashMap,
    env,
    fmt::{self, Display, Formatter},
    os::unix::prelude::CommandExt,
    process::Command,
};

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
        let def_id = krate.as_def_id();

        if let Some(extern_crate) = tcx.extern_crate(def_id) {
            match extern_crate.src {
                ExternCrateSource::Extern(_) => {}
                ExternCrateSource::Path => continue,
            }

            if tcx.item_name(def_id).as_str() != "current" {
                continue;
            }

            let api = CrateApi::from_crate(tcx, *krate);
            println!("{}", api);
        }
    }
}

struct CrateApi(HashMap<String, DefId>);

impl CrateApi {
    fn from_crate(tcx: &TyCtxt, cnum: CrateNum) -> CrateApi {
        let def_id = cnum.as_def_id();
        let mut api = CrateApi(HashMap::new());
        api.visit_pub_mod(tcx, def_id);
        api
    }

    fn visit_pub_mod(&mut self, tcx: &TyCtxt, def_id: DefId) {
        let mod_name = tcx.def_path_str(def_id);
        self.0.insert(mod_name, def_id);

        for item in tcx.item_children(def_id) {
            match &item.vis {
                Visibility::Public => {}
                _ => continue,
            }

            let (def_kind, def_id) = match &item.res {
                Res::Def(def_kind, def_id) => (def_kind, def_id),
                _ => continue,
            };

            match def_kind {
                DefKind::Mod => self.visit_pub_mod(tcx, *def_id),
                _ => {
                    self.0.insert(tcx.def_path_str(*def_id), *def_id);
                }
            }
        }
    }
}

impl Display for CrateApi {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0
            .iter()
            .try_for_each(|(path, id)| writeln!(f, "{} ({:?})", path, id))
    }
}
