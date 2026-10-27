#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLocation {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    Init,
    Main,
    Handler,
    Enable,
    Disable,
    Unit,
    If,
    Then,
    Else,
    While,
    Do,
    Malloc,
    Integer(i64),
    Identifier(String),
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Semicolon,
    Assign,
    Or,
    And,
    Equal,
    Less,
    Greater,
    NotEqual,
    LessEqual,
    GreaterEqual,
    Minus,
    Plus,
    Star,
    Ampersand,
    Eof,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub location: SourceLocation,
}

pub fn lex(source: &str) -> Result<Vec<Token>, String> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let (mut offset, mut line, mut column) = (0, 1, 1);

    while offset < bytes.len() {
        let b = bytes[offset];
        if b == b' ' || b == b'\t' || b == b'\r' || b == b'\n' {
            advance(source, &mut offset, &mut line, &mut column);
            continue;
        }
        if starts_with(bytes, offset, b"//") {
            offset += 2;
            column += 2;
            while offset < bytes.len() && bytes[offset] != b'\r' && bytes[offset] != b'\n' {
                advance(source, &mut offset, &mut line, &mut column);
            }
            continue;
        }
        if starts_with(bytes, offset, b"(*") {
            let start = SourceLocation { offset, line, column };
            offset += 2;
            column += 2;
            let mut depth = 1usize;
            while offset < bytes.len() && depth > 0 {
                if starts_with(bytes, offset, b"(*") {
                    depth += 1;
                    offset += 2;
                    column += 2;
                } else if starts_with(bytes, offset, b"*)") {
                    depth -= 1;
                    offset += 2;
                    column += 2;
                } else {
                    advance(source, &mut offset, &mut line, &mut column);
                }
            }
            if depth != 0 {
                return Err(format!("{}:{}: unterminated block comment", start.line, start.column));
            }
            continue;
        }

        let location = SourceLocation { offset, line, column };
        let rest = &bytes[offset..];
        let multi = [
            (b"<=".as_slice(), TokenKind::LessEqual),
            (b">=".as_slice(), TokenKind::GreaterEqual),
            (b"<>".as_slice(), TokenKind::NotEqual),
            (b":=".as_slice(), TokenKind::Assign),
            (b"||".as_slice(), TokenKind::Or),
            (b"&&".as_slice(), TokenKind::And),
        ];
        if let Some((text, kind)) = multi.iter().find(|(text, _)| rest.starts_with(text)) {
            tokens.push(Token { kind: kind.clone(), location });
            offset += text.len();
            column += text.len();
            continue;
        }

        if is_ident_start(b) {
            let start = offset;
            advance(source, &mut offset, &mut line, &mut column);
            while offset < bytes.len() && is_ident_continue(bytes[offset]) {
                advance(source, &mut offset, &mut line, &mut column);
            }
            let word = &source[start..offset];
            let kind = match word {
                "init" => TokenKind::Init,
                "main" => TokenKind::Main,
                "handler" => TokenKind::Handler,
                "enable" => TokenKind::Enable,
                "disable" => TokenKind::Disable,
                "unit" => TokenKind::Unit,
                "if" => TokenKind::If,
                "then" => TokenKind::Then,
                "else" => TokenKind::Else,
                "while" => TokenKind::While,
                "do" => TokenKind::Do,
                "malloc" => TokenKind::Malloc,
                _ => TokenKind::Identifier(word.to_owned()),
            };
            tokens.push(Token { kind, location });
            continue;
        }

        if b.is_ascii_digit() || (b == b'-' && offset + 1 < bytes.len() && bytes[offset + 1].is_ascii_digit()) {
            let start = offset;
            if b == b'-' {
                advance(source, &mut offset, &mut line, &mut column);
            }
            while offset < bytes.len() && bytes[offset].is_ascii_digit() {
                advance(source, &mut offset, &mut line, &mut column);
            }
            let value = source[start..offset]
                .parse::<i64>()
                .map_err(|_| format!("{}:{}: integer is out of range", location.line, location.column))?;
            tokens.push(Token { kind: TokenKind::Integer(value), location });
            continue;
        }

        let kind = match b {
            b'=' => TokenKind::Equal,
            b'<' => TokenKind::Less,
            b'>' => TokenKind::Greater,
            b'+' => TokenKind::Plus,
            b'-' => TokenKind::Minus,
            b'*' => TokenKind::Star,
            b'{' => TokenKind::LBrace,
            b'}' => TokenKind::RBrace,
            b'(' => TokenKind::LParen,
            b')' => TokenKind::RParen,
            b'[' => TokenKind::LBracket,
            b']' => TokenKind::RBracket,
            b',' => TokenKind::Comma,
            b'&' => TokenKind::Ampersand,
            b';' => TokenKind::Semicolon,
            _ => return Err(format!("{}:{}: unexpected character {:?}", line, column, source[offset..].chars().next().unwrap())),
        };
        tokens.push(Token { kind, location });
        advance(source, &mut offset, &mut line, &mut column);
    }

    tokens.push(Token {
        kind: TokenKind::Eof,
        location: SourceLocation { offset, line, column },
    });
    Ok(tokens)
}

fn starts_with(source: &[u8], offset: usize, prefix: &[u8]) -> bool {
    source[offset..].starts_with(prefix)
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn is_ident_continue(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'\''
}

fn advance(source: &str, offset: &mut usize, line: &mut usize, column: &mut usize) {
    let bytes = source.as_bytes();
    if bytes[*offset] == b'\r' {
        *offset += 1;
        if *offset < bytes.len() && bytes[*offset] == b'\n' {
            *offset += 1;
        }
        *line += 1;
        *column = 1;
    } else if bytes[*offset] == b'\n' {
        *offset += 1;
        *line += 1;
        *column = 1;
    } else {
        let ch = source[*offset..].chars().next().unwrap();
        *offset += ch.len_utf8();
        *column += 1;
    }
}
