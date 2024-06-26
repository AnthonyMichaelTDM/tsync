mod to_typescript;
mod typescript;
pub mod utils;

use state::InitCell;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

/// the #[tsync] attribute macro which marks structs and types to be translated into the final typescript definitions file
pub use tsync_macro::tsync;

use crate::to_typescript::ToTypescript;

pub(crate) static DEBUG: InitCell<bool> = InitCell::new();

/// macro to check from an syn::Item most of them have ident attribs
/// that is the one we want to print but not sure!
macro_rules! check_tsync {
    ($x: ident, in: $y: tt, $z: tt) => {
        let has_tsync_attribute = has_tsync_attribute(&$x.attrs);
        if *DEBUG.get() {
            if has_tsync_attribute {
                println!("Encountered #[tsync] {}: {}", $y, $x.ident.to_string());
            } else {
                println!("Encountered non-tsync {}: {}", $y, $x.ident.to_string());
            }
        }

        if has_tsync_attribute {
            $z
        }
    };
}

#[derive(Default)]
pub struct BuildState /*<'a>*/ {
    pub types: String,
    pub unprocessed_files: Vec<PathBuf>,
    // pub ignore_file_config: Option<gitignore::File<'a>>,
}

/// Settings for the build process
#[derive(Default)]
pub struct BuildSettings {
    pub uses_type_interface: bool,
    pub enable_const_enums: bool,
}

// fn should_ignore_file(ignore_file: &gitignore::File, entry: &DirEntry) -> bool {
//     let path = entry.path();

//     ignore_file.is_excluded(&path).unwrap_or(false)
// }

fn has_tsync_attribute(attributes: &[syn::Attribute]) -> bool {
    utils::has_attribute("tsync", attributes)
}

impl BuildState {
    fn write_comments(&mut self, comments: &Vec<String>, indentation_amount: i8) {
        let indentation = utils::build_indentation(indentation_amount);
        match comments.len() {
            0 => (),
            1 => {
                self.types
                    .push_str(&format!("{}/** {} */\n", indentation, &comments[0]))
            }
            _ => {
                self.types
                    .push_str(&format!("{}/**\n", indentation));
                for comment in comments {
                    self.types
                        .push_str(&format!("{} * {}\n", indentation, &comment))
                }
                self.types
                    .push_str(&format!("{} */\n", indentation))
            }
        }
    }
}

fn process_rust_item(item: syn::Item, state: &mut BuildState, config: &BuildSettings) {
    match item {
        syn::Item::Const(exported_const) => {
            check_tsync!(exported_const, in: "const", {
                exported_const.convert_to_ts(state, config);
            });
        }
        syn::Item::Struct(exported_struct) => {
            check_tsync!(exported_struct, in: "struct", {
                exported_struct.convert_to_ts(state, config);
            });
        }
        syn::Item::Enum(exported_enum) => {
            check_tsync!(exported_enum, in: "enum", {
                exported_enum.convert_to_ts(state, config);
            });
        }
        syn::Item::Type(exported_type) => {
            check_tsync!(exported_type, in: "type", {
                exported_type.convert_to_ts(state, config);
            });
        }
        _ => {}
    }
}

fn process_rust_file<P: AsRef<Path>>(
    input_path: P,
    state: &mut BuildState,
    config: &BuildSettings,
) {
    if *DEBUG.get() {
        println!("processing rust file: {:?}", input_path.as_ref().to_str());
    }

    let Ok(src) = std::fs::read_to_string(input_path.as_ref()) else {
        state.unprocessed_files.push(input_path.as_ref().to_path_buf());
        return;
    };

    let Ok(syntax) = syn::parse_file(&src) else {
        state.unprocessed_files.push(input_path.as_ref().to_path_buf());
        return;
    };

    syntax
        .items
        .into_iter()
        .for_each(|item| process_rust_item(item, state, config))
}

