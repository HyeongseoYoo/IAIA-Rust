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
    Heap { label: Label, allocation_id: u64 },
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
            LocId::Heap { label, allocation_id } => {
                write!(f, "heap({label}, alloc#{allocation_id})")
            }
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
    allocation_sizes: HashMap<u64, usize>,
    next_allocation_id: u64,
}

impl Memory {
    pub fn insert(&mut self, loc: Loc, value: Value, label: Label) {
        self.cells.insert(loc, (value, label));
    }
    pub fn get(&self, loc: &Loc) -> Option<&(Value, Label)> {
        self.cells.get(loc)
    }

    pub fn allocate(&mut self, label: Label, size: usize, initial_value: Value) -> Result<Loc, String> {
        if size == 0 {
            return Err("Malloc 크기는 0보다 커야 합니다".to_owned());
        }

        let allocation_id = self.next_allocation_id;
        self.next_allocation_id = allocation_id
            .checked_add(1)
            .ok_or_else(|| "할당 ID가 범위를 초과했습니다".to_owned())?;
        self.allocation_sizes.insert(allocation_id, size);

        let id = LocId::Heap { label, allocation_id };
        for offset in 0..size {
            self.insert(Loc::new(id.clone(), offset), initial_value.clone(), label);
        }

        Ok(Loc::new(id, 0))
    }

    pub fn check_deref(&self, loc: &Loc) -> Result<(), String> {
        match &loc.id {
            LocId::Var(_) if loc.offset == 0 => Ok(()),
            LocId::Var(_) => Err(format!("변수 주소 범위를 벗어났습니다: {loc}")),
            LocId::Heap { allocation_id, .. } => match self.allocation_sizes.get(allocation_id) {
                Some(size) if loc.offset < *size => Ok(()),
                Some(size) => Err(format!(
                    "Heap 범위를 벗어난 주소입니다: {loc} (크기: {size})"
                )),
                None => Err(format!("존재하지 않는 Heap 할당입니다: {loc}")),
            },
        }
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
