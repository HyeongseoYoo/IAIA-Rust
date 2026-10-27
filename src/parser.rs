pub use crate::domain::{Label, Section, Value};
use crate::lexer::{self, SourceLocation, Token, TokenKind};
use std::fmt::Write as _;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr {
    pub label: Label,
    pub line: Option<usize>,
    pub kind: ExprKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprKind {
    Unit,
    Integer(i64),
    Variable(String),
    AddressOf(String),
    Enable,
    Disable,
    Binary(BinaryOp, Box<Expr>, Box<Expr>),
    Dereference(Box<Expr>, Box<Expr>),
    Malloc(Box<Expr>, Box<Expr>),
    Assign(Box<Expr>, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    While(Box<Expr>, Box<Expr>),
    Sequence(Box<Expr>, Box<Expr>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Equal,
    Less,
    Greater,
    NotEqual,
    LessEqual,
    GreaterEqual,
    Plus,
    Minus,
    Times,
    And,
    Or,
}

impl BinaryOp {
    pub fn apply(self, v1: Value, v2: Value) -> Result<Value, String> {
        let as_int = |value: bool| if value { 1 } else { 0 };

        match self {
            BinaryOp::Equal => return Ok(Value::Integer(as_int(v1 == v2))),
            BinaryOp::NotEqual => return Ok(Value::Integer(as_int(v1 != v2))),
            _ => {}
        }

        let (Value::Integer(i1), Value::Integer(i2)) = (v1, v2) else {
            return Err("Undefined operation".to_owned());
        };

        let result = match self {
            BinaryOp::Less => as_int(i1 < i2),
            BinaryOp::Greater => as_int(i1 > i2),
            BinaryOp::LessEqual => as_int(i1 <= i2),
            BinaryOp::GreaterEqual => as_int(i1 >= i2),
            BinaryOp::Plus => i1.checked_add(i2).ok_or_else(|| "integer overflow".to_owned())?,
            BinaryOp::Minus => i1.checked_sub(i2).ok_or_else(|| "integer overflow".to_owned())?,
            BinaryOp::Times => i1.checked_mul(i2).ok_or_else(|| "integer overflow".to_owned())?,
            BinaryOp::And => as_int(i1 != 0 && i2 != 0),
            BinaryOp::Or => as_int(i1 != 0 || i2 != 0),
            BinaryOp::Equal | BinaryOp::NotEqual => unreachable!(),
        };

        Ok(Value::Integer(result))
    }
}

pub fn binary_op(bop: BinaryOp, v1: Value, v2: Value) -> Result<Value, String> {
    bop.apply(v1, v2)
}

enum InfixOp {
    Assign,
    Binary(BinaryOp),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Handler {
    pub id: usize,
    pub body: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub global: Expr,
    pub handlers: Vec<Handler>,
    pub main: Expr,
}

pub fn pretty_print(program: &Program) -> String {
    let mut output = String::new();
    output.push_str("Program\n  global:\n");
    write_expr(&mut output, &program.global, 4, None);

    output.push_str("  handlers:\n");
    if program.handlers.is_empty() {
        output.push_str("    (none)\n");
    } else {
        for handler in &program.handlers {
            let _ = writeln!(output, "    Handler {}:", handler.id);
            write_expr(&mut output, &handler.body, 6, None);
        }
    }

    output.push_str("  main:\n");
    write_expr(&mut output, &program.main, 4, None);
    output
}

fn write_expr(output: &mut String, expr: &Expr, indent: usize, field: Option<&str>) {
    let padding = " ".repeat(indent);
    if let Some(field) = field {
        let _ = write!(output, "{padding}{field}: ");
    } else {
        output.push_str(&padding);
    }

    let description = match &expr.kind {
        ExprKind::Unit => "Unit".to_owned(),
        ExprKind::Integer(value) => format!("Integer({value})"),
        ExprKind::Variable(name) => format!("Variable({name:?})"),
        ExprKind::Enable => "Enable".to_owned(),
        ExprKind::Disable => "Disable".to_owned(),
        ExprKind::Binary(op, _, _) => format!("Binary({})", binary_op_symbol(*op)),
        ExprKind::AddressOf(name) => format!("AddressOf({name:?})"),
        ExprKind::Dereference(_, _) => "Dereference".to_owned(),
        ExprKind::Malloc(_, _) => "Malloc".to_owned(),
        ExprKind::Assign(_, _) => "Assign".to_owned(),
        ExprKind::If(_, _, _) => "If".to_owned(),
        ExprKind::While(_, _) => "While".to_owned(),
        ExprKind::Sequence(_, _) => "Sequence".to_owned(),
    };

    let label = expr.label.to_string();
    let _ = write!(output, "{description} [{label}");
    if let Some(line) = expr.line {
        let _ = write!(output, ", line {line}");
    }
    output.push_str("]\n");

    let child_indent = indent + 2;
    match &expr.kind {
        ExprKind::Binary(_, left, right) | ExprKind::Assign(left, right) => {
            write_expr(output, left, child_indent, Some("left"));
            write_expr(output, right, child_indent, Some("right"));
        }
        ExprKind::Dereference(base, index) => {
            write_expr(output, base, child_indent, Some("base"));
            write_expr(output, index, child_indent, Some("index"));
        }
        ExprKind::Malloc(size, initial) => {
            write_expr(output, size, child_indent, Some("size"));
            write_expr(output, initial, child_indent, Some("initial"));
        }
        ExprKind::If(condition, then_branch, else_branch) => {
            write_expr(output, condition, child_indent, Some("condition"));
            write_expr(output, then_branch, child_indent, Some("then"));
            write_expr(output, else_branch, child_indent, Some("else"));
        }
        ExprKind::While(condition, body) => {
            write_expr(output, condition, child_indent, Some("condition"));
            write_expr(output, body, child_indent, Some("body"));
        }
        ExprKind::Sequence(first, second) => {
            write_expr(output, first, child_indent, Some("first"));
            write_expr(output, second, child_indent, Some("second"));
        }
        ExprKind::Unit
        | ExprKind::Integer(_)
        | ExprKind::Variable(_)
        | ExprKind::Enable
        | ExprKind::Disable
        | ExprKind::AddressOf(_) => {}
    }
}

fn binary_op_symbol(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Equal => "=",
        BinaryOp::Less => "<",
        BinaryOp::Greater => ">",
        BinaryOp::NotEqual => "<>",
        BinaryOp::LessEqual => "<=",
        BinaryOp::GreaterEqual => ">=",
        BinaryOp::Plus => "+",
        BinaryOp::Minus => "-",
        BinaryOp::Times => "*",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
    }
}

pub fn parse(source: &str) -> Result<Program, String> {
    let tokens = lexer::lex(source)?;
    Parser { tokens, cursor: 0, section: Section::Init, next_label: 0 }.program()
}

struct Parser {
    tokens: Vec<Token>,
    cursor: usize,
    section: Section,
    next_label: usize,
}

impl Parser {
    fn program(mut self) -> Result<Program, String> {
        self.expect_simple(TokenKind::Init)?;
        self.expect_simple(TokenKind::LBrace)?;
        self.section = Section::Init;
        self.next_label = 0;
        let global = match self.sequence(true)? {
            Some(expr) => expr,
            None => self.make_expr(ExprKind::Unit, None),
        };

        let mut handlers = Vec::new();
        while self.at(&TokenKind::Handler) {
            self.bump();
            let location = self.previous_location();
            let id = self.take_integer("handler identifier")?;
            if id < 0 {
                return Err(format!("{}:{}: handler identifier must be non-negative", location.line, location.column));
            }
            self.expect_simple(TokenKind::LBrace)?;
            self.section = Section::Handler(id as usize);
            self.next_label = 0;
            let body = self.sequence(false)?.ok_or_else(|| self.error_here("expected handler body"))?;
            self.expect_simple(TokenKind::RBrace)?;
            handlers.push(Handler { id: id as usize, body });
            self.section = Section::Init;
            self.next_label = 0;
            self.consume_semicolons();
        }
        self.expect_simple(TokenKind::RBrace)?;

        self.expect_simple(TokenKind::Main)?;
        self.expect_simple(TokenKind::LBrace)?;
        self.section = Section::Main;
        self.next_label = 0;
        let main = self.sequence(false)?.ok_or_else(|| self.error_here("expected main body"))?;
        self.expect_simple(TokenKind::RBrace)?;
        self.expect_simple(TokenKind::Eof)?;
        Ok(Program { global, handlers, main })
    }

    fn sequence(&mut self, stop_at_handler: bool) -> Result<Option<Expr>, String> {
        if self.at_sequence_end(stop_at_handler) {
            return Ok(None);
        }

        let mut expressions = vec![self.expression(0)?];
        loop {
            if !self.at(&TokenKind::Semicolon) {
                break;
            }
            self.consume_semicolons();
            if self.at_sequence_end(stop_at_handler) {
                break;
            }
            expressions.push(self.expression(0)?);
        }

        let mut expressions = expressions.into_iter().rev();
        let mut result = expressions.next().expect("sequence contains an expression");
        for first in expressions {
            let line = first.line;
            result = self.make_expr(ExprKind::Sequence(Box::new(first), Box::new(result)), line);
        }
        Ok(Some(result))
    }

    fn at_sequence_end(&self, stop_at_handler: bool) -> bool {
        self.at(&TokenKind::RBrace)
            || self.at(&TokenKind::RParen)
            || (stop_at_handler && self.at(&TokenKind::Handler))
    }

    fn expression(&mut self, min_bp: u8) -> Result<Expr, String> {
        let mut lhs = self.prefix()?;
        loop {
            let Some((op, left_bp, right_bp)) = self.infix_binding() else { break };
            if left_bp < min_bp {
                break;
            }
            self.bump();
            let rhs = self.expression(right_bp)?;
            let line = lhs.line;
            let kind = match op {
                InfixOp::Assign => ExprKind::Assign(Box::new(lhs), Box::new(rhs)),
                InfixOp::Binary(op) => ExprKind::Binary(op, Box::new(lhs), Box::new(rhs)),
            };
            lhs = self.make_expr(kind, line);
        }
        Ok(lhs)
    }

    fn prefix(&mut self) -> Result<Expr, String> {
        let token = self.current().clone();
        let line = Some(token.location.line);
        let kind = match token.kind {
            TokenKind::Unit => { self.bump(); ExprKind::Unit }
            TokenKind::Integer(n) => { self.bump(); ExprKind::Integer(n) }
            TokenKind::Identifier(name) => { self.bump(); ExprKind::Variable(name) }
            TokenKind::Enable => { self.bump(); ExprKind::Enable }
            TokenKind::Disable => { self.bump(); ExprKind::Disable }
            TokenKind::LParen => {
                self.bump();
                let expr = self.sequence(false)?.ok_or_else(|| self.error_here("expected expression"))?;
                self.expect_simple(TokenKind::RParen)?;
                return Ok(expr);
            }
            TokenKind::Ampersand => {
                self.bump();
                let name = self.take_identifier("identifier after '&'")?;
                ExprKind::AddressOf(name)
            }
            TokenKind::Star => {
                self.bump();
                let base = self.expression(7)?;
                self.expect_simple(TokenKind::LBracket)?;
                let index = self.expression(0)?;
                self.expect_simple(TokenKind::RBracket)?;
                ExprKind::Dereference(Box::new(base), Box::new(index))
            }
            TokenKind::Malloc => {
                self.bump();
                self.expect_simple(TokenKind::LParen)?;
                let size = self.expression(0)?;
                self.expect_simple(TokenKind::Comma)?;
                let init = self.expression(0)?;
                self.expect_simple(TokenKind::RParen)?;
                ExprKind::Malloc(Box::new(size), Box::new(init))
            }
            TokenKind::If => {
                self.bump();
                let condition = self.expression(0)?;
                self.expect_simple(TokenKind::Then)?;
                let then_branch = self.expression(0)?;
                self.expect_simple(TokenKind::Else)?;
                let else_branch = self.expression(0)?;
                ExprKind::If(Box::new(condition), Box::new(then_branch), Box::new(else_branch))
            }
            TokenKind::While => {
                self.bump();
                let condition = self.expression(0)?;
                self.expect_simple(TokenKind::Do)?;
                self.expect_simple(TokenKind::LParen)?;
                let body = self.sequence(false)?.ok_or_else(|| self.error_here("expected while body"))?;
                self.expect_simple(TokenKind::RParen)?;
                ExprKind::While(Box::new(condition), Box::new(body))
            }
            _ => return Err(self.error_here("expected expression")),
        };
        Ok(self.make_expr(kind, line))
    }

    fn infix_binding(&self) -> Option<(InfixOp, u8, u8)> {
        let (op, bp, right_assoc) = match &self.current().kind {
            TokenKind::Assign => (InfixOp::Assign, 1, true),
            TokenKind::Or => (InfixOp::Binary(BinaryOp::Or), 2, false),
            TokenKind::And => (InfixOp::Binary(BinaryOp::And), 3, false),
            TokenKind::Equal => (InfixOp::Binary(BinaryOp::Equal), 4, false),
            TokenKind::Less => (InfixOp::Binary(BinaryOp::Less), 4, false),
            TokenKind::Greater => (InfixOp::Binary(BinaryOp::Greater), 4, false),
            TokenKind::NotEqual => (InfixOp::Binary(BinaryOp::NotEqual), 4, false),
            TokenKind::LessEqual => (InfixOp::Binary(BinaryOp::LessEqual), 4, false),
            TokenKind::GreaterEqual => (InfixOp::Binary(BinaryOp::GreaterEqual), 4, false),
            TokenKind::Plus => (InfixOp::Binary(BinaryOp::Plus), 5, false),
            TokenKind::Minus => (InfixOp::Binary(BinaryOp::Minus), 5, false),
            TokenKind::Star => (InfixOp::Binary(BinaryOp::Times), 6, false),
            _ => return None,
        };
        Some((op, bp, if right_assoc { bp } else { bp + 1 }))
    }

    fn make_expr(&mut self, kind: ExprKind, line: Option<usize>) -> Expr {
        let label = Label::At { section: self.section, index: self.next_label };
        self.next_label += 1;
        Expr { label, line, kind }
    }

    fn take_identifier(&mut self, expected: &str) -> Result<String, String> {
        match self.current().kind.clone() {
            TokenKind::Identifier(name) => { self.bump(); Ok(name) }
            _ => Err(self.error_here(&format!("expected {expected}"))),
        }
    }

    fn take_integer(&mut self, expected: &str) -> Result<i64, String> {
        match &self.current().kind {
            TokenKind::Integer(n) => { let n = *n; self.bump(); Ok(n) }
            _ => Err(self.error_here(&format!("expected {expected}"))),
        }
    }

    fn expect_simple(&mut self, expected: TokenKind) -> Result<(), String> {
        if self.at(&expected) {
            self.bump();
            Ok(())
        } else {
            Err(self.error_here(&format!("expected {:?}, found {:?}", expected, self.current().kind)))
        }
    }

    fn at(&self, expected: &TokenKind) -> bool {
        std::mem::discriminant(&self.current().kind) == std::mem::discriminant(expected)
    }

    fn consume_semicolons(&mut self) {
        while self.at(&TokenKind::Semicolon) { self.bump(); }
    }

    fn current(&self) -> &Token {
        self.tokens.get(self.cursor).or_else(|| self.tokens.last()).expect("token stream must not be empty")
    }

    fn bump(&mut self) {
        if !self.at(&TokenKind::Eof) { self.cursor += 1; }
    }

    fn previous_location(&self) -> SourceLocation {
        self.tokens[self.cursor.saturating_sub(1)].location
    }

    fn error_here(&self, message: &str) -> String {
        let loc = self.current().location;
        format!("{}:{}: {}", loc.line, loc.column, message)
    }
}