fn check_path<P: AsRef<Path>>(path: P, state: &mut BuildState) -> bool {
    if !path.as_ref().exists() {
        if *DEBUG.get() { println!("Path `{:#?}` does not exist", path.as_ref()); }
        state.unprocessed_files.push(path.as_ref().to_path_buf());
        return false;
    }

    true
}

fn check_extension<P: AsRef<Path>>(ext: &OsStr, path: P) -> bool {
    if !ext.eq_ignore_ascii_case("rs") {
        if *DEBUG.get() {
            println!("Encountered non-rust file `{:#?}`", path.as_ref());
        }
        return false
    }

    true
}

/// Ensure that the walked entry result is Ok and its path is a file. If not,
/// return `None`, otherwise return `Some(DirEntry)`.
fn validate_dir_entry(entry_result: walkdir::Result<DirEntry>, path: &Path) -> Option<DirEntry> {
    match entry_result {
        Ok(entry) => {
            // skip dir files because they're going to be recursively crawled by WalkDir
            if entry.path().is_dir() {
                if *DEBUG.get() {
                    println!("Encountered directory `{}`", path.display());
                }
                return None;
            }

            Some(entry)
        }
        Err(e) => {
            println!("An error occurred whilst walking directory `{}`...", path.display());
            println!("Details: {e:?}");
            None
        }
    }
}

fn process_dir_entry<P: AsRef<Path>>(path: P, state: &mut BuildState, config: &BuildSettings) {
    WalkDir::new(path.as_ref())
        .sort_by_file_name()
        .into_iter()
        .filter_map(|res| validate_dir_entry(res, path.as_ref()))
        .for_each(|entry| {
            // make sure it is a rust file
            if entry
                .path()
                .extension()
                .is_some_and(|extension| check_extension(extension, path.as_ref()))
            {
                process_rust_file(entry.path(), state, config);
            }
        })
}

pub fn generate_typescript_defs(
    input: Vec<PathBuf>,
    output: PathBuf,
    debug: bool,
    enable_const_enums: bool,
) {
    DEBUG.set(debug);

    let uses_type_interface = output
        .to_str()
        .map(|x| x.ends_with(".d.ts"))
        .unwrap_or(true);

    let config = BuildSettings {
        uses_type_interface,
        enable_const_enums,
    };

    let mut state = BuildState::default();

    state
        .types
        .push_str("/* This file is generated and managed by tsync */\n");

    input.into_iter().for_each(|path| {
        if check_path(&path, &mut state) {
            if path.is_dir() {
                process_dir_entry(&path, &mut state, &config)
            } else {
                process_rust_file(&path, &mut state, &config);
            }
        }
    });

    if debug {
        println!("======================================");
        println!("FINAL FILE:");
        println!("======================================");
        println!("{}", state.types);
        println!("======================================");
        println!("Note: Nothing is written in debug mode");
        println!("======================================");
    } else {
        // Verify that the output file either doesn't exists or has been generated by tsync.
        if output.exists() {
            if !output.is_file() {
                panic!("Specified output path is a directory but must be a file.")
            }
            let original_file = File::open(&output).expect("Couldn't open output file");
            let mut buffer = BufReader::new(original_file);

            let mut first_line = String::new();

            buffer
                .read_line(&mut first_line)
                .expect("Unable to read line");

            if first_line.trim() != "/* This file is generated and managed by tsync */" {
                panic!("Aborting: specified output file exists but doesn't have \"/* This file is generated and managed by tsync */\" as the first line.")
            }
        }

        match std::fs::write(&output, state.types.as_bytes()) {
            Ok(_) => println!("Successfully generated typescript types, see {:#?}", output),
            Err(_) => println!("Failed to generate types, an error occurred."),
        }
    }

    if !state.unprocessed_files.is_empty() {
        println!("Could not parse the following files:");
    }

    for unprocessed_file in state.unprocessed_files {
        println!("• {:#?}", unprocessed_file);
    }
}
