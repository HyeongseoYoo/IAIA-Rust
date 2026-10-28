use crate::domain::{IConfig, Label, Loc, LocId, Memory, Value};
use crate::parser::{Expr, ExprKind, Program};

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

pub fn intp(pgm: &Program) -> Result<Memory, String> {
    let config = init_conf(pgm)
        .map_err(|error| format!("Init code 실행 중 에러: {error}"))?;
    let output = eval(&config, &pgm.main, false)
        .map_err(|error| format!("Main code 실행 중 에러: {error}"))?;
    Ok(output.mem)
}

fn init_conf(pgm: &Program) -> Result<Config, String> {
    let mut config = Config::new();
    let output = eval (&config, &pgm.global, false).map_err(|error| format!("Global eval 실패: {error}"))?;
    config.mem = output.mem;
    config.iconfig = output.iconf;
    Ok(config)
}

fn eval(config: &Config, expression: &Expr, lvalue: bool) -> Result<Output, String> {
    let mut mem = config.mem.clone();
    let mut iconf: IConfig = config.iconfig;
    let mut v: Value = Value::Unit;
    let mut p: Label = Label::Unit;

    let Expr { label, line: _, kind: expr } = expression;

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
            mem = output2.mem;
            iconf = output2.iconf;
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
                    output2.mem.check_deref(&deref_loc)?;
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
            mem = output2.mem;
            iconf = output2.iconf;
        }
        ExprKind::Malloc(e1, e2) => {
            let output1 = eval(config, e1, false)?;
            let output2 = eval(
                &Config {
                    mem: output1.mem,
                    iconfig: output1.iconf,
                },
                e2,
                false,
            )?;

            let n = match output1.value {
                Value::Integer(i) if i>0 => usize::try_from(i).map_err(|_| "Malloc 크기가 너무 큽니다".to_owned())?,
                Value::Integer(_) => return Err(format!("Malloc 할 수 없음 (n <= 0): {e1:?}")),
                _ => return Err(format!("Malloc 크기는 정수여야 합니다: {e1:?}")),
            };
            let v_init = match output2.value {
                Value::Integer(i) => i,
                _ => return Err(format!("Deref 할 수 없는 식: {e2:?}")),
            };

            mem = output2.mem;
            iconf = output2.iconf;
            v = Value::Loc(mem.allocate(*label, n, Value::Integer(v_init))?);
        }
        ExprKind::Assign(e1, e2) => {
            let output1 = eval(config, e1, true)?;
            let output2 = eval(
                &Config {
                    mem: output1.mem,
                    iconfig: output1.iconf,
                },
                e2,
                false,
            )?;

            mem = output2.mem;
            iconf = output2.iconf;

            let l = match output1.value {
                Value::Loc(loc) => loc,
                _ => return Err(format!("Assign 할 수 없는 주소: {e1:?}"))
            };

            mem.insert(l, output2.value, *label);

        }
        ExprKind::Sequence(e1, e2) => {
            let output1 = eval(config, e1, false)?;
            let output2 = eval(
                &Config {
                    mem: output1.mem,
                    iconfig: output1.iconf,
                },
                e2,
                false,
            )?;

            mem = output2.mem;
            iconf = output2.iconf;
        }
        ExprKind::If(c, e1, e2) => {
            let output1 = eval(config, c, false)?;
            let conf_next = &Config {
                mem: output1.mem,
                iconfig: output1.iconf,
            };
            let output_r = match output1.value {
                Value::Integer(0) => eval(conf_next, e2, false)?,
                Value::Integer(_) => eval(conf_next, e1, false)?,
                _ => return Err(format!("Condition으로 사용할 수 없음 {c:?}")),
            };

            mem = output_r.mem;
            iconf = output_r.iconf;
        }
        ExprKind::While(c, e) => {
            let mut loop_config = Config {
                mem: config.mem.clone(),
                iconfig: config.iconfig,
            };

            loop {
                let condition = eval(&loop_config, c, false)?;
                match condition.value {
                    Value::Integer(0) => {
                        // Preserve side effects from the final condition check.
                        mem = condition.mem;
                        iconf = condition.iconf;
                        break;
                    }
                    Value::Integer(_) => {
                        let body = eval(
                            &Config {
                                mem: condition.mem,
                                iconfig: condition.iconf,
                            },
                            e,
                            false,
                        )?;
                        loop_config = Config {
                            mem: body.mem,
                            iconfig: body.iconf,
                        };
                    }
                    _ => return Err(format!("Condition으로 사용할 수 없음 {c:?}")),
                }
            }
        }
    }

    Ok(Output {
        value: v,
        label: p,
        iconf,
        mem,
    })
}
