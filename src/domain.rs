use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Section {
    Init,
    Main,
    Handler(usize),
}

impl fmt::Display for Section {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Section::Init => write!(f, "init"),
            Section::Main => write!(f, "main"),
            Section::Handler(id) => write!(f, "handler[{id}]"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Label {
    Unit,
    At { section: Section, index: usize },
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Label::Unit => write!(f, "Unit"),
            Label::At { section, index } => write!(f, "{section}#{index}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Unit,
    Integer(i64),
    Loc(Loc),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Unit => write!(f, "Unit"),
            Value::Integer(value) => write!(f, "{value}"),
            Value::Loc(loc) => write!(f, "{loc}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LocId {
    Var(String),
    Label(Label),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Loc {
    pub id: LocId,
    pub offset: usize,
}

impl Loc {
    pub fn new(id: LocId, offset: usize) -> Self {
        Self { id, offset }
    }

    pub fn compare(&self, other: &Self) -> Ordering {
        self.cmp(other)
    }

    pub fn add(&self, n: i64) -> Result<Self, String> {
        let n_usize = usize::try_from(n)
            .map_err(|_| "주소 offset은 음수일 수 없습니다".to_owned())?;
        let offset = self.offset
            .checked_add(n_usize)
            .ok_or_else(|| "주소 offset이 범위를 초과했습니다".to_owned())?;

        Ok(Self {
            id: self.id.clone(),
            offset,
        })
    }
}

impl fmt::Display for LocId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LocId::Var(name) => write!(f, "var({name:?})"),
            LocId::Label(label) => write!(f, "label({label})"),
        }
    }
}

impl fmt::Display for Loc {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}+{}", self.id, self.offset)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Memory {
    cells: HashMap<Loc, (Value, Label)>,
}

impl Memory {
    pub fn insert(&mut self, loc: Loc, value: Value, label: Label) {
        self.cells.insert(loc, (value, label));
    }
    pub fn get(&self, loc: &Loc) -> Option<&(Value, Label)> {
        self.cells.get(loc)
    }
}

impl fmt::Display for Memory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut cells: Vec<_> = self.cells.iter().collect();
        cells.sort_by(|(left, _), (right, _)| left.cmp(right));

        for (loc, (value, label)) in cells {
            writeln!(f, "{loc} -> {value} @ {label}")?;
        };
        Ok(())
    }
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interrupt {
    Interrupt(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IConfig {
    Enable,
    Disable,
}
