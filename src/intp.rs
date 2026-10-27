use std::fs::TryLockError::Error;
use std::process::ExitCode;

use crate::domain::{IConfig, Label, Loc, LocId, Memory, Section, Value};
use crate::parser::{Expr, ExprKind, Program, BinaryOp};

#[derive(Debug)]
struct Config {
    mem: Memory,
    iconfig: IConfig,
}

impl Config {
    pub fn new() -> Self {
        Self {
            mem: Memory::default(),
            iconfig: IConfig::Enable,
        }
    }
}

#[derive(Debug)]
struct Output {
    value: Value,
    label: Label,
    iconf: IConfig,
    mem: Memory,
}

pub fn intp(pgm: &Program) -> Memory {
    let config: Config = init_conf(&pgm);
    match eval(&config, &pgm.main, false) {
        Ok(output) => output.mem,
        Err(error) => {
            eprintln!("{error}");
            Memory::default()
        }
    }
}

fn init_conf(pgm: &Program) -> Config {
    let mut config = Config::new();
    config
}

fn eval(config: &Config, expression: &Expr, lvalue: bool) -> Result<Output, String> {
    let mut mem = config.mem.clone();
    let mut iconf: IConfig = config.iconfig;
    let mut v: Value = Value::Unit;
    let mut p: Label = Label::Unit;

    let Expr { label: _, line: _, kind: expr } = expression;

    match expr {
        ExprKind::Unit => {}
        ExprKind::Integer(n) => {
            v = Value::Integer(*n);
        }
        ExprKind::Variable(x) => {
            let loc = Loc::new(LocId::Var(x.clone()), 0);
            if lvalue {
                v = Value::Loc(loc);
            } else {
                match mem.get(&loc) {
                    Some((v_get, p_get)) => {
                        v = v_get.clone();
                        p = *p_get;
                    }
                    None => return Err(format!("정의되지 않은 변수: {x}")),
                }
            }
        }
        ExprKind::AddressOf(x) => {
            let loc = Loc::new(LocId::Var(x.clone()), 0);
            v = Value::Loc(loc);
        }
        ExprKind::Enable => {
            iconf = IConfig::Enable;
        }
        ExprKind::Disable => {
            iconf = IConfig::Disable;
        }
        ExprKind::Binary(bop, e1, e2) => {
            let output1 = eval(config, e1, false)?;
            let output2 = eval(
                &Config {
                    mem: output1.mem,
                    iconfig: output1.iconf,
                },
                e2,
                false,
            )?;
            v = bop.apply(output1.value, output2.value)?;
        }
        ExprKind::Dereference(e1, e2) => {
            let output1 = eval(config, e1, false)?;
            let output2 = eval(
                &Config {
                    mem: output1.mem,
                    iconfig: output1.iconf,
                },
                e2,
                false,
            )?;
            let offset = match output2.value {
                Value::Integer(i) => i,
                _ => return Err(format!("Deref 할 수 없는 식: {e2:?}")),
            };
            match output1.value {
                Value::Loc(loc) => {
                    let deref_loc = loc.add(offset)?;
                    if lvalue {
                        v = Value::Loc(deref_loc);
                    } else if let Some((v_get, p_get)) = output2.mem.get(&deref_loc) {
                        v = v_get.clone();
                        p = *p_get;
                    } else {
                        return Err("메모리에 정의되지 않은 주소".to_owned());
                    }
                }
                _ => return Err(format!("Deref 할 수 없는 식: {e1:?}")),
            }
        }
        _ => {}
    }

    Ok(Output {
        value: v,
        label: p,
        iconf,
        mem,
    })
}
