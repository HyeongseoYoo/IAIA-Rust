# IAIA Lexer와 Parser 설명

이 문서는 [`src/lexer.rs`](src/lexer.rs)와 [`src/parser.rs`](src/parser.rs)가 `.si` 소스 코드를 토큰과 AST로 바꾸는 과정을 설명합니다. 구현은 각각 기존 OCaml `lexer.mll`과 `parser.mly`의 토큰 및 문법을 Rust로 옮긴 것입니다.

## 전체 처리 흐름

```text
.si 소스 문자열
    │
    ▼
lexer::lex
    │ Vec<Token> (마지막에 Eof 포함)
    ▼
parser::parse_tokens
    │
    ▼
Program { global, handlers, main }
```

일반적으로는 `parser::parse(source)`를 호출합니다. 이 함수가 먼저 `lexer::lex`를 부른 다음, 생성된 토큰을 파서에 전달합니다. 각 단계는 실패할 수 있으며 `Result<_, String>`의 `Err`에 오류 내용과 소스 위치를 반환합니다.

## `src/lexer.rs`

### 토큰과 위치 타입

`SourceLocation`은 토큰이 소스의 어디서 시작됐는지 나타냅니다.

| 필드 | 의미 |
|---|---|
| `offset` | UTF-8 문자열 시작부터의 바이트 위치. 0부터 시작합니다. |
| `line` | 줄 번호. 1부터 시작합니다. |
| `column` | 열 번호. 1부터 시작하며 Unicode 문자 단위로 셉니다. |

`TokenKind`는 파서가 구별하는 문법 단위입니다. `Init`, `If`, `While` 같은 예약어와 괄호·연산자는 각각 별도 변형입니다. 정수와 식별자는 실제 값도 함께 저장합니다.

`Token`은 `kind`와 `location`을 묶습니다. 토큰의 원문 문자열은 별도로 보관하지 않습니다. 식별자 이름과 정수 값은 해당 `TokenKind` 안에서 읽을 수 있습니다.

### `lex(source)` 스캔 순서

함수는 소스를 왼쪽에서 오른쪽으로 한 번 훑으며 토큰을 만듭니다.

1. 공백과 탭을 건너뜁니다. LF, CRLF, CR 줄바꿈을 만나면 줄 번호를 올리고 열을 1로 되돌립니다.
2. `//`부터 줄 끝까지를 건너뜁니다.
3. `(* ... *)` 블록 주석을 건너뜁니다. 주석 안에 다시 `(*`가 나오면 깊이를 올리고 `*)`가 나오면 내립니다. 소스 끝까지 닫히지 않으면 오류를 돌려줍니다.
4. `<=`, `>=`, `<>`, `:=`, `||`, `&&`처럼 두 글자인 연산자를 먼저 확인합니다. 그다음 식별자, 정수, 한 글자 연산자와 구두점을 확인합니다.
5. 소스 끝에서 위치가 기록된 `Eof` 토큰을 추가합니다.

예약어는 식별자 모양의 문자열을 읽은 후 키워드 표와 대조합니다. 예약어가 아니면 `Identifier(String)`이 됩니다. 식별자는 ASCII 영문자 또는 밑줄로 시작하고, 뒤에는 ASCII 영숫자·밑줄·작은따옴표를 포함할 수 있습니다.

정수는 `i64`로 변환합니다. `-` 바로 다음에 숫자가 붙어 있으면 부호까지 정수로 읽습니다. 따라서 `x - 1`처럼 빼기 연산을 쓰려면 `-` 뒤에 공백 등 숫자가 아닌 문자가 있어야 합니다. 이는 원본 lexer의 `'-'? digit+` 규칙을 따릅니다. 정수 범위를 넘거나 문법에서 허용하지 않는 문자가 나오면 오류가 납니다.

`advance`는 한 문자를 소비하면서 바이트 오프셋과 줄·열을 갱신합니다. CRLF는 줄바꿈 하나로 처리합니다. `starts_with`, `is_ident_start`, `is_ident_continue`는 주석 시작과 식별자 규칙 확인을 돕는 내부 함수입니다.

## `src/parser.rs`

### AST 자료형

파서는 소스의 동작을 `Program` 트리로 표현합니다.

- `Program`: 초기화 표현식 `global`, `handlers`, `main` 표현식으로 구성됩니다.
- `Handler`: 정수 `id`와 해당 핸들러의 본문 `body`를 가집니다.
- `Expr`: 표현식 종류 `kind`, 소스 줄 `line`, 내부 구분용 `label`을 가집니다. 빈 init 본문처럼 소스에 대응하는 표현식이 없는 경우 줄은 `None`입니다.
- `ExprKind`: 단위값, 정수, 변수, enable/disable, 이항 연산, 주소 참조, 배열 역참조, 메모리 할당, 대입, 조건문, 반복문, 순차 실행을 나타냅니다.
- `BinaryOp`: `=`, 비교 연산, `+`, `-`, `*`, `&&`, `||`의 종류를 나타냅니다.
- `Section`과 `Label`: 표현식이 init, main, 어느 handler에 속하는지와 그 영역에서 할당된 순번을 저장합니다. 파서는 각 영역을 시작할 때 순번을 0으로 초기화하고, 새 AST 노드를 만들 때마다 올립니다.

