//! Copied from https://github.com/Dushistov/rust_swig/blob/master/jni_tests/build.rs.
extern crate syntex;
extern crate rust_swig;
extern crate env_logger;
extern crate bindgen;

use std::{env, io, fmt};
use std::path::{Path, PathBuf};

use self::Error::*;

enum Error {
    FileNotFound(String),
    InvalidUnicodePath(String),
    BindingsFailed,
    Io(io::Error),
}

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Io(e)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FileNotFound(ref file) => write!(f, "Cannot find file {}.", file),
            InvalidUnicodePath(ref to) => write!(f, "Invalid unicode in path to {}.", to),
            BindingsFailed => write!(f, "Failed to generate bindings"),
            Io(ref e) => e.fmt(f),
        }
    }
}

fn search_file_in_directory(dirs: &[&Path], file: &str) -> Result<PathBuf, Error> {
    for dir in dirs.iter() {
        let file_path = dir.join(file);
        if file_path.exists() && file_path.is_file() {
            return Ok(file_path);
        }
    }
    Err(FileNotFound(file.to_owned()))
}

fn gen_binding(include_dirs: &[&Path], c_headers: &[&str], output_rust: &Path) -> Result<(), Error> {
    assert!(!c_headers.is_empty());
    let c_file_path = search_file_in_directory(include_dirs, c_headers[0])?;

    if let Ok(out_meta) = output_rust.metadata() {
        let mut res_recent_enough = true;
        for header in c_headers.iter() {
            let c_file_path = search_file_in_directory(include_dirs, header)?;
            let c_meta = c_file_path.metadata()?;
            if !(c_meta.modified().unwrap() < out_meta.modified().unwrap()) {
                res_recent_enough = false;
                break;
            }
        }
        if res_recent_enough {
            return Ok(());
        }
    }

    let mut bindings: ::bindgen::Builder = bindgen::builder().header(c_file_path.to_str().unwrap());
    bindings = include_dirs.iter()
        .fold(bindings,
              |acc, x| acc.clang_arg("-I".to_string() + x.to_str().unwrap()));

    bindings = bindings.unstable_rust(false);
    bindings = c_headers[1..].iter()
        .fold(Ok(bindings),
              |acc: Result<::bindgen::Builder, Error>, header| {
            let c_file_path = search_file_in_directory(include_dirs, header)?;
            let c_file_str = c_file_path.to_str().ok_or_else(|| InvalidUnicodePath((*header).to_owned()))?;
            Ok(acc?.clang_arg("-include").clang_arg(c_file_str))
        })?;

    let generated_bindings = bindings.generate().map_err(|_| BindingsFailed)?;
    generated_bindings.write_to_file(output_rust)?;

    Ok(())
}

fn main() {
    env_logger::init().unwrap();

    let out_dir = env::var("OUT_DIR").expect("expected OUT_DIR env variable to be set");

    let java_home = env::var("JAVA_HOME").expect("expected JAVA_HOME env variable to be set");
    let mut java_include_dir = PathBuf::new();
    java_include_dir.push(java_home);
    java_include_dir.push("include");

    let mut java_sys_include_dir = java_include_dir.clone();
    let target = env::var("TARGET").expect("expected TARGET env variable to be set");
    java_sys_include_dir.push(if target.contains("windows") {
        "win32"
    } else {
        "linux"
    });

    let result = gen_binding(&[&java_include_dir, &java_sys_include_dir],
                             &["jni.h"],
                             &Path::new(&out_dir).join("jni_c_header.rs"));

    if let Err(e) = result {
        panic!("Generating C bindings failed: {}", e);
    }

    let mut registry = syntex::Registry::new();
    let swig_gen = rust_swig::Generator::new(rust_swig::LanguageConfig::Java {
        output_dir: Path::new("src").join("main").join("java").join("net").join("daboross").join("scrs"),
        package_name: "net.daboross.scrs".into(),
    });
    swig_gen.register(&mut registry);

    let src = Path::new("src").join("main").join("rust").join("lib.rs");
    let dst = Path::new(&out_dir).join("lib.rs");
    registry.expand("scrs-jvm", &src, &dst).expect("expected syntex expansion to succeed");
}
