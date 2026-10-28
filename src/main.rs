use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

mod lexer;
mod parser;
mod intp;
mod domain;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(mode) = args.next() else {
        print_usage();
        return ExitCode::from(2);
    };

    match mode.as_str() {
        "-parse" if args.next().is_none() => run_parse_mode(),
        "-intp" => match (args.next(), args.next()) {
            (Some(target), None) => run_interpreter_mode(&target),
            _ => {
                print_usage();
                ExitCode::from(2)
            }
        },
        _ => {
            print_usage();
            ExitCode::from(2)
        }
    }
}

fn run_parse_mode() -> ExitCode {
    let files = match test_files() {
        Ok(files) => files,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };

    let mut failed = false;
    for path in files {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        match fs::read_to_string(&path) {
            Ok(source) => 
                match parser::parse(&source) {
                Ok(pgm) => {
                    println!("[성공] {name}");
                    println!("{}", parser::pretty_print(&pgm));
                },
                Err(error) => {
                    failed = true;
                    println!("[실패] {name}: {error}");
                }
            },
            Err(error) => {
                failed = true;
                println!("[실패] {name}: 파일을 읽을 수 없습니다: {error}");
            }
        }
    }

    if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

fn run_interpreter_mode(target: &str) -> ExitCode {
    let files = if target == "all" {
        match test_files() {
            Ok(files) => files,
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let requested = PathBuf::from(target);
        let path = if requested.is_absolute() {
            requested
        } else if requested.components().next().is_some_and(|part| part.as_os_str() == "tests") {
            manifest_dir.join(requested)
        } else {
            manifest_dir.join("tests").join(requested)
        };
        if path.extension().is_none_or(|ext| ext != "si") {
            eprintln!("-intp에는 .si 파일 경로나 all을 지정해야 합니다: {target}");
            return ExitCode::from(2);
        }
        if !path.is_file() {
            eprintln!("파일을 찾을 수 없습니다: {}", path.display());
            return ExitCode::FAILURE;
        }
        vec![path]
    };

    let mut failed = false;
    for path in files {

        let name = path.file_name().unwrap_or_default().to_string_lossy();
        match fs::read_to_string(&path) {
            Ok(source) => 
                match parser::parse(&source) {
                Ok(pgm) => {
                    match intp::intp(&pgm) {
                        Ok(memory) => {
                            println!("[성공] {name}");
                            println!("{memory}");
                        }
                        Err(error) => {
                            failed = true;
                            println!("[실패] {name} - 실행에 실패했습니다: {error}");
                        }
                    }
                },
                Err(error) => {
                    failed = true;
                    println!("[실패] {name} - 파싱에 실패했습니다.: {error}");
                }
            },
            Err(error) => {
                failed = true;
                println!("[실패] {name}: 파일을 읽을 수 없습니다: {error}");
            }
        }

    }
    if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

fn test_files() -> Result<Vec<PathBuf>, String> {
    let tests_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests");
    let entries = fs::read_dir(&tests_dir)
        .map_err(|error| format!("테스트 디렉터리를 읽을 수 없습니다 ({}): {error}", tests_dir.display()))?;
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("테스트 디렉터리 항목을 읽을 수 없습니다: {error}"))?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "si") {
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn print_usage() {
    eprintln!("사용법:");
    eprintln!("  -parse  tests 디렉터리의 .si 파일을 파싱합니다.");
    eprintln!("  -intp <file|all>  지정 파일 또는 모든 .si 파일을 interpreter로 실행합니다.");
    eprintln!("예: cargo run -- -intp tests/micro1_direct_buggy.si");
    eprintln!("    cargo run -- -intp all");
}
