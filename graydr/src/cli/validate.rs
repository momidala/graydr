use std::fs;
use std::path::PathBuf;
use crate::hooks::CompileHooks;
use crate::parser::module::parse_module_file;
use crate::parser::template::parse_template_file;
use crate::cli::args::ValidateArgs;

pub fn run_validate(args: ValidateArgs, hooks: &CompileHooks) {
    let mut has_error = false;

    for path in &args.files {
        let filename = path.to_string_lossy();
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let source = match ext {
            "gmod" => {
                // Use module resolver (EXT-1) for .gmod files
                let module_name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                let include_dir = path.parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("."));
                match hooks.module_resolver.resolve(module_name, &[include_dir]) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("error reading {}: {}", filename, e);
                        has_error = true;
                        continue;
                    }
                }
            }
            _ => {
                // For .gtpl and other files, use fs::read_to_string directly
                match fs::read_to_string(path) {
                    Ok(s) => s,
                    Err(e) => {
                        eprintln!("error reading {}: {}", filename, e);
                        has_error = true;
                        continue;
                    }
                }
            }
        };
        let result = match ext {
            "gmod" => parse_module_file(&source, &filename).map(|_| ()),
            "gtpl" => parse_template_file(&source, &filename).map(|_| ()),
            _ => {
                eprintln!("warning: skipping unknown file type: {}", filename);
                continue;
            }
        };
        if let Err(e) = result {
            eprintln!("{}", e);
            has_error = true;
        }
    }

    if has_error {
        std::process::exit(1);
    }
}