하위 표현식이 먼저 파싱된 뒤 부모 표현식이 만들어지므로 이 순번은 트리 생성 순서(자식 우선)를 반영합니다. 괄호는 별도 AST 노드를 만들지 않고 괄호 안의 표현식을 그대로 반환합니다.

### AST pretty print

`pretty_print(&Program) -> String`은 AST를 들여쓰기 트리로 바꿉니다. 프로그램의 `global`, 각 handler, `main`을 구분하고, 각 표현식의 종류·레이블·소스 줄과 자식 역할(`left`, `condition`, `body` 등)을 출력합니다.

```rust
let program = parser::parse(source)?;
println!("{}", parser::pretty_print(&program));
```

예를 들어 `x := 1 + 2`는 대략 다음 구조로 보입니다.

```text
Assign [main:..., line ...]
  left: Variable("x") [main:..., line ...]
  right: Binary(+) [main:..., line ...]
    left: Integer(1) [main:..., line ...]
    right: Integer(2) [main:..., line ...]
```

### 입력 문법과 `program`

상위 구조는 다음과 같습니다.

```text
init {
    초기화 표현식들
    handler 번호 { 핸들러 표현식들 }
}
main {
    메인 표현식들
}
```

`program`은 `init {`을 확인한 뒤 `sequence(true)`로 init 표현식을 읽습니다. 이 모드는 `handler` 토큰을 만나면 초기화 표현식 읽기를 멈춥니다. 핸들러는 `handler` 다음의 정수 ID와 중괄호 본문으로 읽으며, 핸들러 ID가 음수면 오류입니다. 이후 `main` 블록을 읽고 입력 끝의 `Eof`까지 확인합니다. 빈 init 블록은 줄이 없는 `Unit`으로 표현합니다.

### 순차 표현식 `sequence`

세미콜론으로 이어진 식들을 읽은 뒤 오른쪽부터 `Sequence(left, right)`로 묶습니다. 따라서 `e1; e2; e3`는 `Sequence(e1, Sequence(e2, e3))`가 됩니다. 마지막 세미콜론은 허용합니다. init 블록에서는 핸들러 선언 직전에도 세미콜론이 올 수 있습니다. 핸들러 본문·main 본문·괄호 안에서는 닫는 괄호나 중괄호까지 읽습니다.

### 표현식과 연산자 우선순위

`expression`과 `prefix`는 우선순위 클라이밍 방식으로 표현식을 파싱합니다. `prefix`가 원자식과 특수 문법을 읽고, `expression`이 현재 연산자의 우선순위에 따라 오른쪽 피연산자를 읽어 AST 노드를 만듭니다.

| 우선순위 (낮음 → 높음) | 연산자 | 결합 방향 |
|---:|---|---|
| 1 | `:=` | 오른쪽 |
| 2 | `||` | 왼쪽 |
| 3 | `&&` | 왼쪽 |
| 4 | `=`, `<`, `>`, `<>`, `<=`, `>=` | 왼쪽 |
| 5 | `+`, `-` | 왼쪽 |
| 6 | `*` | 왼쪽 |

이 표는 원본 parser의 precedence 선언 순서를 따릅니다. 왼쪽 결합 연산자는 오른쪽 피연산자를 현재 우선순위보다 한 단계 높게 파싱합니다. 대입은 같은 우선순위로 오른쪽을 읽어 오른쪽 결합을 만듭니다.

`prefix`에서 처리하는 나머지 문법은 다음과 같습니다.

- `unit`, 정수, 식별자, `enable`, `disable`: 각각 대응하는 원자 AST를 만듭니다.
- `( ... )`: 괄호 내부의 세미콜론 순차식을 읽고 그 AST를 반환합니다.
- `&name`: `AddressOf(name)`을 만듭니다.
- `*base[index]`: `Dereference(base, index)`를 만듭니다.
- `malloc(size, initial)`: 인자 두 개를 읽어 `Malloc`을 만듭니다.
- `if condition then expr else expr`: 조건과 두 분기를 읽어 `If`를 만듭니다.
- `while condition do (body)`: 조건과 괄호로 감싼 순차 본문을 읽어 `While`을 만듭니다.

`then`, `else`, `do`, 쉼표, 닫는 괄호처럼 현재 위치에서 이항 연산자가 아닌 토큰을 만나면 표현식 파싱이 멈춥니다. 호출자가 해당 구분 토큰이 있는지 확인하고 잘못된 문법이면 현재 토큰의 줄·열을 포함해 오류를 반환합니다.

### 보조 함수와 호출 방법

- `make_expr`는 현재 영역의 다음 `Label`을 할당해 표현식을 만듭니다.
- `take_identifier`, `take_integer`는 원하는 타입의 토큰을 읽고, 타입이 다르면 오류를 만듭니다.
- `expect_simple`는 괄호나 키워드 등 특정 토큰이 나오는지 확인합니다.
- `at`, `bump`, `current`는 토큰 스트림의 현재 위치를 관리합니다.
- `error_here`는 현재 토큰 위치를 오류 문자열에 붙입니다.
소스 문자열을 파싱할 때는 다음 함수를 사용합니다.

```rust
let program = parser::parse(source)?;
```

`parse`는 내부에서 `lexer::lex`를 호출하므로 lexer만 별도로 사용할 때는 `lexer::lex(source)`를 호출하면 됩니다.
