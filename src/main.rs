use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use crate::domain::{Label, Section};

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
        "-intp-test" => match (args.next(), args.next(), args.next()) {
            (Some(program), Some(input), None) => run_interrupt_test_mode(&program, &input),
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

fn run_interrupt_test_mode(program_target: &str, input_target: &str) -> ExitCode {
    let program_path = project_test_path(program_target);
    if program_path.extension().is_none_or(|ext| ext != "si") {
        eprintln!("프로그램 파일은 .si 파일이어야 합니다: {program_target}");
        return ExitCode::from(2);
    }

    let input_path = project_test_path(input_target);
    let source = match fs::read_to_string(&program_path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("프로그램 파일을 읽을 수 없습니다 ({}): {error}", program_path.display());
            return ExitCode::FAILURE;
        }
    };
    let input = match fs::read_to_string(&input_path) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("interrupt 입력 파일을 읽을 수 없습니다 ({}): {error}", input_path.display());
            return ExitCode::FAILURE;
        }
    };
    let trigger_points = match parse_trigger_points(&input) {
        Ok(points) => points,
        Err(error) => {
            eprintln!("interrupt 입력 형식 오류: {error}");
            return ExitCode::FAILURE;
        }
    };
    let program = match parser::parse(&source) {
        Ok(program) => program,
        Err(error) => {
            eprintln!("프로그램 파싱 실패: {error}");
            return ExitCode::FAILURE;
        }
    };

    match intp::intp_with_interrupts(&program, trigger_points) {
        Ok(memory) => {
            println!("[성공] {}", program_path.file_name().unwrap_or_default().to_string_lossy());
            println!("{memory}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("[실패] {}: {error}", program_path.file_name().unwrap_or_default().to_string_lossy());
            ExitCode::FAILURE
        }
    }
}

fn parse_trigger_points(input: &str) -> Result<HashMap<Label, VecDeque<usize>>, String> {
    let mut points = HashMap::<Label, VecDeque<usize>>::new();
    for (line_index, line) in input.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }

        let mut fields = line.split_whitespace();
        let label_text = fields.next().expect("non-empty line has a first field");
        let handler_text = fields
            .next()
            .ok_or_else(|| format!("{}번째 줄: handler 번호가 없습니다", line_index + 1))?;
        if fields.next().is_some() {
            return Err(format!("{}번째 줄: 항목은 'label handler_id' 두 개여야 합니다", line_index + 1));
        }

        let label = parse_label(label_text)
            .map_err(|error| format!("{}번째 줄: {error}", line_index + 1))?;
        let handler_id = handler_text
            .parse::<usize>()
            .map_err(|_| format!("{}번째 줄: 잘못된 handler 번호: {handler_text}", line_index + 1))?;
        points.entry(label).or_default().push_back(handler_id);
    }
    Ok(points)
}

fn parse_label(text: &str) -> Result<Label, String> {
    let (section_text, index_text) = text
        .split_once('#')
        .ok_or_else(|| format!("잘못된 label: {text} (예: main#2)"))?;
    let section = match section_text {
        "init" => Section::Init,
        "main" => Section::Main,
        handler if handler.starts_with("handler[") && handler.ends_with(']') => {
            let id_text = &handler[8..handler.len() - 1];
            Section::Handler(
                id_text
                    .parse::<usize>()
                    .map_err(|_| format!("잘못된 handler section: {section_text}"))?,
            )
        }
        _ => return Err(format!("알 수 없는 section: {section_text}")),
    };
    let index = index_text
        .parse::<usize>()
        .map_err(|_| format!("잘못된 label index: {index_text}"))?;
    Ok(Label::At { section, index })
}

fn project_test_path(target: &str) -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let requested = PathBuf::from(target);
    if requested.is_absolute() {
        requested
    } else if requested.components().next().is_some_and(|part| part.as_os_str() == "tests") {
        manifest_dir.join(requested)
    } else {
        manifest_dir.join("tests").join(requested)
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
    eprintln!("  -intp-test <program.si> <interrupt-points.txt>  지정 지점에서 handler를 실행합니다.");
    eprintln!("예: cargo run -- -intp tests/micro1_direct_buggy.si");
    eprintln!("    cargo run -- -intp all");
    eprintln!("    cargo run -- -intp-test micro1_direct_buggy.si micro1_input1.txt");
}
