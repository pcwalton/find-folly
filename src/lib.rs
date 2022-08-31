// find-folly/src/lib.rs
//
//! This crate is a simple build dependency you can use in your `build.rs` scripts to compile and
//! link against the [Folly C++ library](https://github.com/facebook/folly).
//! 
//! In theory, the [`pkg-config`](https://crates.io/crates/pkg-config) library would be all you
//! need in order to locate Folly, because Folly is typically packed with a `.pc` file. In
//! practice, that is insufficient, because the `.pc` file doesn't fully describe all the
//! dependencies that Folly has, and it has bugs. This crate knows about these idiosyncrasies and
//! provides workarounds for them.
//! 
//! The following snippet should suffice for most use cases:
//!
//! ```ignore
//! let folly = find_folly::probe_folly().unwrap();
//! let mut build = cc::Build::new();
//! ... populate `build` ...
//! build.includes(&folly.include_paths);
//! for other_cflag in &folly.other_cflags {
//!     build.flag(other_cflag);
//! }
//! ```

use pkg_config::{Config, Error as PkgConfigError};
use shlex::Shlex;
use std::io::Error as IoError;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use thiserror::Error;

/// Information about the Folly library.
///
/// You can the information in this structure to populate a `cc::Build` in order to compile code
/// that uses Folly:
///
///     let folly = find_folly::probe_folly().unwrap();
///     let mut build = cc::Build::new();
///     ... populate `build` ...
///     build.includes(&folly.include_paths);
///     for other_cflag in &folly.other_cflags {
///         build.flag(other_cflag);
///     }
pub struct Folly {
    pub lib_dirs: Vec<PathBuf>,
    pub include_paths: Vec<PathBuf>,
    pub other_cflags: Vec<String>,
    _priv: (),
}

#[derive(Error, Debug)]
pub enum FollyError {
    #[error("`fmt` dependency couldn't be located")]
    FmtDependency(PkgConfigError),
    #[error("`gflags` dependency couldn't be located")]
    GflagsDependency(PkgConfigError),
    #[error("main `folly` package couldn't be located")]
    MainPackage(IoError),
    #[error("could not find `boost_context`; make sure either `libboost_context.a` or \
            `libboost_context-mt.a` is located in the same directory as Folly")]
    BoostContext,
}

pub fn probe_folly() -> Result<Folly, FollyError> {
    // Folly's `.pc` file is missing the `fmt` and `gflags` dependencies. Find them here.
    Config::new()
        .statik(true)
        .probe("fmt")
        .map_err(FollyError::FmtDependency)?;
    Config::new()
        .statik(true)
        .probe("gflags")
        .map_err(FollyError::GflagsDependency)?;

    // Unfortunately, the `pkg-config` crate doesn't successfully parse some of Folly's
    // dependencies, because it passes the raw `.so` files instead of using `-l` flags. So call
    // `pkg-config` manually.
    let mut folly = Folly::new();
    let output = Command::new("pkg-config")
        .args(&["--static", "--libs", "libfolly"])
        .output()
        .map_err(FollyError::MainPackage)?;
    let output = String::from_utf8(output.stdout).expect("`pkg-config --libs` wasn't UTF-8!");
    for arg in Shlex::new(&output) {
        if arg.starts_with('-') {
            if let Some(rest) = arg.strip_prefix("-L") {
                folly.lib_dirs.push(PathBuf::from(rest));
            } else if let Some(rest) = arg.strip_prefix("-l") {
                println!("cargo:rustc-link-lib={}", rest);
            }
            continue;
        }

        let path = PathBuf::from_str(&arg).unwrap();
        let (parent, lib_name) = match (path.parent(), path.file_stem()) {
            (Some(parent), Some(lib_name)) => (parent, lib_name),
            _ => continue,
        };
        let lib_name = lib_name.to_string_lossy();
        if let Some(rest) = lib_name.strip_prefix("lib") {
            println!("cargo:rustc-link-search={}", parent.display());
            println!("cargo:rustc-link-lib={}", rest);
        }
    }

    // Unfortunately, just like `fmt` and `gflags`, Folly's `.pc` file doesn't contain a link flag
    // for `boost_context`. What's worse, the name varies based on different systems
    // (`libboost_context.a` vs.  `libboost_context-mt.a`). So find that library manually. We assume
    // it's in the same directory as the Folly installation itself.
    let mut found_boost_context = false;
    for lib_dir in &folly.lib_dirs {
        println!("cargo:rustc-link-search={}", lib_dir.display());

        if found_boost_context {
            continue;
        }
        for possible_lib_name in &["boost_context", "boost_context-mt"] {
            let mut lib_dir = (*lib_dir).clone();
            lib_dir.push(&format!("lib{}.a", possible_lib_name));
            if !lib_dir.exists() {
                continue;
            }
            println!("cargo:rustc-link-lib={}", possible_lib_name);
            found_boost_context = true;
            break;
        }
    }
    if !found_boost_context {
        return Err(FollyError::BoostContext);
    }

    let output = Command::new("pkg-config")
        .args(&["--static", "--cflags", "libfolly"])
        .output()
        .map_err(FollyError::MainPackage)?;
    let output = String::from_utf8(output.stdout).expect("`pkg-config --libs` wasn't UTF-8!");

    for arg in output.split_whitespace() {
        if let Some(rest) = arg.strip_prefix("-I") {
            let path = Path::new(rest);
            if path.starts_with("/Library/Developer/CommandLineTools/SDKs")
                && path.ends_with("usr/include")
            {
                // Change any attempt to specify system headers from `-I` to `-isysroot`. `-I` is
                // not the proper way to include a system header and will cause compilation failures
                // on macOS Catalina.
                //
                // Pop off the trailing `usr/include`.
                let sysroot = path.parent().unwrap().parent().unwrap();
                folly.other_cflags.push("-isysroot".to_owned());
                folly.other_cflags.push(sysroot.to_string_lossy().into_owned());
            } else {
                folly.include_paths.push(path.to_owned());
            }
        }
    }

    Ok(folly)
}

impl Folly {
    fn new() -> Self {
        Self {
            lib_dirs: vec![],
            include_paths: vec![],
            other_cflags: vec![],
            _priv: (),
        }
    }
}
