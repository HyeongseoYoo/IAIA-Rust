use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::rc::Rc;

use crate::domain::{IConfig, Label, Loc, LocId, Memory, Value};
use crate::parser::{Expr, ExprKind, Program};

#[derive(Debug)]
struct Config {
    mem: Memory,
    iconfig: IConfig,
    handlers: Rc<HashMap<usize, Expr>>,
    trigger_points: Rc<RefCell<HashMap<Label, VecDeque<usize>>>>,
}

impl Config {
    pub fn new(trigger_points: HashMap<Label, VecDeque<usize>>) -> Self {
        Self {
            mem: Memory::default(),
            iconfig: IConfig::Enable,
            handlers: Rc::new(HashMap::new()),
            trigger_points: Rc::new(RefCell::new(trigger_points)),
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
    intp_with_interrupts(pgm, HashMap::new())
}

pub fn intp_with_interrupts(
    pgm: &Program,
    trigger_points: HashMap<Label, VecDeque<usize>>,
) -> Result<Memory, String> {
    let config = init_conf(pgm, trigger_points)
        .map_err(|error| format!("Init code 실행 중 에러: {error}"))?;
    let output = eval(&config, &pgm.main, false)
        .map_err(|error| format!("Main code 실행 중 에러: {error}"))?;
    Ok(output.mem)
}

fn init_conf(
    pgm: &Program,
    trigger_points: HashMap<Label, VecDeque<usize>>,
) -> Result<Config, String> {
    let mut config = Config::new(trigger_points);
    let handlers = Rc::new(
    pgm.handlers
        .iter()
        .map(|handler| (handler.id, handler.body.clone()))
        .collect::<HashMap<_, _>>(),
    );
    config.handlers = handlers;
    let output = eval (&config, &pgm.global, false).map_err(|error| format!("Global eval 실패: {error}"))?;
    config.mem = output.mem;
    config.iconfig = output.iconf;
    Ok(config)
}

fn trigger_handler(i: usize, config: &Config) -> Result<Output, String> {
    let mem = config.mem.clone();
    let iconf = IConfig::Disable;
    let body = config
        .handlers
        .get(&i)
        .ok_or_else(|| format!("정의되지 않은 handler: {i}"))?;
    let mut output =  match eval(
        &Config {
            mem,
            iconfig: iconf,
            handlers: Rc::clone(&config.handlers),
            trigger_points: Rc::clone(&config.trigger_points),
        },
        body,
        false,
    ) {
        Ok(o) => {o},
        Err(str) => return Err(format!("Handler {i} 실패 - {str}")),
    };
    output.iconf = config.iconfig;
    Ok(output)
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
                    handlers: Rc::clone(&config.handlers),
                    trigger_points: Rc::clone(&config.trigger_points),
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
                    handlers: Rc::clone(&config.handlers),
                    trigger_points: Rc::clone(&config.trigger_points),
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
                    handlers: Rc::clone(&config.handlers),
                    trigger_points: Rc::clone(&config.trigger_points),
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
                    handlers: Rc::clone(&config.handlers),
                    trigger_points: Rc::clone(&config.trigger_points),
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
                    handlers: Rc::clone(&config.handlers),
                    trigger_points: Rc::clone(&config.trigger_points),
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
                handlers: Rc::clone(&config.handlers),
                trigger_points: Rc::clone(&config.trigger_points),
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
                handlers: Rc::clone(&config.handlers),
                trigger_points: Rc::clone(&config.trigger_points),
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
                                handlers: Rc::clone(&config.handlers),
                                trigger_points: Rc::clone(&config.trigger_points),
                            },
                            e,
                            false,
                        )?;
                        loop_config = Config {
                            mem: body.mem,
                            iconfig: body.iconf,
                            handlers: Rc::clone(&config.handlers),
                            trigger_points: Rc::clone(&config.trigger_points),
                        };
                    }
                    _ => return Err(format!("Condition으로 사용할 수 없음 {c:?}")),
                }
            }
        }
    }

    let mut output = Output {
        value: v,
        label: p,
        iconf,
        mem,
    };

    if output.iconf == IConfig::Enable {
        let handler_id = config
            .trigger_points
            .borrow_mut()
            .get_mut(label)
            .and_then(VecDeque::pop_front);

        if let Some(handler_id) = handler_id {
            let handler_config = Config {
                mem: output.mem,
                iconfig: output.iconf,
                handlers: Rc::clone(&config.handlers),
                trigger_points: Rc::clone(&config.trigger_points),
            };
            let handler_output = trigger_handler(handler_id, &handler_config)?;
            output.mem = handler_output.mem;
            output.iconf = handler_output.iconf;
        }
    }

    Ok(output)
}
