use crate::domain::{IConfig, Label, Loc, LocId, Memory, Value};
use crate::parser::{BinaryOp, Expr, ExprKind, Program};

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


pub fn intp (pgm: &Program) -> Memory {
    let config = init_conf(pgm);
    match eval(&config, &pgm.main, false) {
        Ok(output) => output.mem,
        Err(error) => {
            eprintln!("{error}");
            Memory::default()
        }
    }
}

fn init_conf(_pgm: &Program) -> Config {
    Config::new()
}

fn eval(config: &Config, expression: &Expr, lvalue: bool) -> Result<Output, String> {
    let mut mem = config.mem.clone();
    let mut iconf = config.iconfig;
    let mut value = Value::Unit;
    let mut label_out = Label::Unit;
    let Expr { label, kind, .. } = expression;

    match kind {
        ExprKind::Unit => {}
        ExprKind::Integer(n) => value = Value::Integer(*n),
        ExprKind::Variable(name) => {
            let loc = Loc::new(LocId::Var(name.clone()), 0);
            if lvalue {
                value = Value::Loc(loc);
            } else if let Some((stored_value, stored_label)) = mem.get(&loc) {
                value = stored_value.clone();
                label_out = *stored_label;
            } else {
                return Err(format!("정의되지 않은 변수: {name}"));
            }
        }
        ExprKind::AddressOf(name) => {
            value = Value::Loc(Loc::new(LocId::Var(name.clone()), 0));
        }
        ExprKind::Enable => {
            iconf = IConfig::Enable;
            value = Value::Integer(1);
        }
        ExprKind::Disable => iconf = IConfig::Disable,
        ExprKind::Binary(op, left_expr, right_expr) => {
            let left = eval(config, left_expr, false)?;
            let right = eval(
                &Config { mem: left.mem, iconfig: left.iconf },
                right_expr,
                false,
            )?;
            value = op.apply(left.value, right.value)?;
            mem = right.mem;
            iconf = right.iconf;
        }
        ExprKind::Dereference(base_expr, offset_expr) => {
            let base_output = eval(config, base_expr, false)?;
            let offset_output = eval(
                &Config {
                    mem: base_output.mem,
                    iconfig: base_output.iconf,
                },
                offset_expr,
                false,
            )?;
            let offset = match offset_output.value {
                Value::Integer(offset) => offset,
                _ => return Err(format!("Deref offset must be an integer: {offset_expr:?}")),
            };
            let base = match base_output.value {
                Value::Loc(loc) => loc,
                _ => return Err(format!("Deref base must be a location: {base_expr:?}")),
            };
            let loc = base.add(offset)?;

            if lvalue {
                value = Value::Loc(loc);
            } else if let Some((stored_value, stored_label)) = offset_output.mem.get(&loc) {
                value = stored_value.clone();
                label_out = *stored_label;
            } else {
                return Err(format!("메모리에 정의되지 않은 주소: {loc}"));
            }
            mem = offset_output.mem;
            iconf = offset_output.iconf;
        }
        ExprKind::Malloc(size_expr, initial_expr) => {
            let size_output = eval(config, size_expr, false)?;
            let size = match size_output.value {
                Value::Integer(size) => usize::try_from(size)
                    .map_err(|_| "Malloc size는 0 이상의 정수여야 합니다".to_owned())?,
                _ => return Err("Malloc size는 정수여야 합니다".to_owned()),
            };
            let initial = eval(
                &Config {
                    mem: size_output.mem,
                    iconfig: size_output.iconf,
                },
                initial_expr,
                false,
            )?;
            let loc_id = LocId::Label(*label);
            value = Value::Loc(Loc::new(loc_id, 0));
            mem = initial.mem;
            iconf = initial.iconf;
            for offset in 0..size {
                mem.insert(
                    Loc::new(LocId::Label(*label), offset),
                    initial.value.clone(),
                    *label,
                );
            }
        }
        ExprKind::Assign(lhs_expr, rhs_expr) => {
            let lhs = eval(config, lhs_expr, true)?;
            let rhs = eval(
                &Config { mem: lhs.mem, iconfig: lhs.iconf },
                rhs_expr,
                false,
            )?;
            let loc = match lhs.value {
                Value::Loc(loc) => loc,
                _ => return Err("대입의 왼쪽 식은 location이어야 합니다".to_owned()),
            };
            mem = rhs.mem;
            mem.insert(loc, rhs.value.clone(), *label);
            iconf = rhs.iconf;
            value = rhs.value;
        }
        ExprKind::Sequence(first, second) => {
            let first = eval(config, first, false)?;
            return eval(
                &Config { mem: first.mem, iconfig: first.iconf },
                second,
                false,
            );
        }
        ExprKind::If(condition, then_expr, else_expr) => {
            let condition = eval(config, condition, false)?;
            let branch = match condition.value {
                Value::Integer(n) if n != 0 => then_expr,
                Value::Integer(_) => else_expr,
                _ => return Err("Condition expression must evaluate to an integer".to_owned()),
            };
            return eval(
                &Config { mem: condition.mem, iconfig: condition.iconf },
                branch,
                false,
            );
        }
        ExprKind::While(condition_expr, body_expr) => {
            let mut current = Config {
                mem,
                iconfig: iconf,
            };
            loop {
                let condition = eval(&current, condition_expr, false)?;
                let should_continue = match condition.value {
                    Value::Integer(n) => n != 0,
                    _ => {
                        return Err("Condition expression must evaluate to an integer".to_owned());
                    }
                };
                current = Config {
                    mem: condition.mem,
                    iconfig: condition.iconf,
                };
                if !should_continue {
                    break;
                }
                let body = eval(&current, body_expr, false)?;
                current = Config {
                    mem: body.mem,
                    iconfig: body.iconf,
                };
            }
            mem = current.mem;
            iconf = current.iconfig;
        }
    }

    Ok(Output {
        value,
        label: label_out,
        iconf,
        mem,
    })
}

