use crate::diag::{Diagnostic, Severity};
use crate::FailureClass;
use std::collections::{BTreeMap, BTreeSet};

pub const ORIGIN: &str = "compiler.infrastructure";

pub const FORMAT_VERSION: u32 = 1;

pub fn fail(code: &str, message: &str) -> Diagnostic {
    Diagnostic::new(
        code,
        Severity::Error,
        FailureClass::InternalError,
        ORIGIN,
        message,
    )
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Scalar {
    Bool,
    I8,
    I16,
    I32,
    I64,
    I128,
    F32,
    F64,
    Pointer,
    Reference,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Type {
    Scalar(Scalar),
    Vector { lanes: u8, element: Scalar },
}

impl Scalar {
    pub const fn bits(self) -> u32 {
        match self {
            Scalar::Bool => 1,
            Scalar::I8 => 8,
            Scalar::I16 => 16,
            Scalar::I32 | Scalar::F32 => 32,
            Scalar::I64 | Scalar::F64 => 64,
            Scalar::I128 => 128,
            Scalar::Pointer | Scalar::Reference => 64,
        }
    }

    pub const fn is_integer(self) -> bool {
        matches!(
            self,
            Scalar::Bool | Scalar::I8 | Scalar::I16 | Scalar::I32 | Scalar::I64 | Scalar::I128
        )
    }

    pub const fn is_float(self) -> bool {
        matches!(self, Scalar::F32 | Scalar::F64)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Scalar::Bool => "bool",
            Scalar::I8 => "i8",
            Scalar::I16 => "i16",
            Scalar::I32 => "i32",
            Scalar::I64 => "i64",
            Scalar::I128 => "i128",
            Scalar::F32 => "f32",
            Scalar::F64 => "f64",
            Scalar::Pointer => "ptr",
            Scalar::Reference => "ref",
        }
    }

    pub fn named(text: &str) -> Option<Scalar> {
        Some(match text {
            "bool" => Scalar::Bool,
            "i8" => Scalar::I8,
            "i16" => Scalar::I16,
            "i32" => Scalar::I32,
            "i64" => Scalar::I64,
            "i128" => Scalar::I128,
            "f32" => Scalar::F32,
            "f64" => Scalar::F64,
            "ptr" => Scalar::Pointer,
            "ref" => Scalar::Reference,
            _ => return None,
        })
    }

    const fn code(self) -> u8 {
        match self {
            Scalar::Bool => 1,
            Scalar::I8 => 2,
            Scalar::I16 => 3,
            Scalar::I32 => 4,
            Scalar::I64 => 5,
            Scalar::I128 => 6,
            Scalar::F32 => 7,
            Scalar::F64 => 8,
            Scalar::Pointer => 9,
            Scalar::Reference => 10,
        }
    }

    fn from_code(code: u8) -> Option<Scalar> {
        Some(match code {
            1 => Scalar::Bool,
            2 => Scalar::I8,
            3 => Scalar::I16,
            4 => Scalar::I32,
            5 => Scalar::I64,
            6 => Scalar::I128,
            7 => Scalar::F32,
            8 => Scalar::F64,
            9 => Scalar::Pointer,
            10 => Scalar::Reference,
            _ => return None,
        })
    }
}

impl Type {
    pub const BOOL: Type = Type::Scalar(Scalar::Bool);
    pub const I8: Type = Type::Scalar(Scalar::I8);
    pub const I16: Type = Type::Scalar(Scalar::I16);
    pub const I32: Type = Type::Scalar(Scalar::I32);
    pub const I64: Type = Type::Scalar(Scalar::I64);
    pub const I128: Type = Type::Scalar(Scalar::I128);
    pub const F32: Type = Type::Scalar(Scalar::F32);
    pub const F64: Type = Type::Scalar(Scalar::F64);
    pub const POINTER: Type = Type::Scalar(Scalar::Pointer);
    pub const REFERENCE: Type = Type::Scalar(Scalar::Reference);

    pub fn element(self) -> Scalar {
        match self {
            Type::Scalar(scalar) => scalar,
            Type::Vector { element, .. } => element,
        }
    }

    pub fn lanes(self) -> u32 {
        match self {
            Type::Scalar(_) => 1,
            Type::Vector { lanes, .. } => u32::from(lanes),
        }
    }

    pub fn is_integer(self) -> bool {
        self.element().is_integer()
    }

    pub fn is_float(self) -> bool {
        self.element().is_float()
    }

    pub fn is_scalar_integer(self) -> bool {
        matches!(self, Type::Scalar(scalar) if scalar.is_integer())
    }

    pub fn is_scalar_float(self) -> bool {
        matches!(self, Type::Scalar(scalar) if scalar.is_float())
    }

    pub fn text(self) -> String {
        match self {
            Type::Scalar(scalar) => scalar.name().to_string(),
            Type::Vector { lanes, element } => format!("<{lanes} x {}>", element.name()),
        }
    }

    pub fn parse(text: &str) -> Option<Type> {
        if let Some(inner) = text
            .strip_prefix('<')
            .and_then(|rest| rest.strip_suffix('>'))
        {
            let (lanes, element) = inner.split_once(" x ")?;
            let lanes: u8 = lanes.trim().parse().ok()?;
            let element = Scalar::named(element.trim())?;
            return Type::vector(lanes, element);
        }
        Scalar::named(text).map(Type::Scalar)
    }

    pub fn vector(lanes: u8, element: Scalar) -> Option<Type> {
        let fits = element.is_float() || (element.is_integer() && element != Scalar::Bool);
        (lanes >= 2 && fits).then_some(Type::Vector { lanes, element })
    }
}

macro_rules! entity {
    ($name:ident) => {
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
        pub struct $name(pub u32);

        impl $name {
            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

entity!(Value);
entity!(Block);
entity!(Inst);
entity!(FunctionId);
entity!(GlobalId);
entity!(StructId);
entity!(ClassId);
entity!(Variable);

macro_rules! named_enum {
    ($(#[$outer:meta])* $name:ident { $($(#[$inner:meta])* $variant:ident = $text:literal),* $(,)? }) => {
        $(#[$outer])*
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
        pub enum $name {
            $($(#[$inner])* $variant),*
        }

        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$variant),*];

            pub const fn name(self) -> &'static str {
                match self {
                    $($name::$variant => $text),*
                }
            }

            pub fn named(text: &str) -> Option<$name> {
                $name::ALL.iter().copied().find(|item| item.name() == text)
            }

            pub fn code(self) -> u8 {
                $name::ALL.iter().position(|item| *item == self).unwrap_or(0) as u8
            }

            pub fn from_code(code: u8) -> Option<$name> {
                $name::ALL.get(usize::from(code)).copied()
            }
        }
    };
}

named_enum!(Ordering {
    Relaxed = "relaxed",
    Acquire = "acquire",
    Release = "release",
    AcquireRelease = "acq_rel",
    SequentiallyConsistent = "seq_cst",
});

impl Ordering {
    pub const fn strength(self) -> u8 {
        match self {
            Ordering::Relaxed => 0,
            Ordering::Acquire | Ordering::Release => 1,
            Ordering::AcquireRelease => 2,
            Ordering::SequentiallyConsistent => 3,
        }
    }
}

named_enum!(BinaryOp {
    Add = "add",
    Subtract = "sub",
    Multiply = "mul",
    DivideSigned = "sdiv",
    DivideUnsigned = "udiv",
    RemainderSigned = "srem",
    RemainderUnsigned = "urem",
    And = "and",
    Or = "or",
    Xor = "xor",
    ShiftLeft = "shl",
    ShiftRightLogical = "lshr",
    ShiftRightArithmetic = "ashr",
    RotateLeft = "rotl",
    RotateRight = "rotr",
    MinimumSigned = "smin",
    MaximumSigned = "smax",
    MinimumUnsigned = "umin",
    MaximumUnsigned = "umax",
    AddSaturatingSigned = "sadd_sat",
    AddSaturatingUnsigned = "uadd_sat",
    SubtractSaturatingSigned = "ssub_sat",
    SubtractSaturatingUnsigned = "usub_sat",
    AddOverflowsSigned = "sadd_overflows",
    AddOverflowsUnsigned = "uadd_overflows",
    SubtractOverflowsSigned = "ssub_overflows",
    SubtractOverflowsUnsigned = "usub_overflows",
    MultiplyOverflowsSigned = "smul_overflows",
    MultiplyOverflowsUnsigned = "umul_overflows",
    MultiplyHighSigned = "smulh",
    MultiplyHighUnsigned = "umulh",
    FloatAdd = "fadd",
    FloatSubtract = "fsub",
    FloatMultiply = "fmul",
    FloatDivide = "fdiv",
    FloatRemainder = "frem",
    FloatMinimum = "fmin",
    FloatMaximum = "fmax",
    FloatMinimumNumber = "fminnum",
    FloatMaximumNumber = "fmaxnum",
    CopySign = "copysign",
});

impl BinaryOp {
    pub fn is_float(self) -> bool {
        matches!(
            self,
            BinaryOp::FloatAdd
                | BinaryOp::FloatSubtract
                | BinaryOp::FloatMultiply
                | BinaryOp::FloatDivide
                | BinaryOp::FloatRemainder
                | BinaryOp::FloatMinimum
                | BinaryOp::FloatMaximum
                | BinaryOp::FloatMinimumNumber
                | BinaryOp::FloatMaximumNumber
                | BinaryOp::CopySign
        )
    }

    pub fn yields_bool(self) -> bool {
        matches!(
            self,
            BinaryOp::AddOverflowsSigned
                | BinaryOp::AddOverflowsUnsigned
                | BinaryOp::SubtractOverflowsSigned
                | BinaryOp::SubtractOverflowsUnsigned
                | BinaryOp::MultiplyOverflowsSigned
                | BinaryOp::MultiplyOverflowsUnsigned
        )
    }
}

named_enum!(UnaryOp {
    Negate = "neg",
    Not = "not",
    PopulationCount = "popcount",
    LeadingZeros = "clz",
    TrailingZeros = "ctz",
    ByteSwap = "bswap",
    BitReverse = "bitreverse",
    AbsoluteSigned = "abs",
    FloatNegate = "fneg",
    FloatAbsolute = "fabs",
    SquareRoot = "sqrt",
    Floor = "floor",
    Ceiling = "ceil",
    TruncateFloat = "ftrunc",
    RoundNearestEven = "nearest",
});

impl UnaryOp {
    pub fn is_float(self) -> bool {
        matches!(
            self,
            UnaryOp::FloatNegate
                | UnaryOp::FloatAbsolute
                | UnaryOp::SquareRoot
                | UnaryOp::Floor
                | UnaryOp::Ceiling
                | UnaryOp::TruncateFloat
                | UnaryOp::RoundNearestEven
        )
    }
}

named_enum!(Comparison {
    Equal = "eq",
    NotEqual = "ne",
    LessSigned = "slt",
    LessEqualSigned = "sle",
    GreaterSigned = "sgt",
    GreaterEqualSigned = "sge",
    LessUnsigned = "ult",
    LessEqualUnsigned = "ule",
    GreaterUnsigned = "ugt",
    GreaterEqualUnsigned = "uge",
});

named_enum!(FloatComparison {
    OrderedEqual = "oeq",
    OrderedNotEqual = "one",
    OrderedLess = "olt",
    OrderedLessEqual = "ole",
    OrderedGreater = "ogt",
    OrderedGreaterEqual = "oge",
    Ordered = "ord",
    Unordered = "uno",
    UnorderedEqual = "ueq",
    UnorderedNotEqual = "une",
    UnorderedLess = "ult",
    UnorderedLessEqual = "ule",
    UnorderedGreater = "ugt",
    UnorderedGreaterEqual = "uge",
});

named_enum!(Conversion {
    ZeroExtend = "zext",
    SignExtend = "sext",
    Truncate = "trunc",
    FloatExtend = "fpext",
    FloatTruncate = "fptrunc",
    SignedToFloat = "sitofp",
    UnsignedToFloat = "uitofp",
    FloatToSigned = "fptosi",
    FloatToUnsigned = "fptoui",
    FloatToSignedSaturating = "fptosi_sat",
    FloatToUnsignedSaturating = "fptoui_sat",
    Bitcast = "bitcast",
    PointerToInteger = "ptrtoint",
    IntegerToPointer = "inttoptr",
});

named_enum!(AtomicOp {
    Exchange = "xchg",
    Add = "add",
    Subtract = "sub",
    And = "and",
    Nand = "nand",
    Or = "or",
    Xor = "xor",
    MinimumSigned = "smin",
    MaximumSigned = "smax",
    MinimumUnsigned = "umin",
    MaximumUnsigned = "umax",
});

named_enum!(Convention {
    Default = "default",
    C = "c",
    System = "system",
    Fast = "fast",
    Cold = "cold",
});

named_enum!(Linkage {
    Export = "export",
    Internal = "internal",
    Import = "import",
    Weak = "weak",
});

named_enum!(Attribute {
    NoReturn = "noreturn",
    NoUnwind = "nounwind",
    ReadOnly = "readonly",
    Cold = "cold",
    Inline = "inline",
    NoInline = "noinline",
    Entry = "entry",
    StackProtect = "ssp",
});

#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct Signature {
    pub parameters: Vec<Type>,
    pub result: Option<Type>,
    pub convention: Option<Convention>,
    pub variadic: bool,
}

impl Signature {
    pub fn new(parameters: Vec<Type>, result: Option<Type>) -> Signature {
        Signature {
            parameters,
            result,
            convention: None,
            variadic: false,
        }
    }

    pub fn convention(&self) -> Convention {
        self.convention.unwrap_or(Convention::Default)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Operation {
    IntegerConstant(i128),
    FloatConstant(u64),
    Null,
    Poison,
    GlobalAddress(GlobalId),
    FunctionAddress(FunctionId),
    StringConstant(u32),
    Binary(BinaryOp),
    Unary(UnaryOp),
    Compare(Comparison),
    FloatCompare(FloatComparison),
    Convert(Conversion),
    Select,
    Splat,
    ExtractLane(u8),
    InsertLane(u8),
    StackSlot {
        size: u32,
        align: u32,
    },
    DynamicStackSlot {
        align: u32,
    },
    StackSave,
    StackRestore,
    VaStart,
    VaArg,
    VaCopy,
    Load {
        align: u32,
        volatile: bool,
        ordering: Option<Ordering>,
    },
    Store {
        align: u32,
        volatile: bool,
        ordering: Option<Ordering>,
    },
    Offset,
    FieldAddress(StructId, u32),
    ElementAddress(Type),
    AtomicUpdate(AtomicOp, Ordering),
    CompareExchange(Ordering, Ordering),
    Fence(Ordering),
    MemoryCopy,
    MemoryMove,
    MemoryFill,
    Call(FunctionId),
    CallIndirect(Signature),
    Allocate(ClassId),
    AllocateArray(ClassId),
    LoadField(ClassId, u32),
    StoreField(ClassId, u32),
    LoadElement(Type),
    StoreElement(Type),
    ArrayLength,
    InstanceOf(ClassId),
    VirtualMethod(ClassId, u32),
    InterfaceMethod(ClassId, u32),
    SameObject,
    Trap(u32),
}

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug, Default)]
pub struct SourceSpot {
    pub file: u32,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Instruction {
    pub operation: Operation,
    pub arguments: Vec<Value>,
    pub result: Option<Value>,
    pub spot: Option<SourceSpot>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BlockCall {
    pub block: Block,
    pub arguments: Vec<Value>,
}

impl BlockCall {
    pub fn to(block: Block) -> BlockCall {
        BlockCall {
            block,
            arguments: Vec::new(),
        }
    }

    pub fn with(block: Block, arguments: Vec<Value>) -> BlockCall {
        BlockCall { block, arguments }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Terminator {
    Jump(BlockCall),
    Branch {
        condition: Value,
        taken: BlockCall,
        otherwise: BlockCall,
    },
    Switch {
        value: Value,
        cases: Vec<(i128, BlockCall)>,
        default: BlockCall,
    },
    Return(Option<Value>),
    Unreachable,
    Throw(Value),
    Invoke {
        call: Instruction,
        normal: BlockCall,
        unwind: BlockCall,
    },
}

impl Terminator {
    pub fn successors(&self) -> Vec<&BlockCall> {
        match self {
            Terminator::Jump(call) => vec![call],
            Terminator::Branch {
                taken, otherwise, ..
            } => vec![taken, otherwise],
            Terminator::Switch { cases, default, .. } => {
                let mut out: Vec<&BlockCall> = cases.iter().map(|(_, call)| call).collect();
                out.push(default);
                out
            }
            Terminator::Invoke { normal, unwind, .. } => vec![normal, unwind],
            Terminator::Return(_) | Terminator::Unreachable | Terminator::Throw(_) => Vec::new(),
        }
    }

    pub fn successors_mut(&mut self) -> Vec<&mut BlockCall> {
        match self {
            Terminator::Jump(call) => vec![call],
            Terminator::Branch {
                taken, otherwise, ..
            } => vec![taken, otherwise],
            Terminator::Switch { cases, default, .. } => {
                let mut out: Vec<&mut BlockCall> = cases.iter_mut().map(|(_, call)| call).collect();
                out.push(default);
                out
            }
            Terminator::Invoke { normal, unwind, .. } => vec![normal, unwind],
            Terminator::Return(_) | Terminator::Unreachable | Terminator::Throw(_) => Vec::new(),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct BlockData {
    pub parameters: Vec<Value>,
    pub instructions: Vec<Inst>,
    pub terminator: Terminator,
}

#[derive(Clone, PartialEq, Debug)]
pub struct Function {
    pub name: String,
    pub signature: Signature,
    pub linkage: Linkage,
    pub attributes: BTreeSet<Attribute>,
    pub value_types: Vec<Type>,
    pub instructions: Vec<Instruction>,
    pub blocks: Vec<BlockData>,
}

impl Function {
    pub fn new(name: &str, signature: Signature, linkage: Linkage) -> Function {
        Function {
            name: name.to_string(),
            signature,
            linkage,
            attributes: BTreeSet::new(),
            value_types: Vec::new(),
            instructions: Vec::new(),
            blocks: Vec::new(),
        }
    }

    pub fn is_declaration(&self) -> bool {
        self.blocks.is_empty()
    }

    pub fn new_value(&mut self, ty: Type) -> Value {
        self.value_types.push(ty);
        Value(self.value_types.len() as u32 - 1)
    }

    pub fn type_of(&self, value: Value) -> Type {
        self.value_types[value.index()]
    }

    pub fn block(&self, block: Block) -> &BlockData {
        &self.blocks[block.index()]
    }

    pub fn instruction(&self, inst: Inst) -> &Instruction {
        &self.instructions[inst.index()]
    }

    pub fn entry(&self) -> Option<Block> {
        if self.blocks.is_empty() {
            None
        } else {
            Some(Block(0))
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Relocation {
    pub offset: u32,
    pub target: Symbol,
    pub addend: i64,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Symbol {
    Global(GlobalId),
    Function(FunctionId),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GlobalContent {
    Zero,
    Bytes(Vec<u8>),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Global {
    pub name: String,
    pub size: u32,
    pub align: u32,
    pub mutable: bool,
    pub thread_local: bool,
    pub linkage: Linkage,
    pub content: GlobalContent,
    pub relocations: Vec<Relocation>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct StructType {
    pub name: String,
    pub fields: Vec<Type>,
    pub packed: bool,
}

#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct ClassType {
    pub name: String,
    pub parent: Option<ClassId>,
    pub interface: bool,
    pub abstract_class: bool,
    pub element: Option<Type>,
    pub fields: Vec<Type>,
    pub methods: Vec<Option<FunctionId>>,
    pub implementations: Vec<(ClassId, Vec<Option<FunctionId>>)>,
}

named_enum!(#[derive(Default)] Layout {
    #[default]
    Lp64 = "lp64",
    Ilp32 = "ilp32",
    I386 = "i386",
});

impl Layout {
    pub const fn data(self) -> DataLayout {
        match self {
            Layout::Lp64 => DataLayout::LP64,
            Layout::Ilp32 => DataLayout::ILP32,
            Layout::I386 => DataLayout::I386,
        }
    }
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Module {
    pub name: String,
    pub layout: Layout,
    pub files: Vec<String>,
    pub strings: Vec<String>,
    pub structs: Vec<StructType>,
    pub classes: Vec<ClassType>,
    pub globals: Vec<Global>,
    pub functions: Vec<Function>,
}

impl Module {
    pub fn new(name: &str) -> Module {
        Module {
            name: name.to_string(),
            ..Module::default()
        }
    }

    pub fn add_function(&mut self, function: Function) -> FunctionId {
        self.functions.push(function);
        FunctionId(self.functions.len() as u32 - 1)
    }

    pub fn add_global(&mut self, global: Global) -> GlobalId {
        self.globals.push(global);
        GlobalId(self.globals.len() as u32 - 1)
    }

    pub fn add_struct(&mut self, structure: StructType) -> StructId {
        self.structs.push(structure);
        StructId(self.structs.len() as u32 - 1)
    }

    pub fn add_class(&mut self, class: ClassType) -> ClassId {
        self.classes.push(class);
        ClassId(self.classes.len() as u32 - 1)
    }

    pub fn intern(&mut self, text: &str) -> u32 {
        if let Some(found) = self.strings.iter().position(|held| held == text) {
            return found as u32;
        }
        self.strings.push(text.to_string());
        self.strings.len() as u32 - 1
    }

    pub fn function_named(&self, name: &str) -> Option<FunctionId> {
        self.functions
            .iter()
            .position(|function| function.name == name)
            .map(|at| FunctionId(at as u32))
    }

    pub fn global_named(&self, name: &str) -> Option<GlobalId> {
        self.globals
            .iter()
            .position(|global| global.name == name)
            .map(|at| GlobalId(at as u32))
    }

    pub fn class_named(&self, name: &str) -> Option<ClassId> {
        self.classes
            .iter()
            .position(|class| class.name == name)
            .map(|at| ClassId(at as u32))
    }

    pub fn struct_named(&self, name: &str) -> Option<StructId> {
        self.structs
            .iter()
            .position(|structure| structure.name == name)
            .map(|at| StructId(at as u32))
    }

    pub fn is_subclass(&self, class: ClassId, ancestor: ClassId) -> bool {
        let mut pending = vec![class];
        let mut visited = BTreeSet::new();
        while let Some(at) = pending.pop() {
            if at == ancestor {
                return true;
            }
            if !visited.insert(at) {
                continue;
            }
            let Some(data) = self.classes.get(at.index()) else {
                continue;
            };
            pending.extend(data.implementations.iter().map(|(interface, _)| *interface));
            pending.extend(data.parent);
        }
        false
    }

    pub fn field_types(&self, class: ClassId) -> Vec<Type> {
        let mut chain = Vec::new();
        let mut current = Some(class);
        while let Some(at) = current {
            let Some(data) = self.classes.get(at.index()) else {
                break;
            };
            if chain.contains(&at) {
                break;
            }
            chain.push(at);
            current = data.parent;
        }
        let mut fields = Vec::new();
        for at in chain.into_iter().rev() {
            fields.extend(self.classes[at.index()].fields.iter().copied());
        }
        fields
    }

    pub fn field_offset(&self, class: ClassId) -> usize {
        match self.classes.get(class.index()).and_then(|data| data.parent) {
            Some(parent) => self.field_types(parent).len(),
            None => 0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct DataLayout {
    pub pointer_bytes: u32,
    pub big_endian: bool,
    pub i64_align: u32,
    pub f64_align: u32,
    pub i128_align: u32,
}

impl DataLayout {
    pub const LP64: DataLayout = DataLayout {
        pointer_bytes: 8,
        big_endian: false,
        i64_align: 8,
        f64_align: 8,
        i128_align: 16,
    };

    pub const ILP32: DataLayout = DataLayout {
        pointer_bytes: 4,
        big_endian: false,
        i64_align: 8,
        f64_align: 8,
        i128_align: 16,
    };

    pub const I386: DataLayout = DataLayout {
        pointer_bytes: 4,
        big_endian: false,
        i64_align: 4,
        f64_align: 4,
        i128_align: 4,
    };

    pub fn size_of(&self, ty: Type) -> u32 {
        match ty {
            Type::Scalar(scalar) => self.scalar_size(scalar),
            Type::Vector { lanes, element } => self.scalar_size(element) * u32::from(lanes),
        }
    }

    pub fn align_of(&self, ty: Type) -> u32 {
        match ty {
            Type::Scalar(scalar) => match scalar {
                Scalar::I64 => self.i64_align,
                Scalar::F64 => self.f64_align,
                Scalar::I128 => self.i128_align,
                other => self.scalar_size(other),
            },
            Type::Vector { .. } => self.size_of(ty).min(16),
        }
    }

    fn scalar_size(&self, scalar: Scalar) -> u32 {
        match scalar {
            Scalar::Bool | Scalar::I8 => 1,
            Scalar::I16 => 2,
            Scalar::I32 | Scalar::F32 => 4,
            Scalar::I64 | Scalar::F64 => 8,
            Scalar::I128 => 16,
            Scalar::Pointer | Scalar::Reference => self.pointer_bytes,
        }
    }

    pub fn struct_layout(&self, structure: &StructType) -> (Vec<u32>, u32, u32) {
        let mut offsets = Vec::with_capacity(structure.fields.len());
        let mut size = 0u32;
        let mut align = 1u32;
        for field in &structure.fields {
            let field_align = if structure.packed {
                1
            } else {
                self.align_of(*field)
            };
            size = size.div_ceil(field_align) * field_align;
            offsets.push(size);
            size += self.size_of(*field);
            align = align.max(field_align);
        }
        size = size.div_ceil(align) * align;
        (offsets, size, align)
    }
}

pub struct FunctionBuilder<'f> {
    pub function: &'f mut Function,
    current: Option<Block>,
    terminated: Vec<bool>,
    sealed: Vec<bool>,
    predecessors: Vec<Vec<Block>>,
    variable_types: Vec<Type>,
    definitions: Vec<BTreeMap<Variable, Value>>,
    incomplete: Vec<Vec<Variable>>,
    variable_parameters: Vec<usize>,
    aliases: BTreeMap<Value, Value>,
    spot: Option<SourceSpot>,
}

impl<'f> FunctionBuilder<'f> {
    pub fn new(function: &'f mut Function) -> FunctionBuilder<'f> {
        let count = function.blocks.len();
        FunctionBuilder {
            function,
            current: None,
            terminated: vec![true; count],
            sealed: vec![true; count],
            predecessors: vec![Vec::new(); count],
            variable_types: Vec::new(),
            definitions: vec![BTreeMap::new(); count],
            incomplete: vec![Vec::new(); count],
            variable_parameters: vec![usize::MAX; count],
            aliases: BTreeMap::new(),
            spot: None,
        }
    }

    pub fn create_block(&mut self) -> Block {
        let entry = self.function.blocks.is_empty();
        self.function.blocks.push(BlockData {
            parameters: Vec::new(),
            instructions: Vec::new(),
            terminator: Terminator::Unreachable,
        });
        self.terminated.push(false);
        self.sealed.push(entry);
        self.predecessors.push(Vec::new());
        self.definitions.push(BTreeMap::new());
        self.incomplete.push(Vec::new());
        self.variable_parameters.push(usize::MAX);
        Block(self.function.blocks.len() as u32 - 1)
    }

    pub fn entry_parameters(&mut self, block: Block) -> Vec<Value> {
        let types = self.function.signature.parameters.clone();
        types
            .into_iter()
            .map(|ty| self.block_parameter(block, ty))
            .collect()
    }

    pub fn block_parameter(&mut self, block: Block, ty: Type) -> Value {
        assert!(
            self.variable_parameters[block.index()] == usize::MAX,
            "an explicit block parameter must come before any parameter the builder adds for a variable"
        );
        let value = self.function.new_value(ty);
        self.function.blocks[block.index()].parameters.push(value);
        value
    }

    pub fn switch_to(&mut self, block: Block) {
        self.current = Some(block);
    }

    pub fn current_block(&self) -> Option<Block> {
        self.current
    }

    pub fn is_terminated(&self, block: Block) -> bool {
        self.terminated[block.index()]
    }

    pub fn set_spot(&mut self, spot: Option<SourceSpot>) {
        self.spot = spot;
    }

    pub fn push(
        &mut self,
        operation: Operation,
        arguments: Vec<Value>,
        result: Option<Type>,
    ) -> Option<Value> {
        let block = self.current.expect("an instruction needs a current block");
        assert!(
            !self.terminated[block.index()],
            "the block is already terminated"
        );
        let result = result.map(|ty| self.function.new_value(ty));
        self.function.instructions.push(Instruction {
            operation,
            arguments,
            result,
            spot: self.spot.clone(),
        });
        let inst = Inst(self.function.instructions.len() as u32 - 1);
        self.function.blocks[block.index()].instructions.push(inst);
        result
    }

    fn value(&mut self, operation: Operation, arguments: Vec<Value>, ty: Type) -> Value {
        self.push(operation, arguments, Some(ty))
            .unwrap_or(Value(u32::MAX))
    }

    pub fn integer(&mut self, ty: Type, value: i128) -> Value {
        self.value(Operation::IntegerConstant(value), Vec::new(), ty)
    }

    pub fn float(&mut self, ty: Type, value: f64) -> Value {
        let bits = if ty == Type::F32 {
            u64::from((value as f32).to_bits())
        } else {
            value.to_bits()
        };
        self.value(Operation::FloatConstant(bits), Vec::new(), ty)
    }

    pub fn null(&mut self, ty: Type) -> Value {
        self.value(Operation::Null, Vec::new(), ty)
    }

    pub fn binary(&mut self, op: BinaryOp, left: Value, right: Value) -> Value {
        let ty = if op.yields_bool() {
            Type::BOOL
        } else {
            self.function.type_of(left)
        };
        self.value(Operation::Binary(op), vec![left, right], ty)
    }

    pub fn unary(&mut self, op: UnaryOp, operand: Value) -> Value {
        let ty = self.function.type_of(operand);
        self.value(Operation::Unary(op), vec![operand], ty)
    }

    pub fn compare(&mut self, comparison: Comparison, left: Value, right: Value) -> Value {
        self.value(
            Operation::Compare(comparison),
            vec![left, right],
            Type::BOOL,
        )
    }

    pub fn float_compare(
        &mut self,
        comparison: FloatComparison,
        left: Value,
        right: Value,
    ) -> Value {
        self.value(
            Operation::FloatCompare(comparison),
            vec![left, right],
            Type::BOOL,
        )
    }

    pub fn convert(&mut self, conversion: Conversion, operand: Value, ty: Type) -> Value {
        self.value(Operation::Convert(conversion), vec![operand], ty)
    }

    pub fn select(&mut self, condition: Value, taken: Value, otherwise: Value) -> Value {
        let ty = self.function.type_of(taken);
        self.value(Operation::Select, vec![condition, taken, otherwise], ty)
    }

    pub fn stack_slot(&mut self, size: u32, align: u32) -> Value {
        self.value(
            Operation::StackSlot { size, align },
            Vec::new(),
            Type::POINTER,
        )
    }

    pub fn dynamic_stack_slot(&mut self, size: Value, align: u32) -> Value {
        self.value(
            Operation::DynamicStackSlot { align },
            vec![size],
            Type::POINTER,
        )
    }

    pub fn stack_save(&mut self) -> Value {
        self.value(Operation::StackSave, Vec::new(), Type::POINTER)
    }

    pub fn stack_restore(&mut self, saved: Value) {
        self.push(Operation::StackRestore, vec![saved], None);
    }

    pub fn va_start(&mut self, list: Value) {
        self.push(Operation::VaStart, vec![list], None);
    }

    pub fn va_arg(&mut self, list: Value, ty: Type) -> Value {
        self.value(Operation::VaArg, vec![list], ty)
    }

    pub fn va_copy(&mut self, destination: Value, source: Value) {
        self.push(Operation::VaCopy, vec![destination, source], None);
    }

    pub fn load(&mut self, ty: Type, address: Value, align: u32) -> Value {
        self.value(
            Operation::Load {
                align,
                volatile: false,
                ordering: None,
            },
            vec![address],
            ty,
        )
    }

    pub fn store(&mut self, value: Value, address: Value, align: u32) {
        self.push(
            Operation::Store {
                align,
                volatile: false,
                ordering: None,
            },
            vec![value, address],
            None,
        );
    }

    pub fn load_volatile(&mut self, ty: Type, address: Value, align: u32) -> Value {
        self.value(
            Operation::Load {
                align,
                volatile: true,
                ordering: None,
            },
            vec![address],
            ty,
        )
    }

    pub fn store_volatile(&mut self, value: Value, address: Value, align: u32) {
        self.push(
            Operation::Store {
                align,
                volatile: true,
                ordering: None,
            },
            vec![value, address],
            None,
        );
    }

    pub fn offset(&mut self, address: Value, bytes: Value) -> Value {
        self.value(Operation::Offset, vec![address, bytes], Type::POINTER)
    }

    pub fn global_address(&mut self, global: GlobalId) -> Value {
        self.value(Operation::GlobalAddress(global), Vec::new(), Type::POINTER)
    }

    pub fn function_address(&mut self, function: FunctionId) -> Value {
        self.value(
            Operation::FunctionAddress(function),
            Vec::new(),
            Type::POINTER,
        )
    }

    pub fn call(
        &mut self,
        callee: FunctionId,
        result: Option<Type>,
        arguments: Vec<Value>,
    ) -> Option<Value> {
        self.push(Operation::Call(callee), arguments, result)
    }

    pub fn trap(&mut self, code: u32) {
        self.push(Operation::Trap(code), Vec::new(), None);
    }

    pub fn terminate(&mut self, terminator: Terminator) {
        let block = self.current.expect("a terminator needs a current block");
        assert!(
            !self.terminated[block.index()],
            "the block is already terminated"
        );
        for successor in terminator.successors() {
            assert!(
                !self.sealed[successor.block.index()],
                "a sealed block cannot gain a predecessor"
            );
            if !self.predecessors[successor.block.index()].contains(&block) {
                self.predecessors[successor.block.index()].push(block);
            }
        }
        self.function.blocks[block.index()].terminator = terminator;
        self.terminated[block.index()] = true;
    }

    pub fn jump(&mut self, target: Block, arguments: Vec<Value>) {
        self.terminate(Terminator::Jump(BlockCall::with(target, arguments)));
    }

    pub fn branch(&mut self, condition: Value, taken: Block, otherwise: Block) {
        self.terminate(Terminator::Branch {
            condition,
            taken: BlockCall::to(taken),
            otherwise: BlockCall::to(otherwise),
        });
    }

    pub fn ret(&mut self, value: Option<Value>) {
        self.terminate(Terminator::Return(value));
    }

    pub fn declare_variable(&mut self, ty: Type) -> Variable {
        self.variable_types.push(ty);
        Variable(self.variable_types.len() as u32 - 1)
    }

    pub fn define(&mut self, variable: Variable, value: Value) {
        let block = self.current.expect("a definition needs a current block");
        self.definitions[block.index()].insert(variable, value);
    }

    pub fn read(&mut self, variable: Variable) -> Value {
        let block = self.current.expect("a read needs a current block");
        self.read_in(variable, block)
    }

    fn resolve(&self, value: Value) -> Value {
        let mut current = value;
        while let Some(next) = self.aliases.get(&current) {
            current = *next;
        }
        current
    }

    fn read_in(&mut self, variable: Variable, block: Block) -> Value {
        if let Some(found) = self.definitions[block.index()].get(&variable) {
            return self.resolve(*found);
        }
        self.read_recursive(variable, block)
    }

    fn variable_parameter(&mut self, variable: Variable, block: Block) -> Value {
        let ty = self.variable_types[variable.index()];
        let value = self.function.new_value(ty);
        let data = &mut self.function.blocks[block.index()];
        if self.variable_parameters[block.index()] == usize::MAX {
            self.variable_parameters[block.index()] = data.parameters.len();
        }
        data.parameters.push(value);
        value
    }

    fn read_recursive(&mut self, variable: Variable, block: Block) -> Value {
        let value = if !self.sealed[block.index()] {
            let parameter = self.variable_parameter(variable, block);
            self.incomplete[block.index()].push(variable);
            parameter
        } else if self.predecessors[block.index()].len() == 1 {
            let only = self.predecessors[block.index()][0];
            self.read_in(variable, only)
        } else if self.predecessors[block.index()].is_empty() {
            let ty = self.variable_types[variable.index()];
            let value = self.function.new_value(ty);
            self.function.instructions.push(Instruction {
                operation: Operation::Poison,
                arguments: Vec::new(),
                result: Some(value),
                spot: None,
            });
            let inst = Inst(self.function.instructions.len() as u32 - 1);
            self.function.blocks[block.index()]
                .instructions
                .insert(0, inst);
            value
        } else {
            let parameter = self.variable_parameter(variable, block);
            self.definitions[block.index()].insert(variable, parameter);
            self.supply(variable, block);
            parameter
        };
        self.definitions[block.index()].insert(variable, value);
        value
    }

    fn supply(&mut self, variable: Variable, block: Block) {
        let predecessors = self.predecessors[block.index()].clone();
        for predecessor in predecessors {
            let incoming = self.read_in(variable, predecessor);
            let terminator = &mut self.function.blocks[predecessor.index()].terminator;
            for edge in terminator.successors_mut() {
                if edge.block == block {
                    edge.arguments.push(incoming);
                }
            }
        }
    }

    pub fn seal(&mut self, block: Block) {
        if self.sealed[block.index()] {
            return;
        }
        let pending = std::mem::take(&mut self.incomplete[block.index()]);
        self.sealed[block.index()] = true;
        for variable in pending {
            self.supply(variable, block);
        }
    }

    pub fn seal_all(&mut self) {
        for index in 0..self.function.blocks.len() {
            self.seal(Block(index as u32));
        }
    }

    pub fn finish(mut self) -> Result<(), Diagnostic> {
        self.seal_all();
        if let Some(open) = self.terminated.iter().position(|done| !done) {
            let violation = Violation {
                rule: Rule::UnterminatedBlock,
                subject: global_symbol(&self.function.name),
                block: Some(open as u32),
                instruction: None,
            };
            return Err(violation.to_diagnostic());
        }
        self.drop_trivial_parameters();
        self.rewrite_aliases();
        Ok(())
    }

    fn drop_trivial_parameters(&mut self) {
        loop {
            let mut changed = false;
            let mut incoming: Vec<Vec<(Block, usize, usize)>> =
                vec![Vec::new(); self.function.blocks.len()];
            for (index, data) in self.function.blocks.iter().enumerate() {
                let invoke = matches!(data.terminator, Terminator::Invoke { .. });
                for (edge_index, edge) in data.terminator.successors().iter().enumerate() {
                    let shift = usize::from(invoke && edge_index == 1);
                    incoming[edge.block.index()].push((Block(index as u32), edge_index, shift));
                }
            }
            for (block, sources) in incoming.iter().enumerate() {
                let start = self.variable_parameters[block];
                if start == usize::MAX || block == 0 {
                    continue;
                }
                let mut position = start;
                while position < self.function.blocks[block].parameters.len() {
                    let parameter = self.function.blocks[block].parameters[position];
                    let mut same: Option<Value> = None;
                    let mut trivial = true;
                    for (from, edge_index, shift) in sources {
                        let edges = self.function.blocks[from.index()].terminator.successors();
                        let argument = position
                            .checked_sub(*shift)
                            .and_then(|at| edges[*edge_index].arguments.get(at));
                        let Some(argument) = argument else {
                            trivial = false;
                            break;
                        };
                        let argument = self.resolve(*argument);
                        if argument == parameter || Some(argument) == same {
                            continue;
                        }
                        if same.is_some() {
                            trivial = false;
                            break;
                        }
                        same = Some(argument);
                    }
                    let Some(replacement) = same.filter(|_| trivial) else {
                        position += 1;
                        continue;
                    };
                    self.aliases.insert(parameter, replacement);
                    self.function.blocks[block].parameters.remove(position);
                    for (from, edge_index, shift) in sources {
                        let terminator = &mut self.function.blocks[from.index()].terminator;
                        let mut edges = terminator.successors_mut();
                        let at = position - shift;
                        if at < edges[*edge_index].arguments.len() {
                            edges[*edge_index].arguments.remove(at);
                        }
                    }
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
    }

    fn rewrite_aliases(&mut self) {
        if self.aliases.is_empty() {
            return;
        }
        let aliases = self.aliases.clone();
        let resolve = |value: Value| -> Value {
            let mut current = value;
            while let Some(next) = aliases.get(&current) {
                current = *next;
            }
            current
        };
        for instruction in &mut self.function.instructions {
            for argument in &mut instruction.arguments {
                *argument = resolve(*argument);
            }
        }
        for data in &mut self.function.blocks {
            match &mut data.terminator {
                Terminator::Branch { condition, .. } => *condition = resolve(*condition),
                Terminator::Switch { value, .. } => *value = resolve(*value),
                Terminator::Return(Some(value)) => *value = resolve(*value),
                Terminator::Throw(value) => *value = resolve(*value),
                Terminator::Invoke { call, .. } => {
                    for argument in &mut call.arguments {
                        *argument = resolve(*argument);
                    }
                }
                _ => {}
            }
            for edge in data.terminator.successors_mut() {
                for argument in &mut edge.arguments {
                    *argument = resolve(*argument);
                }
            }
        }
    }
}

fn plain_name(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with(|first: char| first.is_ascii_digit())
        && name.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | '$')
        })
}

fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            other if (other as u32) < 32 || other as u32 == 127 => {
                out.push_str(&format!("\\u{{{:x}}}", other as u32));
            }
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn symbol(prefix: char, name: &str) -> String {
    if plain_name(name) {
        format!("{prefix}{name}")
    } else {
        format!("{prefix}{}", quote(name))
    }
}

fn hex_bytes(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[usize::from(byte >> 4)] as char);
        out.push(DIGITS[usize::from(byte & 15)] as char);
    }
    out
}

struct Numbering {
    names: BTreeMap<Value, usize>,
}

impl Numbering {
    fn of(function: &Function) -> Numbering {
        let mut names = BTreeMap::new();
        for data in &function.blocks {
            for parameter in &data.parameters {
                let next = names.len();
                names.entry(*parameter).or_insert(next);
            }
            for inst in &data.instructions {
                if let Some(result) = function.instructions[inst.index()].result {
                    let next = names.len();
                    names.entry(result).or_insert(next);
                }
            }
            if let Terminator::Invoke { call, .. } = &data.terminator {
                if let Some(result) = call.result {
                    let next = names.len();
                    names.entry(result).or_insert(next);
                }
            }
        }
        Numbering { names }
    }

    fn name(&self, value: Value) -> String {
        match self.names.get(&value) {
            Some(number) => format!("%{number}"),
            None => format!("%undefined{}", value.0),
        }
    }
}

pub struct Printer<'m> {
    module: &'m Module,
}

impl<'m> Printer<'m> {
    pub fn new(module: &'m Module) -> Printer<'m> {
        Printer { module }
    }

    fn function_name(&self, id: FunctionId) -> String {
        match self.module.functions.get(id.index()) {
            Some(function) => symbol('@', &function.name),
            None => format!("@\"#function{}\"", id.0),
        }
    }

    fn global_name(&self, id: GlobalId) -> String {
        match self.module.globals.get(id.index()) {
            Some(global) => symbol('@', &global.name),
            None => format!("@\"#global{}\"", id.0),
        }
    }

    fn class_name(&self, id: ClassId) -> String {
        match self.module.classes.get(id.index()) {
            Some(class) => symbol('%', &class.name),
            None => format!("%\"#class{}\"", id.0),
        }
    }

    fn struct_name(&self, id: StructId) -> String {
        match self.module.structs.get(id.index()) {
            Some(structure) => symbol('%', &structure.name),
            None => format!("%\"#struct{}\"", id.0),
        }
    }

    fn signature(&self, signature: &Signature) -> String {
        let parameters: Vec<String> = signature.parameters.iter().map(|ty| ty.text()).collect();
        let mut out = format!("({})", parameters.join(", "));
        if let Some(result) = signature.result {
            out.push_str(&format!(" -> {}", result.text()));
        }
        if let Some(convention) = signature.convention {
            out.push_str(&format!(" {}", convention.name()));
        }
        if signature.variadic {
            out.push_str(" variadic");
        }
        out
    }

    fn values(&self, numbering: &Numbering, values: &[Value]) -> String {
        values
            .iter()
            .map(|value| numbering.name(*value))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn block_call(&self, numbering: &Numbering, call: &BlockCall) -> String {
        if call.arguments.is_empty() {
            format!("block{}", call.block.0)
        } else {
            format!(
                "block{}({})",
                call.block.0,
                self.values(numbering, &call.arguments)
            )
        }
    }

    fn memory_flags(volatile: bool, ordering: Option<Ordering>, align: u32) -> String {
        let mut out = String::new();
        if volatile {
            out.push_str("volatile ");
        }
        if let Some(ordering) = ordering {
            out.push_str(&format!("atomic {} ", ordering.name()));
        }
        out.push_str(&format!("align {align}"));
        out
    }

    fn operation(&self, numbering: &Numbering, instruction: &Instruction) -> String {
        let arguments = self.values(numbering, &instruction.arguments);
        let with = |head: String| -> String {
            if arguments.is_empty() {
                head
            } else {
                format!("{head} {arguments}")
            }
        };
        match &instruction.operation {
            Operation::IntegerConstant(value) => format!("const {value}"),
            Operation::FloatConstant(bits) => format!("fconst 0x{bits:x}"),
            Operation::Null => "null".to_string(),
            Operation::Poison => "poison".to_string(),
            Operation::GlobalAddress(id) => format!("global_address {}", self.global_name(*id)),
            Operation::FunctionAddress(id) => {
                format!("function_address {}", self.function_name(*id))
            }
            Operation::StringConstant(index) => format!("string {index}"),
            Operation::Binary(op) => with(op.name().to_string()),
            Operation::Unary(op) => with(op.name().to_string()),
            Operation::Compare(comparison) => with(format!("icmp {}", comparison.name())),
            Operation::FloatCompare(comparison) => with(format!("fcmp {}", comparison.name())),
            Operation::Convert(conversion) => with(conversion.name().to_string()),
            Operation::Select => with("select".to_string()),
            Operation::Splat => with("splat".to_string()),
            Operation::ExtractLane(lane) => with(format!("extract_lane {lane}")),
            Operation::InsertLane(lane) => with(format!("insert_lane {lane}")),
            Operation::StackSlot { size, align } => format!("stack_slot size {size} align {align}"),
            Operation::DynamicStackSlot { align } => {
                with(format!("dynamic_stack_slot align {align}"))
            }
            Operation::StackSave => with("stack_save".to_string()),
            Operation::StackRestore => with("stack_restore".to_string()),
            Operation::VaStart => with("va_start".to_string()),
            Operation::VaArg => with("va_arg".to_string()),
            Operation::VaCopy => with("va_copy".to_string()),
            Operation::Load {
                align,
                volatile,
                ordering,
            } => with(format!(
                "load {}",
                Self::memory_flags(*volatile, *ordering, *align)
            )),
            Operation::Store {
                align,
                volatile,
                ordering,
            } => with(format!(
                "store {}",
                Self::memory_flags(*volatile, *ordering, *align)
            )),
            Operation::Offset => with("offset".to_string()),
            Operation::FieldAddress(structure, index) => with(format!(
                "field_address {} {index}",
                self.struct_name(*structure)
            )),
            Operation::ElementAddress(ty) => with(format!("element_address {}", ty.text())),
            Operation::AtomicUpdate(op, ordering) => {
                with(format!("atomic_update {} {}", op.name(), ordering.name()))
            }
            Operation::CompareExchange(success, failure) => with(format!(
                "compare_exchange {} {}",
                success.name(),
                failure.name()
            )),
            Operation::Fence(ordering) => format!("fence {}", ordering.name()),
            Operation::MemoryCopy => with("memory_copy".to_string()),
            Operation::MemoryMove => with("memory_move".to_string()),
            Operation::MemoryFill => with("memory_fill".to_string()),
            Operation::Call(callee) => format!("call {}({arguments})", self.function_name(*callee)),
            Operation::CallIndirect(signature) => {
                let callee = instruction
                    .arguments
                    .first()
                    .map(|value| numbering.name(*value))
                    .unwrap_or_default();
                let rest = self.values(numbering, instruction.arguments.get(1..).unwrap_or(&[]));
                format!(
                    "call_indirect {} {callee}({rest})",
                    self.signature(signature)
                )
            }
            Operation::Allocate(class) => format!("allocate {}", self.class_name(*class)),
            Operation::AllocateArray(class) => {
                with(format!("allocate_array {}", self.class_name(*class)))
            }
            Operation::LoadField(class, index) => {
                with(format!("load_field {} {index}", self.class_name(*class)))
            }
            Operation::StoreField(class, index) => {
                with(format!("store_field {} {index}", self.class_name(*class)))
            }
            Operation::LoadElement(ty) => with(format!("load_element {}", ty.text())),
            Operation::StoreElement(ty) => with(format!("store_element {}", ty.text())),
            Operation::ArrayLength => with("array_length".to_string()),
            Operation::InstanceOf(class) => {
                with(format!("instance_of {}", self.class_name(*class)))
            }
            Operation::VirtualMethod(class, slot) => {
                with(format!("virtual_method {} {slot}", self.class_name(*class)))
            }
            Operation::InterfaceMethod(class, slot) => with(format!(
                "interface_method {} {slot}",
                self.class_name(*class)
            )),
            Operation::SameObject => with("same_object".to_string()),
            Operation::Trap(code) => format!("trap {code}"),
        }
    }

    fn instruction(
        &self,
        numbering: &Numbering,
        function: &Function,
        instruction: &Instruction,
    ) -> String {
        let mut line = String::new();
        if let Some(result) = instruction.result {
            line.push_str(&format!(
                "{}: {} = ",
                numbering.name(result),
                function
                    .value_types
                    .get(result.index())
                    .map(|ty| ty.text())
                    .unwrap_or_default()
            ));
        }
        line.push_str(&self.operation(numbering, instruction));
        if let Some(spot) = &instruction.spot {
            line.push_str(&format!(
                " !loc {}:{}:{}",
                spot.file, spot.line, spot.column
            ));
        }
        line
    }

    fn terminator(
        &self,
        numbering: &Numbering,
        function: &Function,
        terminator: &Terminator,
    ) -> String {
        match terminator {
            Terminator::Jump(call) => format!("jump {}", self.block_call(numbering, call)),
            Terminator::Branch {
                condition,
                taken,
                otherwise,
            } => format!(
                "branch {}, {}, {}",
                numbering.name(*condition),
                self.block_call(numbering, taken),
                self.block_call(numbering, otherwise)
            ),
            Terminator::Switch {
                value,
                cases,
                default,
            } => {
                let listed: Vec<String> = cases
                    .iter()
                    .map(|(case, call)| format!("{case}: {}", self.block_call(numbering, call)))
                    .collect();
                format!(
                    "switch {}, [{}], default {}",
                    numbering.name(*value),
                    listed.join(", "),
                    self.block_call(numbering, default)
                )
            }
            Terminator::Return(None) => "return".to_string(),
            Terminator::Return(Some(value)) => format!("return {}", numbering.name(*value)),
            Terminator::Unreachable => "unreachable".to_string(),
            Terminator::Throw(value) => format!("throw {}", numbering.name(*value)),
            Terminator::Invoke {
                call,
                normal,
                unwind,
            } => {
                let head = self.instruction(numbering, function, call);
                let (prefix, rest) = match head.find(" = ") {
                    Some(at) if call.result.is_some() => {
                        (head[..at + 3].to_string(), head[at + 3..].to_string())
                    }
                    _ => (String::new(), head),
                };
                format!(
                    "{prefix}invoke {rest} to {} unwind {}",
                    self.block_call(numbering, normal),
                    self.block_call(numbering, unwind)
                )
            }
        }
    }

    pub fn print(&self) -> String {
        let module = self.module;
        let mut out = String::new();
        out.push_str(&format!(
            "ir {FORMAT_VERSION}\nmodule {}\nlayout {}\n",
            quote(&module.name),
            module.layout.name()
        ));
        for file in &module.files {
            out.push_str(&format!("file {}\n", quote(file)));
        }
        for text in &module.strings {
            out.push_str(&format!("string {}\n", quote(text)));
        }
        for structure in &module.structs {
            let fields: Vec<String> = structure.fields.iter().map(|ty| ty.text()).collect();
            out.push_str(&format!(
                "struct {}{} {{ {} }}\n",
                symbol('%', &structure.name),
                if structure.packed { " packed" } else { "" },
                fields.join(", ")
            ));
        }
        for class in &module.classes {
            let mut line = format!("class {}", symbol('%', &class.name));
            if class.interface {
                line.push_str(" interface");
            }
            if class.abstract_class {
                line.push_str(" abstract");
            }
            if let Some(element) = class.element {
                line.push_str(&format!(" array {}", element.text()));
            }
            if let Some(parent) = class.parent {
                line.push_str(&format!(" : {}", self.class_name(parent)));
            }
            let fields: Vec<String> = class.fields.iter().map(|ty| ty.text()).collect();
            let methods = |list: &[Option<FunctionId>]| -> String {
                list.iter()
                    .map(|slot| match slot {
                        Some(id) => self.function_name(*id),
                        None => "-".to_string(),
                    })
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            line.push_str(&format!(
                " {{ fields [{}] methods [{}]",
                fields.join(", "),
                methods(&class.methods)
            ));
            for (interface, slots) in &class.implementations {
                line.push_str(&format!(
                    " implements {} [{}]",
                    self.class_name(*interface),
                    methods(slots)
                ));
            }
            line.push_str(" }\n");
            out.push_str(&line);
        }
        for global in &module.globals {
            let mut line = format!(
                "global {} size {} align {}",
                symbol('@', &global.name),
                global.size,
                global.align
            );
            if global.mutable {
                line.push_str(" mutable");
            }
            if global.thread_local {
                line.push_str(" thread_local");
            }
            line.push_str(&format!(" {}", global.linkage.name()));
            match &global.content {
                GlobalContent::Zero => line.push_str(" zero"),
                GlobalContent::Bytes(bytes) => {
                    line.push_str(&format!(" bytes \"{}\"", hex_bytes(bytes)))
                }
            }
            for relocation in &global.relocations {
                let target = match relocation.target {
                    Symbol::Global(id) => format!("global {}", self.global_name(id)),
                    Symbol::Function(id) => format!("function {}", self.function_name(id)),
                };
                line.push_str(&format!(
                    " relocate {} {target} {}",
                    relocation.offset, relocation.addend
                ));
            }
            line.push('\n');
            out.push_str(&line);
        }
        for function in &module.functions {
            out.push_str(&format!(
                "function {}{} {}",
                symbol('@', &function.name),
                self.signature(&function.signature),
                function.linkage.name()
            ));
            for attribute in &function.attributes {
                out.push_str(&format!(" {}", attribute.name()));
            }
            if function.is_declaration() {
                out.push('\n');
                continue;
            }
            out.push_str(" {\n");
            let numbering = Numbering::of(function);
            for (index, data) in function.blocks.iter().enumerate() {
                if data.parameters.is_empty() {
                    out.push_str(&format!("block{index}:\n"));
                } else {
                    let parameters: Vec<String> = data
                        .parameters
                        .iter()
                        .map(|value| {
                            format!(
                                "{}: {}",
                                numbering.name(*value),
                                function
                                    .value_types
                                    .get(value.index())
                                    .map(|ty| ty.text())
                                    .unwrap_or_default()
                            )
                        })
                        .collect();
                    out.push_str(&format!("block{index}({}):\n", parameters.join(", ")));
                }
                for inst in &data.instructions {
                    out.push_str("    ");
                    out.push_str(&self.instruction(
                        &numbering,
                        function,
                        &function.instructions[inst.index()],
                    ));
                    out.push('\n');
                }
                out.push_str("    ");
                out.push_str(&self.terminator(&numbering, function, &data.terminator));
                out.push('\n');
            }
            out.push_str("}\n");
        }
        out
    }
}

impl Module {
    pub fn to_text(&self) -> String {
        Printer::new(self).print()
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Word(String),
    Local(String),
    GlobalName(String),
    Text(String),
    Number(i128),
    Punct(char),
    Arrow,
}

impl Token {
    fn shown(&self) -> String {
        match self {
            Token::Word(word) => word.clone(),
            Token::Local(name) => symbol('%', name),
            Token::GlobalName(name) => symbol('@', name),
            Token::Text(text) => quote(text),
            Token::Number(number) => number.to_string(),
            Token::Punct(mark) => mark.to_string(),
            Token::Arrow => "->".to_string(),
        }
    }
}

struct ReadError {
    line: usize,
    reason: &'static str,
    expected: String,
    found: String,
}

type Parsed<T> = Result<T, ReadError>;

fn read_error<T>(line: usize, reason: &'static str, found: impl Into<String>) -> Parsed<T> {
    Err(ReadError {
        line,
        reason,
        expected: String::new(),
        found: found.into(),
    })
}

struct Tokens {
    items: Vec<(Token, usize)>,
    at: usize,
}

fn read_string(characters: &[char], at: &mut usize, line: usize) -> Parsed<String> {
    let mut out = String::new();
    let opened = *at;
    *at += 1;
    while *at < characters.len() {
        let character = characters[*at];
        *at += 1;
        match character {
            '"' => return Ok(out),
            '\\' => {
                let Some(escaped) = characters.get(*at).copied() else {
                    break;
                };
                *at += 1;
                match escaped {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'r' => out.push('\r'),
                    '"' => out.push('"'),
                    '\\' => out.push('\\'),
                    'u' => {
                        let start = *at;
                        let mut stop = start;
                        while stop < characters.len()
                            && characters[stop] != '}'
                            && stop - start < 10
                        {
                            stop += 1;
                        }
                        let decoded = (characters.get(start) == Some(&'{')
                            && characters.get(stop) == Some(&'}'))
                        .then(|| characters[start + 1..stop].iter().collect::<String>())
                        .and_then(|digits| u32::from_str_radix(&digits, 16).ok())
                        .and_then(char::from_u32);
                        let Some(decoded) = decoded else {
                            let shown: String = characters[*at - 2..stop.min(characters.len())]
                                .iter()
                                .collect();
                            return read_error(line, "unknown-escape", shown);
                        };
                        out.push(decoded);
                        *at = stop + 1;
                    }
                    other => return read_error(line, "unknown-escape", format!("\\{other}")),
                }
            }
            other => out.push(other),
        }
    }
    let shown: String = characters[opened..characters.len().min(opened + 24)]
        .iter()
        .collect();
    read_error(line, "unclosed-string", shown)
}

fn tokenize(text: &str) -> Parsed<Vec<(Token, usize)>> {
    let characters: Vec<char> = text.chars().collect();
    let mut tokens = Vec::new();
    let mut at = 0usize;
    let mut line = 1usize;
    let naming =
        |character: char| character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | '$');
    while at < characters.len() {
        let character = characters[at];
        if character == '\n' {
            line += 1;
            at += 1;
            continue;
        }
        if character.is_whitespace() {
            at += 1;
            continue;
        }
        if character == '%' || character == '@' {
            at += 1;
            let name = if characters.get(at) == Some(&'"') {
                read_string(&characters, &mut at, line)?
            } else {
                let start = at;
                while at < characters.len() && naming(characters[at]) {
                    at += 1;
                }
                characters[start..at].iter().collect()
            };
            if name.is_empty() {
                return read_error(line, "missing-name", character.to_string());
            }
            tokens.push((
                if character == '%' {
                    Token::Local(name)
                } else {
                    Token::GlobalName(name)
                },
                line,
            ));
            continue;
        }
        if character == '"' {
            let text = read_string(&characters, &mut at, line)?;
            tokens.push((Token::Text(text), line));
            continue;
        }
        if character == '-' && characters.get(at + 1) == Some(&'>') {
            tokens.push((Token::Arrow, line));
            at += 2;
            continue;
        }
        if character.is_ascii_digit()
            || (character == '-' && characters.get(at + 1).is_some_and(char::is_ascii_digit))
        {
            let start = at;
            at += 1;
            while at < characters.len() && characters[at].is_ascii_alphanumeric() {
                at += 1;
            }
            let raw: String = characters[start..at].iter().collect();
            let (negative, body) = match raw.strip_prefix('-') {
                Some(rest) => (true, rest),
                None => (false, raw.as_str()),
            };
            let magnitude = match body.strip_prefix("0x") {
                Some(hex) => u128::from_str_radix(hex, 16).ok(),
                None => body.parse::<u128>().ok(),
            };
            let value = magnitude.and_then(|magnitude| {
                if negative {
                    if magnitude <= 1u128 << 127 {
                        Some((magnitude as i128).wrapping_neg())
                    } else {
                        None
                    }
                } else if body.starts_with("0x") {
                    Some(magnitude as i128)
                } else {
                    i128::try_from(magnitude).ok()
                }
            });
            let Some(value) = value else {
                return read_error(line, "invalid-number", raw);
            };
            tokens.push((Token::Number(value), line));
            continue;
        }
        if character.is_ascii_alphabetic() || character == '_' {
            let start = at;
            while at < characters.len()
                && (characters[at].is_ascii_alphanumeric() || characters[at] == '_')
            {
                at += 1;
            }
            tokens.push((Token::Word(characters[start..at].iter().collect()), line));
            continue;
        }
        if "(){}[],:=<>!-".contains(character) {
            tokens.push((Token::Punct(character), line));
            at += 1;
            continue;
        }
        return read_error(line, "unexpected-character", character.to_string());
    }
    Ok(tokens)
}

impl Tokens {
    fn line(&self) -> usize {
        self.items
            .get(self.at.saturating_sub(1))
            .or_else(|| self.items.last())
            .map(|(_, line)| *line)
            .unwrap_or(1)
    }

    fn peek(&self) -> Option<&Token> {
        self.items.get(self.at).map(|(token, _)| token)
    }

    fn peek_second(&self) -> Option<&Token> {
        self.items.get(self.at + 1).map(|(token, _)| token)
    }

    fn next(&mut self) -> Option<Token> {
        let token = self.items.get(self.at).map(|(token, _)| token.clone());
        if token.is_some() {
            self.at += 1;
        }
        token
    }

    fn fault<T>(&self, reason: &'static str, found: impl Into<String>) -> Parsed<T> {
        read_error(self.line(), reason, found)
    }

    fn mismatch<T>(&self, expected: &str, found: Option<Token>) -> Parsed<T> {
        Err(ReadError {
            line: self.line(),
            reason: if found.is_some() {
                "unexpected-token"
            } else {
                "unexpected-end"
            },
            expected: expected.to_string(),
            found: found.map(|token| token.shown()).unwrap_or_default(),
        })
    }

    fn expect_punct(&mut self, wanted: char) -> Parsed<()> {
        match self.next() {
            Some(Token::Punct(found)) if found == wanted => Ok(()),
            other => self.mismatch(&wanted.to_string(), other),
        }
    }

    fn eat_punct(&mut self, wanted: char) -> bool {
        if self.peek() == Some(&Token::Punct(wanted)) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn expect_word(&mut self, wanted: &str) -> Parsed<()> {
        match self.next() {
            Some(Token::Word(found)) if found == wanted => Ok(()),
            other => self.mismatch(wanted, other),
        }
    }

    fn eat_word(&mut self, wanted: &str) -> bool {
        if matches!(self.peek(), Some(Token::Word(found)) if found == wanted) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn word(&mut self) -> Parsed<String> {
        match self.next() {
            Some(Token::Word(found)) => Ok(found),
            other => self.mismatch("word", other),
        }
    }

    fn named<T>(&mut self, lookup: fn(&str) -> Option<T>) -> Parsed<T> {
        let word = self.word()?;
        match lookup(&word) {
            Some(found) => Ok(found),
            None => self.fault("unknown-keyword", word),
        }
    }

    fn number(&mut self) -> Parsed<i128> {
        match self.next() {
            Some(Token::Number(found)) => Ok(found),
            other => self.mismatch("number", other),
        }
    }

    fn small(&mut self) -> Parsed<u32> {
        let value = self.number()?;
        match u32::try_from(value) {
            Ok(small) => Ok(small),
            Err(_) => self.fault("number-too-large", value.to_string()),
        }
    }

    fn lane(&mut self) -> Parsed<u8> {
        let value = self.number()?;
        match u8::try_from(value) {
            Ok(lane) => Ok(lane),
            Err(_) => self.fault("number-too-large", value.to_string()),
        }
    }

    fn text(&mut self) -> Parsed<String> {
        match self.next() {
            Some(Token::Text(found)) => Ok(found),
            other => self.mismatch("\"", other),
        }
    }

    fn global_name(&mut self) -> Parsed<String> {
        match self.next() {
            Some(Token::GlobalName(found)) => Ok(found),
            other => self.mismatch("@", other),
        }
    }

    fn local_name(&mut self) -> Parsed<String> {
        match self.next() {
            Some(Token::Local(found)) => Ok(found),
            other => self.mismatch("%", other),
        }
    }

    fn ty(&mut self) -> Parsed<Type> {
        if self.eat_punct('<') {
            let lanes = self.number()?;
            self.expect_word("x")?;
            let element = self.word()?;
            self.expect_punct('>')?;
            let shown = format!("<{lanes} x {element}>");
            let vector = u8::try_from(lanes)
                .ok()
                .zip(Scalar::named(&element))
                .and_then(|(lanes, element)| Type::vector(lanes, element));
            return match vector {
                Some(vector) => Ok(vector),
                None => self.fault("unknown-type", shown),
            };
        }
        let word = self.word()?;
        match Type::parse(&word) {
            Some(ty) => Ok(ty),
            None => self.fault("unknown-type", word),
        }
    }
}

struct Names {
    functions: BTreeMap<String, FunctionId>,
    globals: BTreeMap<String, GlobalId>,
    classes: BTreeMap<String, ClassId>,
    structs: BTreeMap<String, StructId>,
}

fn collect_names(tokens: &[(Token, usize)]) -> Parsed<Names> {
    let mut names = Names {
        functions: BTreeMap::new(),
        globals: BTreeMap::new(),
        classes: BTreeMap::new(),
        structs: BTreeMap::new(),
    };
    let mut depth = 0i32;
    for (index, (token, line)) in tokens.iter().enumerate() {
        match token {
            Token::Punct('{') => depth += 1,
            Token::Punct('}') => depth -= 1,
            Token::Word(word)
                if depth == 0
                    && !(index >= 2
                        && matches!(&tokens[index - 2].0, Token::Word(before) if before == "relocate")) =>
            {
                let next = tokens.get(index + 1).map(|(token, _)| token);
                let fresh = match (word.as_str(), next) {
                    ("function", Some(Token::GlobalName(name))) => {
                        let id = FunctionId(names.functions.len() as u32);
                        Some((
                            names.functions.insert(name.clone(), id).is_none(),
                            symbol('@', name),
                        ))
                    }
                    ("global", Some(Token::GlobalName(name))) => {
                        let id = GlobalId(names.globals.len() as u32);
                        Some((
                            names.globals.insert(name.clone(), id).is_none(),
                            symbol('@', name),
                        ))
                    }
                    ("class", Some(Token::Local(name))) => {
                        let id = ClassId(names.classes.len() as u32);
                        Some((
                            names.classes.insert(name.clone(), id).is_none(),
                            symbol('%', name),
                        ))
                    }
                    ("struct", Some(Token::Local(name))) => {
                        let id = StructId(names.structs.len() as u32);
                        Some((
                            names.structs.insert(name.clone(), id).is_none(),
                            symbol('%', name),
                        ))
                    }
                    _ => None,
                };
                if let Some((false, shown)) = fresh {
                    return read_error(*line, "duplicate-name", shown);
                }
            }
            _ => {}
        }
    }
    Ok(names)
}

struct BodyValues {
    names: BTreeMap<String, Value>,
    defined: BTreeSet<Value>,
}

struct TextReader {
    tokens: Tokens,
    names: Names,
}

impl TextReader {
    fn reference<T: Copy>(
        &mut self,
        table: fn(&Names) -> &BTreeMap<String, T>,
        local: bool,
    ) -> Parsed<T> {
        let name = if local {
            self.tokens.local_name()?
        } else {
            self.tokens.global_name()?
        };
        match table(&self.names).get(&name) {
            Some(found) => Ok(*found),
            None => self
                .tokens
                .fault("unknown-name", symbol(if local { '%' } else { '@' }, &name)),
        }
    }

    fn function_ref(&mut self) -> Parsed<FunctionId> {
        self.reference(|names| &names.functions, false)
    }

    fn global_ref(&mut self) -> Parsed<GlobalId> {
        self.reference(|names| &names.globals, false)
    }

    fn class_ref(&mut self) -> Parsed<ClassId> {
        self.reference(|names| &names.classes, true)
    }

    fn struct_ref(&mut self) -> Parsed<StructId> {
        self.reference(|names| &names.structs, true)
    }

    fn signature(&mut self) -> Parsed<Signature> {
        self.tokens.expect_punct('(')?;
        let mut parameters = Vec::new();
        if !self.tokens.eat_punct(')') {
            loop {
                parameters.push(self.tokens.ty()?);
                if self.tokens.eat_punct(')') {
                    break;
                }
                self.tokens.expect_punct(',')?;
            }
        }
        let mut signature = Signature::new(parameters, None);
        if self.tokens.peek() == Some(&Token::Arrow) {
            self.tokens.next();
            signature.result = Some(self.tokens.ty()?);
        }
        if let Some(Token::Word(word)) = self.tokens.peek() {
            if let Some(convention) = Convention::named(word) {
                signature.convention = Some(convention);
                self.tokens.next();
            }
        }
        if self.tokens.eat_word("variadic") {
            signature.variadic = true;
        }
        Ok(signature)
    }

    fn value(&mut self, function: &mut Function, values: &mut BodyValues) -> Parsed<Value> {
        let name = self.tokens.local_name()?;
        if let Some(found) = values.names.get(&name) {
            return Ok(*found);
        }
        let value = function.new_value(Type::BOOL);
        values.names.insert(name, value);
        Ok(value)
    }

    fn define(
        &mut self,
        function: &mut Function,
        values: &mut BodyValues,
        name: String,
        ty: Type,
    ) -> Parsed<Value> {
        let value = match values.names.get(&name) {
            Some(found) => *found,
            None => {
                let value = function.new_value(ty);
                values.names.insert(name.clone(), value);
                value
            }
        };
        if !values.defined.insert(value) {
            return self.tokens.fault("duplicate-value", symbol('%', &name));
        }
        function.value_types[value.index()] = ty;
        Ok(value)
    }

    fn value_list(
        &mut self,
        function: &mut Function,
        values: &mut BodyValues,
    ) -> Parsed<Vec<Value>> {
        let mut out = Vec::new();
        if !matches!(self.tokens.peek(), Some(Token::Local(_))) {
            return Ok(out);
        }
        loop {
            out.push(self.value(function, values)?);
            if !self.tokens.eat_punct(',') {
                break;
            }
        }
        Ok(out)
    }

    fn block_number(&mut self) -> Parsed<u32> {
        let word = self.tokens.word()?;
        match word
            .strip_prefix("block")
            .and_then(|digits| digits.parse::<u32>().ok())
        {
            Some(number) => Ok(number),
            None => self.tokens.mismatch("block", Some(Token::Word(word))),
        }
    }

    fn block_call(
        &mut self,
        function: &mut Function,
        values: &mut BodyValues,
        referenced: &mut BTreeMap<u32, usize>,
    ) -> Parsed<BlockCall> {
        let number = self.block_number()?;
        referenced.entry(number).or_insert(self.tokens.line());
        let mut arguments = Vec::new();
        if self.tokens.eat_punct('(') && !self.tokens.eat_punct(')') {
            arguments = self.value_list(function, values)?;
            self.tokens.expect_punct(')')?;
        }
        Ok(BlockCall {
            block: Block(number),
            arguments,
        })
    }

    fn memory(&mut self) -> Parsed<(u32, bool, Option<Ordering>)> {
        let volatile = self.tokens.eat_word("volatile");
        let mut ordering = None;
        if self.tokens.eat_word("atomic") {
            ordering = Some(self.tokens.named(Ordering::named)?);
        }
        self.tokens.expect_word("align")?;
        let align = self.tokens.small()?;
        Ok((align, volatile, ordering))
    }

    fn operation(
        &mut self,
        function: &mut Function,
        values: &mut BodyValues,
        opcode: &str,
    ) -> Parsed<(Operation, Vec<Value>)> {
        let mut arguments = Vec::new();
        let operation = match opcode {
            "const" => Operation::IntegerConstant(self.tokens.number()?),
            "fconst" => {
                let bits = self.tokens.number()?;
                match u64::try_from(bits) {
                    Ok(bits) => Operation::FloatConstant(bits),
                    Err(_) => return self.tokens.fault("number-too-large", bits.to_string()),
                }
            }
            "null" => Operation::Null,
            "poison" => Operation::Poison,
            "global_address" => Operation::GlobalAddress(self.global_ref()?),
            "function_address" => Operation::FunctionAddress(self.function_ref()?),
            "string" => Operation::StringConstant(self.tokens.small()?),
            "icmp" => {
                let comparison = self.tokens.named(Comparison::named)?;
                arguments = self.value_list(function, values)?;
                Operation::Compare(comparison)
            }
            "fcmp" => {
                let comparison = self.tokens.named(FloatComparison::named)?;
                arguments = self.value_list(function, values)?;
                Operation::FloatCompare(comparison)
            }
            "select" => {
                arguments = self.value_list(function, values)?;
                Operation::Select
            }
            "splat" => {
                arguments = self.value_list(function, values)?;
                Operation::Splat
            }
            "extract_lane" | "insert_lane" => {
                let lane = self.tokens.lane()?;
                arguments = self.value_list(function, values)?;
                if opcode == "extract_lane" {
                    Operation::ExtractLane(lane)
                } else {
                    Operation::InsertLane(lane)
                }
            }
            "stack_slot" => {
                self.tokens.expect_word("size")?;
                let size = self.tokens.small()?;
                self.tokens.expect_word("align")?;
                let align = self.tokens.small()?;
                Operation::StackSlot { size, align }
            }
            "dynamic_stack_slot" => {
                self.tokens.expect_word("align")?;
                let align = self.tokens.small()?;
                arguments = self.value_list(function, values)?;
                Operation::DynamicStackSlot { align }
            }
            "stack_save" => Operation::StackSave,
            "stack_restore" => {
                arguments = self.value_list(function, values)?;
                Operation::StackRestore
            }
            "va_start" => {
                arguments = self.value_list(function, values)?;
                Operation::VaStart
            }
            "va_arg" => {
                arguments = self.value_list(function, values)?;
                Operation::VaArg
            }
            "va_copy" => {
                arguments = self.value_list(function, values)?;
                Operation::VaCopy
            }
            "load" | "store" => {
                let (align, volatile, ordering) = self.memory()?;
                arguments = self.value_list(function, values)?;
                if opcode == "load" {
                    Operation::Load {
                        align,
                        volatile,
                        ordering,
                    }
                } else {
                    Operation::Store {
                        align,
                        volatile,
                        ordering,
                    }
                }
            }
            "offset" => {
                arguments = self.value_list(function, values)?;
                Operation::Offset
            }
            "field_address" => {
                let structure = self.struct_ref()?;
                let index = self.tokens.small()?;
                arguments = self.value_list(function, values)?;
                Operation::FieldAddress(structure, index)
            }
            "element_address" => {
                let ty = self.tokens.ty()?;
                arguments = self.value_list(function, values)?;
                Operation::ElementAddress(ty)
            }
            "atomic_update" => {
                let op = self.tokens.named(AtomicOp::named)?;
                let ordering = self.tokens.named(Ordering::named)?;
                arguments = self.value_list(function, values)?;
                Operation::AtomicUpdate(op, ordering)
            }
            "compare_exchange" => {
                let success = self.tokens.named(Ordering::named)?;
                let failure = self.tokens.named(Ordering::named)?;
                arguments = self.value_list(function, values)?;
                Operation::CompareExchange(success, failure)
            }
            "fence" => Operation::Fence(self.tokens.named(Ordering::named)?),
            "memory_copy" | "memory_move" | "memory_fill" => {
                arguments = self.value_list(function, values)?;
                match opcode {
                    "memory_copy" => Operation::MemoryCopy,
                    "memory_move" => Operation::MemoryMove,
                    _ => Operation::MemoryFill,
                }
            }
            "call" => {
                let callee = self.function_ref()?;
                self.tokens.expect_punct('(')?;
                arguments = self.value_list(function, values)?;
                self.tokens.expect_punct(')')?;
                Operation::Call(callee)
            }
            "call_indirect" => {
                let signature = self.signature()?;
                let callee = self.value(function, values)?;
                self.tokens.expect_punct('(')?;
                arguments = vec![callee];
                arguments.extend(self.value_list(function, values)?);
                self.tokens.expect_punct(')')?;
                Operation::CallIndirect(signature)
            }
            "allocate" => Operation::Allocate(self.class_ref()?),
            "allocate_array" => {
                let class = self.class_ref()?;
                arguments = self.value_list(function, values)?;
                Operation::AllocateArray(class)
            }
            "load_field" | "store_field" | "virtual_method" | "interface_method" => {
                let class = self.class_ref()?;
                let index = self.tokens.small()?;
                arguments = self.value_list(function, values)?;
                match opcode {
                    "load_field" => Operation::LoadField(class, index),
                    "store_field" => Operation::StoreField(class, index),
                    "virtual_method" => Operation::VirtualMethod(class, index),
                    _ => Operation::InterfaceMethod(class, index),
                }
            }
            "load_element" | "store_element" => {
                let ty = self.tokens.ty()?;
                arguments = self.value_list(function, values)?;
                if opcode == "load_element" {
                    Operation::LoadElement(ty)
                } else {
                    Operation::StoreElement(ty)
                }
            }
            "array_length" => {
                arguments = self.value_list(function, values)?;
                Operation::ArrayLength
            }
            "instance_of" => {
                let class = self.class_ref()?;
                arguments = self.value_list(function, values)?;
                Operation::InstanceOf(class)
            }
            "same_object" => {
                arguments = self.value_list(function, values)?;
                Operation::SameObject
            }
            "trap" => Operation::Trap(self.tokens.small()?),
            other => {
                let operation = if let Some(op) = BinaryOp::named(other) {
                    Operation::Binary(op)
                } else if let Some(op) = UnaryOp::named(other) {
                    Operation::Unary(op)
                } else if let Some(conversion) = Conversion::named(other) {
                    Operation::Convert(conversion)
                } else {
                    return self.tokens.fault("unknown-operation", other);
                };
                arguments = self.value_list(function, values)?;
                operation
            }
        };
        Ok((operation, arguments))
    }

    fn spot(&mut self) -> Parsed<Option<SourceSpot>> {
        let marked = self.tokens.peek() == Some(&Token::Punct('!'))
            && matches!(self.tokens.peek_second(), Some(Token::Word(word)) if word == "loc");
        if !marked {
            return Ok(None);
        }
        self.tokens.next();
        self.tokens.next();
        let file = self.tokens.small()?;
        self.tokens.expect_punct(':')?;
        let line = self.tokens.small()?;
        self.tokens.expect_punct(':')?;
        let column = self.tokens.small()?;
        Ok(Some(SourceSpot { file, line, column }))
    }

    fn terminator(
        &mut self,
        opcode: &str,
        function: &mut Function,
        values: &mut BodyValues,
        referenced: &mut BTreeMap<u32, usize>,
    ) -> Parsed<Terminator> {
        Ok(match opcode {
            "jump" => Terminator::Jump(self.block_call(function, values, referenced)?),
            "branch" => {
                let condition = self.value(function, values)?;
                self.tokens.expect_punct(',')?;
                let taken = self.block_call(function, values, referenced)?;
                self.tokens.expect_punct(',')?;
                let otherwise = self.block_call(function, values, referenced)?;
                Terminator::Branch {
                    condition,
                    taken,
                    otherwise,
                }
            }
            "switch" => {
                let value = self.value(function, values)?;
                self.tokens.expect_punct(',')?;
                self.tokens.expect_punct('[')?;
                let mut cases = Vec::new();
                if !self.tokens.eat_punct(']') {
                    loop {
                        let case = self.tokens.number()?;
                        self.tokens.expect_punct(':')?;
                        cases.push((case, self.block_call(function, values, referenced)?));
                        if self.tokens.eat_punct(']') {
                            break;
                        }
                        self.tokens.expect_punct(',')?;
                    }
                }
                self.tokens.expect_punct(',')?;
                self.tokens.expect_word("default")?;
                let default = self.block_call(function, values, referenced)?;
                Terminator::Switch {
                    value,
                    cases,
                    default,
                }
            }
            "return" => {
                if matches!(self.tokens.peek(), Some(Token::Local(_))) {
                    Terminator::Return(Some(self.value(function, values)?))
                } else {
                    Terminator::Return(None)
                }
            }
            "throw" => Terminator::Throw(self.value(function, values)?),
            _ => Terminator::Unreachable,
        })
    }

    fn body(&mut self, function: &mut Function) -> Parsed<()> {
        let mut values = BodyValues {
            names: BTreeMap::new(),
            defined: BTreeSet::new(),
        };
        let mut referenced: BTreeMap<u32, usize> = BTreeMap::new();
        let mut expected_block = 0u32;
        while !self.tokens.eat_punct('}') {
            let number = self.block_number()?;
            if number != expected_block {
                return self.tokens.fault("block-order", format!("block{number}"));
            }
            expected_block += 1;
            let mut parameters = Vec::new();
            if self.tokens.eat_punct('(') {
                loop {
                    let name = self.tokens.local_name()?;
                    self.tokens.expect_punct(':')?;
                    let ty = self.tokens.ty()?;
                    parameters.push(self.define(function, &mut values, name, ty)?);
                    if self.tokens.eat_punct(')') {
                        break;
                    }
                    self.tokens.expect_punct(',')?;
                }
            }
            self.tokens.expect_punct(':')?;
            let mut instructions = Vec::new();
            let terminator = loop {
                let mut result = None;
                if matches!(self.tokens.peek(), Some(Token::Local(_))) {
                    let name = self.tokens.local_name()?;
                    self.tokens.expect_punct(':')?;
                    let ty = self.tokens.ty()?;
                    self.tokens.expect_punct('=')?;
                    result = Some((name, ty));
                }
                let opcode = self.tokens.word()?;
                match opcode.as_str() {
                    "jump" | "branch" | "switch" | "return" | "unreachable" | "throw"
                        if result.is_none() =>
                    {
                        break self.terminator(&opcode, function, &mut values, &mut referenced)?;
                    }
                    "invoke" => {
                        let inner = self.tokens.word()?;
                        if inner != "call" && inner != "call_indirect" {
                            return self.tokens.mismatch("call", Some(Token::Word(inner)));
                        }
                        let (operation, arguments) =
                            self.operation(function, &mut values, &inner)?;
                        let result = match result {
                            Some((name, ty)) => {
                                Some(self.define(function, &mut values, name, ty)?)
                            }
                            None => None,
                        };
                        let spot = self.spot()?;
                        self.tokens.expect_word("to")?;
                        let normal = self.block_call(function, &mut values, &mut referenced)?;
                        self.tokens.expect_word("unwind")?;
                        let unwind = self.block_call(function, &mut values, &mut referenced)?;
                        break Terminator::Invoke {
                            call: Instruction {
                                operation,
                                arguments,
                                result,
                                spot,
                            },
                            normal,
                            unwind,
                        };
                    }
                    _ => {
                        let (operation, arguments) =
                            self.operation(function, &mut values, &opcode)?;
                        let result = match result {
                            Some((name, ty)) => {
                                Some(self.define(function, &mut values, name, ty)?)
                            }
                            None => None,
                        };
                        let spot = self.spot()?;
                        function.instructions.push(Instruction {
                            operation,
                            arguments,
                            result,
                            spot,
                        });
                        instructions.push(Inst(function.instructions.len() as u32 - 1));
                    }
                }
            };
            function.blocks.push(BlockData {
                parameters,
                instructions,
                terminator,
            });
        }
        if let Some((number, line)) = referenced
            .iter()
            .find(|(number, _)| **number >= expected_block)
        {
            return read_error(*line, "undefined-block", format!("block{number}"));
        }
        if let Some((name, _)) = values
            .names
            .iter()
            .find(|(_, value)| !values.defined.contains(value))
        {
            return self.tokens.fault("undefined-value", symbol('%', name));
        }
        Ok(())
    }

    fn function_list(&mut self) -> Parsed<Vec<Option<FunctionId>>> {
        self.tokens.expect_punct('[')?;
        let mut list = Vec::new();
        if self.tokens.eat_punct(']') {
            return Ok(list);
        }
        loop {
            if self.tokens.eat_punct('-') {
                list.push(None);
            } else {
                list.push(Some(self.function_ref()?));
            }
            if self.tokens.eat_punct(']') {
                break;
            }
            self.tokens.expect_punct(',')?;
        }
        Ok(list)
    }

    fn type_list(&mut self, closing: char) -> Parsed<Vec<Type>> {
        let mut types = Vec::new();
        if self.tokens.eat_punct(closing) {
            return Ok(types);
        }
        loop {
            types.push(self.tokens.ty()?);
            if self.tokens.eat_punct(closing) {
                break;
            }
            self.tokens.expect_punct(',')?;
        }
        Ok(types)
    }

    fn class(&mut self) -> Parsed<ClassType> {
        let name = self.tokens.local_name()?;
        let mut class = ClassType {
            name,
            ..ClassType::default()
        };
        class.interface = self.tokens.eat_word("interface");
        class.abstract_class = self.tokens.eat_word("abstract");
        if self.tokens.eat_word("array") {
            class.element = Some(self.tokens.ty()?);
        }
        if self.tokens.eat_punct(':') {
            class.parent = Some(self.class_ref()?);
        }
        self.tokens.expect_punct('{')?;
        self.tokens.expect_word("fields")?;
        self.tokens.expect_punct('[')?;
        class.fields = self.type_list(']')?;
        self.tokens.expect_word("methods")?;
        class.methods = self.function_list()?;
        while self.tokens.eat_word("implements") {
            let interface = self.class_ref()?;
            let slots = self.function_list()?;
            class.implementations.push((interface, slots));
        }
        self.tokens.expect_punct('}')?;
        Ok(class)
    }

    fn global(&mut self) -> Parsed<Global> {
        let name = self.tokens.global_name()?;
        self.tokens.expect_word("size")?;
        let size = self.tokens.small()?;
        self.tokens.expect_word("align")?;
        let align = self.tokens.small()?;
        let mutable = self.tokens.eat_word("mutable");
        let thread_local = self.tokens.eat_word("thread_local");
        let linkage = self.tokens.named(Linkage::named)?;
        let content = if self.tokens.eat_word("zero") {
            GlobalContent::Zero
        } else {
            self.tokens.expect_word("bytes")?;
            let hex = self.tokens.text()?;
            let digits: Option<Vec<u8>> = hex
                .chars()
                .map(|digit| digit.to_digit(16).map(|value| value as u8))
                .collect();
            match digits {
                Some(digits) if digits.len().is_multiple_of(2) => GlobalContent::Bytes(
                    digits
                        .chunks(2)
                        .map(|pair| (pair[0] << 4) | pair[1])
                        .collect(),
                ),
                _ => return self.tokens.fault("invalid-hex", quote(&hex)),
            }
        };
        let mut relocations = Vec::new();
        while self.tokens.eat_word("relocate") {
            let offset = self.tokens.small()?;
            let kind = self.tokens.word()?;
            let target = match kind.as_str() {
                "global" => Symbol::Global(self.global_ref()?),
                "function" => Symbol::Function(self.function_ref()?),
                _ => return self.tokens.fault("unknown-keyword", kind),
            };
            let addend = self.tokens.number()?;
            let Ok(addend) = i64::try_from(addend) else {
                return self.tokens.fault("number-too-large", addend.to_string());
            };
            relocations.push(Relocation {
                offset,
                target,
                addend,
            });
        }
        Ok(Global {
            name,
            size,
            align,
            mutable,
            thread_local,
            linkage,
            content,
            relocations,
        })
    }

    fn function(&mut self) -> Parsed<Function> {
        let name = self.tokens.global_name()?;
        let signature = self.signature()?;
        let linkage = self.tokens.named(Linkage::named)?;
        let mut function = Function::new(&name, signature, linkage);
        while let Some(Token::Word(word)) = self.tokens.peek() {
            let Some(attribute) = Attribute::named(word) else {
                break;
            };
            function.attributes.insert(attribute);
            self.tokens.next();
        }
        if self.tokens.eat_punct('{') {
            self.body(&mut function)?;
        }
        Ok(function)
    }

    fn module(&mut self) -> Parsed<Module> {
        self.tokens.expect_word("ir")?;
        let version = self.tokens.number()?;
        if version != i128::from(FORMAT_VERSION) {
            return self.tokens.fault("unknown-version", version.to_string());
        }
        self.tokens.expect_word("module")?;
        let mut module = Module::new(&self.tokens.text()?);
        self.tokens.expect_word("layout")?;
        module.layout = self.tokens.named(Layout::named)?;
        while let Some(token) = self.tokens.next() {
            let Token::Word(keyword) = token else {
                return self.tokens.mismatch("function", Some(token));
            };
            match keyword.as_str() {
                "file" => module.files.push(self.tokens.text()?),
                "string" => module.strings.push(self.tokens.text()?),
                "struct" => {
                    let name = self.tokens.local_name()?;
                    let packed = self.tokens.eat_word("packed");
                    self.tokens.expect_punct('{')?;
                    let fields = self.type_list('}')?;
                    module.structs.push(StructType {
                        name,
                        fields,
                        packed,
                    });
                }
                "class" => {
                    let class = self.class()?;
                    module.classes.push(class);
                }
                "global" => {
                    let global = self.global()?;
                    module.globals.push(global);
                }
                "function" => {
                    let function = self.function()?;
                    module.functions.push(function);
                }
                other => return self.tokens.fault("unknown-keyword", other),
            }
        }
        Ok(module)
    }
}

impl Module {
    pub fn from_text(text: &str) -> Result<Module, Diagnostic> {
        let wrap = |error: ReadError| {
            let mut diagnostic = fail("EI001", "The intermediate text could not be read.")
                .with_context(format!("Line: {}", error.line))
                .with_context(format!("Reason: {}", error.reason));
            if !error.expected.is_empty() {
                diagnostic = diagnostic.with_context(format!("Expected: {}", error.expected));
            }
            if !error.found.is_empty() {
                diagnostic = diagnostic.with_context(format!("Found: {}", error.found));
            }
            diagnostic
        };
        let tokens = tokenize(text).map_err(wrap)?;
        let names = collect_names(&tokens).map_err(wrap)?;
        let mut reader = TextReader {
            tokens: Tokens {
                items: tokens,
                at: 0,
            },
            names,
        };
        let module = reader.module().map_err(wrap)?;
        Ok(module.canonical())
    }
}

const MAGIC: &[u8; 4] = b"CBIR";

struct Encoder {
    bytes: Vec<u8>,
}

impl Encoder {
    fn byte(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn unsigned(&mut self, value: u64) {
        let mut rest = value;
        loop {
            let low = (rest & 0x7f) as u8;
            rest >>= 7;
            if rest == 0 {
                self.bytes.push(low);
                return;
            }
            self.bytes.push(low | 0x80);
        }
    }

    fn signed(&mut self, value: i128) {
        let mut rest = value;
        loop {
            let low = (rest & 0x7f) as u8;
            rest >>= 7;
            let done = (rest == 0 && low & 0x40 == 0) || (rest == -1 && low & 0x40 != 0);
            if done {
                self.bytes.push(low);
                return;
            }
            self.bytes.push(low | 0x80);
        }
    }

    fn index(&mut self, value: usize) {
        self.unsigned(value as u64);
    }

    fn text(&mut self, value: &str) {
        self.index(value.len());
        self.bytes.extend_from_slice(value.as_bytes());
    }

    fn ty(&mut self, ty: Type) {
        match ty {
            Type::Scalar(scalar) => self.byte(scalar.code()),
            Type::Vector { lanes, element } => {
                self.byte(0x40 | element.code());
                self.byte(lanes);
            }
        }
    }

    fn types(&mut self, types: &[Type]) {
        self.index(types.len());
        for ty in types {
            self.ty(*ty);
        }
    }

    fn signature(&mut self, signature: &Signature) {
        let mut flags = 0u8;
        if signature.result.is_some() {
            flags |= 1;
        }
        if signature.variadic {
            flags |= 2;
        }
        if signature.convention.is_some() {
            flags |= 4;
        }
        self.byte(flags);
        if let Some(result) = signature.result {
            self.ty(result);
        }
        if let Some(convention) = signature.convention {
            self.byte(convention.code());
        }
        self.types(&signature.parameters);
    }

    fn slots(&mut self, slots: &[Option<FunctionId>]) {
        self.index(slots.len());
        for slot in slots {
            match slot {
                Some(id) => self.unsigned(u64::from(id.0) + 1),
                None => self.unsigned(0),
            }
        }
    }

    fn ordering(&mut self, ordering: Option<Ordering>) {
        match ordering {
            Some(ordering) => self.byte(ordering.code() + 1),
            None => self.byte(0),
        }
    }
}

struct BodyWriter<'a> {
    numbers: BTreeMap<Value, u64>,
    function: &'a Function,
}

impl BodyWriter<'_> {
    fn value(&self, out: &mut Encoder, value: Value) {
        out.unsigned(self.numbers.get(&value).copied().unwrap_or(u64::MAX));
    }

    fn values(&self, out: &mut Encoder, values: &[Value]) {
        out.index(values.len());
        for value in values {
            self.value(out, *value);
        }
    }

    fn block_call(&self, out: &mut Encoder, call: &BlockCall) {
        out.unsigned(u64::from(call.block.0));
        self.values(out, &call.arguments);
    }

    fn operation(&self, out: &mut Encoder, operation: &Operation) {
        match operation {
            Operation::IntegerConstant(value) => {
                out.byte(0);
                out.signed(*value);
            }
            Operation::FloatConstant(bits) => {
                out.byte(1);
                out.unsigned(*bits);
            }
            Operation::Null => out.byte(2),
            Operation::Poison => out.byte(3),
            Operation::GlobalAddress(id) => {
                out.byte(4);
                out.unsigned(u64::from(id.0));
            }
            Operation::FunctionAddress(id) => {
                out.byte(5);
                out.unsigned(u64::from(id.0));
            }
            Operation::StringConstant(index) => {
                out.byte(6);
                out.unsigned(u64::from(*index));
            }
            Operation::Binary(op) => {
                out.byte(7);
                out.byte(op.code());
            }
            Operation::Unary(op) => {
                out.byte(8);
                out.byte(op.code());
            }
            Operation::Compare(comparison) => {
                out.byte(9);
                out.byte(comparison.code());
            }
            Operation::FloatCompare(comparison) => {
                out.byte(10);
                out.byte(comparison.code());
            }
            Operation::Convert(conversion) => {
                out.byte(11);
                out.byte(conversion.code());
            }
            Operation::Select => out.byte(12),
            Operation::Splat => out.byte(13),
            Operation::ExtractLane(lane) => {
                out.byte(14);
                out.byte(*lane);
            }
            Operation::InsertLane(lane) => {
                out.byte(15);
                out.byte(*lane);
            }
            Operation::StackSlot { size, align } => {
                out.byte(16);
                out.unsigned(u64::from(*size));
                out.unsigned(u64::from(*align));
            }
            Operation::Load {
                align,
                volatile,
                ordering,
            } => {
                out.byte(17);
                out.unsigned(u64::from(*align));
                out.byte(u8::from(*volatile));
                out.ordering(*ordering);
            }
            Operation::Store {
                align,
                volatile,
                ordering,
            } => {
                out.byte(18);
                out.unsigned(u64::from(*align));
                out.byte(u8::from(*volatile));
                out.ordering(*ordering);
            }
            Operation::Offset => out.byte(19),
            Operation::FieldAddress(structure, index) => {
                out.byte(20);
                out.unsigned(u64::from(structure.0));
                out.unsigned(u64::from(*index));
            }
            Operation::ElementAddress(ty) => {
                out.byte(21);
                out.ty(*ty);
            }
            Operation::AtomicUpdate(op, ordering) => {
                out.byte(22);
                out.byte(op.code());
                out.ordering(Some(*ordering));
            }
            Operation::CompareExchange(success, failure) => {
                out.byte(23);
                out.ordering(Some(*success));
                out.ordering(Some(*failure));
            }
            Operation::Fence(ordering) => {
                out.byte(24);
                out.ordering(Some(*ordering));
            }
            Operation::MemoryCopy => out.byte(25),
            Operation::MemoryMove => out.byte(26),
            Operation::MemoryFill => out.byte(27),
            Operation::Call(callee) => {
                out.byte(28);
                out.unsigned(u64::from(callee.0));
            }
            Operation::CallIndirect(signature) => {
                out.byte(29);
                out.signature(signature);
            }
            Operation::Allocate(class) => {
                out.byte(30);
                out.unsigned(u64::from(class.0));
            }
            Operation::AllocateArray(class) => {
                out.byte(31);
                out.unsigned(u64::from(class.0));
            }
            Operation::LoadField(class, index) => {
                out.byte(32);
                out.unsigned(u64::from(class.0));
                out.unsigned(u64::from(*index));
            }
            Operation::StoreField(class, index) => {
                out.byte(33);
                out.unsigned(u64::from(class.0));
                out.unsigned(u64::from(*index));
            }
            Operation::LoadElement(ty) => {
                out.byte(34);
                out.ty(*ty);
            }
            Operation::StoreElement(ty) => {
                out.byte(35);
                out.ty(*ty);
            }
            Operation::ArrayLength => out.byte(36),
            Operation::InstanceOf(class) => {
                out.byte(37);
                out.unsigned(u64::from(class.0));
            }
            Operation::VirtualMethod(class, slot) => {
                out.byte(38);
                out.unsigned(u64::from(class.0));
                out.unsigned(u64::from(*slot));
            }
            Operation::InterfaceMethod(class, slot) => {
                out.byte(39);
                out.unsigned(u64::from(class.0));
                out.unsigned(u64::from(*slot));
            }
            Operation::SameObject => out.byte(40),
            Operation::Trap(code) => {
                out.byte(41);
                out.unsigned(u64::from(*code));
            }
            Operation::DynamicStackSlot { align } => {
                out.byte(42);
                out.unsigned(u64::from(*align));
            }
            Operation::StackSave => out.byte(43),
            Operation::StackRestore => out.byte(44),
            Operation::VaStart => out.byte(45),
            Operation::VaArg => out.byte(46),
            Operation::VaCopy => out.byte(47),
        }
    }

    fn instruction(&self, out: &mut Encoder, instruction: &Instruction) {
        self.operation(out, &instruction.operation);
        let mut flags = 0u8;
        if instruction.result.is_some() {
            flags |= 1;
        }
        if instruction.spot.is_some() {
            flags |= 2;
        }
        out.byte(flags);
        if let Some(result) = instruction.result {
            out.ty(self
                .function
                .value_types
                .get(result.index())
                .copied()
                .unwrap_or(Type::BOOL));
        }
        self.values(out, &instruction.arguments);
        if let Some(spot) = &instruction.spot {
            out.unsigned(u64::from(spot.file));
            out.unsigned(u64::from(spot.line));
            out.unsigned(u64::from(spot.column));
        }
    }

    fn terminator(&self, out: &mut Encoder, terminator: &Terminator) {
        match terminator {
            Terminator::Jump(call) => {
                out.byte(0);
                self.block_call(out, call);
            }
            Terminator::Branch {
                condition,
                taken,
                otherwise,
            } => {
                out.byte(1);
                self.value(out, *condition);
                self.block_call(out, taken);
                self.block_call(out, otherwise);
            }
            Terminator::Switch {
                value,
                cases,
                default,
            } => {
                out.byte(2);
                self.value(out, *value);
                out.index(cases.len());
                for (case, call) in cases {
                    out.signed(*case);
                    self.block_call(out, call);
                }
                self.block_call(out, default);
            }
            Terminator::Return(None) => out.byte(3),
            Terminator::Return(Some(value)) => {
                out.byte(4);
                self.value(out, *value);
            }
            Terminator::Unreachable => out.byte(5),
            Terminator::Throw(value) => {
                out.byte(6);
                self.value(out, *value);
            }
            Terminator::Invoke {
                call,
                normal,
                unwind,
            } => {
                out.byte(7);
                self.instruction(out, call);
                self.block_call(out, normal);
                self.block_call(out, unwind);
            }
        }
    }
}

fn used_values(function: &Function) -> Vec<Value> {
    let mut used = Vec::new();
    for data in &function.blocks {
        for inst in &data.instructions {
            if let Some(instruction) = function.instructions.get(inst.index()) {
                used.extend(instruction.arguments.iter().copied());
            }
        }
        match &data.terminator {
            Terminator::Branch { condition, .. } => used.push(*condition),
            Terminator::Switch { value, .. } => used.push(*value),
            Terminator::Return(Some(value)) | Terminator::Throw(value) => used.push(*value),
            Terminator::Invoke { call, .. } => used.extend(call.arguments.iter().copied()),
            _ => {}
        }
        for edge in data.terminator.successors() {
            used.extend(edge.arguments.iter().copied());
        }
    }
    used
}

fn defined_values(function: &Function) -> Vec<Value> {
    let mut defined = Vec::new();
    for data in &function.blocks {
        defined.extend(data.parameters.iter().copied());
        for inst in &data.instructions {
            if let Some(result) = function
                .instructions
                .get(inst.index())
                .and_then(|instruction| instruction.result)
            {
                defined.push(result);
            }
        }
        if let Terminator::Invoke { call, .. } = &data.terminator {
            if let Some(result) = call.result {
                defined.push(result);
            }
        }
    }
    defined
}

fn value_numbers(function: &Function) -> (BTreeMap<Value, u64>, Vec<Value>) {
    let mut numbers = BTreeMap::new();
    for value in defined_values(function) {
        let next = numbers.len() as u64;
        numbers.entry(value).or_insert(next);
    }
    let mut loose = Vec::new();
    for value in used_values(function) {
        if !numbers.contains_key(&value) {
            let next = numbers.len() as u64;
            numbers.insert(value, next);
            loose.push(value);
        }
    }
    (numbers, loose)
}

impl Function {
    pub fn canonical(&self) -> Function {
        let (numbers, _) = value_numbers(self);
        let mut ordered: Vec<(u64, Value)> = numbers
            .iter()
            .map(|(value, number)| (*number, *value))
            .collect();
        ordered.sort();
        let renamed = |value: Value| -> Value {
            Value(numbers.get(&value).copied().unwrap_or(u64::from(u32::MAX)) as u32)
        };
        let mut out = Function::new(&self.name, self.signature.clone(), self.linkage);
        out.attributes = self.attributes.clone();
        out.value_types = ordered
            .iter()
            .map(|(_, value)| {
                self.value_types
                    .get(value.index())
                    .copied()
                    .unwrap_or(Type::BOOL)
            })
            .collect();
        let rename_call = |call: &BlockCall| BlockCall {
            block: call.block,
            arguments: call.arguments.iter().map(|value| renamed(*value)).collect(),
        };
        let rename_instruction = |instruction: &Instruction| Instruction {
            operation: instruction.operation.clone(),
            arguments: instruction
                .arguments
                .iter()
                .map(|value| renamed(*value))
                .collect(),
            result: instruction.result.map(renamed),
            spot: instruction.spot.clone(),
        };
        for data in &self.blocks {
            let mut instructions = Vec::with_capacity(data.instructions.len());
            for inst in &data.instructions {
                if let Some(instruction) = self.instructions.get(inst.index()) {
                    out.instructions.push(rename_instruction(instruction));
                    instructions.push(Inst(out.instructions.len() as u32 - 1));
                }
            }
            let terminator = match &data.terminator {
                Terminator::Jump(call) => Terminator::Jump(rename_call(call)),
                Terminator::Branch {
                    condition,
                    taken,
                    otherwise,
                } => Terminator::Branch {
                    condition: renamed(*condition),
                    taken: rename_call(taken),
                    otherwise: rename_call(otherwise),
                },
                Terminator::Switch {
                    value,
                    cases,
                    default,
                } => Terminator::Switch {
                    value: renamed(*value),
                    cases: cases
                        .iter()
                        .map(|(case, call)| (*case, rename_call(call)))
                        .collect(),
                    default: rename_call(default),
                },
                Terminator::Return(value) => Terminator::Return(value.map(renamed)),
                Terminator::Unreachable => Terminator::Unreachable,
                Terminator::Throw(value) => Terminator::Throw(renamed(*value)),
                Terminator::Invoke {
                    call,
                    normal,
                    unwind,
                } => Terminator::Invoke {
                    call: rename_instruction(call),
                    normal: rename_call(normal),
                    unwind: rename_call(unwind),
                },
            };
            out.blocks.push(BlockData {
                parameters: data
                    .parameters
                    .iter()
                    .map(|value| renamed(*value))
                    .collect(),
                instructions,
                terminator,
            });
        }
        out
    }
}

impl Module {
    pub fn canonical(&self) -> Module {
        let mut out = self.clone();
        out.functions = self.functions.iter().map(Function::canonical).collect();
        out
    }

    pub fn to_binary(&self) -> Vec<u8> {
        let mut out = Encoder { bytes: Vec::new() };
        out.bytes.extend_from_slice(MAGIC);
        out.unsigned(u64::from(FORMAT_VERSION));
        out.text(&self.name);
        out.byte(self.layout.code());
        out.index(self.files.len());
        for file in &self.files {
            out.text(file);
        }
        out.index(self.strings.len());
        for text in &self.strings {
            out.text(text);
        }
        out.index(self.structs.len());
        out.index(self.classes.len());
        out.index(self.globals.len());
        out.index(self.functions.len());
        for structure in &self.structs {
            out.text(&structure.name);
            out.byte(u8::from(structure.packed));
            out.types(&structure.fields);
        }
        for class in &self.classes {
            out.text(&class.name);
            let mut flags = 0u8;
            if class.interface {
                flags |= 1;
            }
            if class.abstract_class {
                flags |= 2;
            }
            if class.parent.is_some() {
                flags |= 4;
            }
            if class.element.is_some() {
                flags |= 8;
            }
            out.byte(flags);
            if let Some(parent) = class.parent {
                out.unsigned(u64::from(parent.0));
            }
            if let Some(element) = class.element {
                out.ty(element);
            }
            out.types(&class.fields);
            out.slots(&class.methods);
            out.index(class.implementations.len());
            for (interface, slots) in &class.implementations {
                out.unsigned(u64::from(interface.0));
                out.slots(slots);
            }
        }
        for global in &self.globals {
            out.text(&global.name);
            out.unsigned(u64::from(global.size));
            out.unsigned(u64::from(global.align));
            let mut flags = 0u8;
            if global.mutable {
                flags |= 1;
            }
            if global.thread_local {
                flags |= 2;
            }
            if matches!(global.content, GlobalContent::Bytes(_)) {
                flags |= 4;
            }
            out.byte(flags);
            out.byte(global.linkage.code());
            if let GlobalContent::Bytes(bytes) = &global.content {
                out.index(bytes.len());
                out.bytes.extend_from_slice(bytes);
            }
            out.index(global.relocations.len());
            for relocation in &global.relocations {
                out.unsigned(u64::from(relocation.offset));
                match relocation.target {
                    Symbol::Global(id) => {
                        out.byte(0);
                        out.unsigned(u64::from(id.0));
                    }
                    Symbol::Function(id) => {
                        out.byte(1);
                        out.unsigned(u64::from(id.0));
                    }
                }
                out.signed(i128::from(relocation.addend));
            }
        }
        for function in &self.functions {
            out.text(&function.name);
            out.signature(&function.signature);
            out.byte(function.linkage.code());
            let mut attributes = 0u64;
            for attribute in &function.attributes {
                attributes |= 1 << attribute.code();
            }
            out.unsigned(attributes);
            out.index(function.blocks.len());
            if function.blocks.is_empty() {
                continue;
            }
            let (numbers, loose) = value_numbers(function);
            let writer = BodyWriter { numbers, function };
            for data in &function.blocks {
                let parameter_types: Vec<Type> = data
                    .parameters
                    .iter()
                    .map(|value| {
                        function
                            .value_types
                            .get(value.index())
                            .copied()
                            .unwrap_or(Type::BOOL)
                    })
                    .collect();
                out.types(&parameter_types);
                out.index(data.instructions.len());
                for inst in &data.instructions {
                    match function.instructions.get(inst.index()) {
                        Some(instruction) => writer.instruction(&mut out, instruction),
                        None => writer.instruction(
                            &mut out,
                            &Instruction {
                                operation: Operation::Poison,
                                arguments: Vec::new(),
                                result: None,
                                spot: None,
                            },
                        ),
                    }
                }
                writer.terminator(&mut out, &data.terminator);
            }
            let loose_types: Vec<Type> = loose
                .iter()
                .map(|value| {
                    function
                        .value_types
                        .get(value.index())
                        .copied()
                        .unwrap_or(Type::BOOL)
                })
                .collect();
            out.types(&loose_types);
        }
        out.bytes
    }

    pub fn from_binary(bytes: &[u8]) -> Result<Module, Diagnostic> {
        let mut reader = Decoder { bytes, at: 0 };
        reader.module().map_err(|(offset, reason)| {
            fail("EI002", "The intermediate binary could not be read.")
                .with_context(format!("Offset: {offset}"))
                .with_context(format!("Reason: {reason}"))
        })
    }
}

type Decoded<T> = Result<T, (usize, &'static str)>;

struct Decoder<'b> {
    bytes: &'b [u8],
    at: usize,
}

struct Counts {
    files: usize,
    strings: usize,
    structs: usize,
    classes: usize,
    globals: usize,
    functions: usize,
}

impl Decoder<'_> {
    fn fault<T>(&self, reason: &'static str) -> Decoded<T> {
        Err((self.at, reason))
    }

    fn byte(&mut self) -> Decoded<u8> {
        match self.bytes.get(self.at) {
            Some(byte) => {
                self.at += 1;
                Ok(*byte)
            }
            None => self.fault("truncated"),
        }
    }

    fn unsigned(&mut self) -> Decoded<u64> {
        let start = self.at;
        let mut value = 0u64;
        let mut shift = 0u32;
        loop {
            let byte = self.byte()?;
            if shift >= 64 {
                self.at = start;
                return self.fault("number-too-large");
            }
            value |= u64::from(byte & 0x7f).checked_shl(shift).unwrap_or(0);
            shift += 7;
            if byte & 0x80 == 0 {
                break;
            }
        }
        let mut check = Encoder { bytes: Vec::new() };
        check.unsigned(value);
        if check.bytes.as_slice() != &self.bytes[start..self.at] {
            self.at = start;
            return self.fault("overlong-number");
        }
        Ok(value)
    }

    fn signed(&mut self) -> Decoded<i128> {
        let start = self.at;
        let mut value = 0i128;
        let mut shift = 0u32;
        loop {
            let byte = self.byte()?;
            if shift >= 133 {
                self.at = start;
                return self.fault("number-too-large");
            }
            if shift < 128 {
                value |= i128::from(byte & 0x7f) << shift;
            }
            shift += 7;
            if byte & 0x80 == 0 {
                if shift < 128 && byte & 0x40 != 0 {
                    value |= -1i128 << shift;
                }
                break;
            }
        }
        let mut check = Encoder { bytes: Vec::new() };
        check.signed(value);
        if check.bytes.as_slice() != &self.bytes[start..self.at] {
            self.at = start;
            return self.fault("overlong-number");
        }
        Ok(value)
    }

    fn small(&mut self) -> Decoded<u32> {
        let start = self.at;
        let value = self.unsigned()?;
        u32::try_from(value).or_else(|_| {
            self.at = start;
            self.fault("number-too-large")
        })
    }

    fn count(&mut self) -> Decoded<usize> {
        let start = self.at;
        let value = self.unsigned()?;
        if value > (self.bytes.len() - self.at) as u64 {
            self.at = start;
            return self.fault("count-too-large");
        }
        Ok(value as usize)
    }

    fn index(&mut self, limit: usize) -> Decoded<u32> {
        let start = self.at;
        let value = self.unsigned()?;
        if value >= limit as u64 {
            self.at = start;
            return self.fault("index-out-of-range");
        }
        Ok(value as u32)
    }

    fn text(&mut self) -> Decoded<String> {
        let length = self.count()?;
        let start = self.at;
        let slice = &self.bytes[start..start + length];
        match std::str::from_utf8(slice) {
            Ok(text) => {
                self.at += length;
                Ok(text.to_string())
            }
            Err(_) => self.fault("invalid-text"),
        }
    }

    fn flags(&mut self, allowed: u8) -> Decoded<u8> {
        let start = self.at;
        let flags = self.byte()?;
        if flags & !allowed != 0 {
            self.at = start;
            return self.fault("unknown-flags");
        }
        Ok(flags)
    }

    fn ty(&mut self) -> Decoded<Type> {
        let start = self.at;
        let code = self.byte()?;
        let ty = if code & 0x40 != 0 {
            let lanes = self.byte()?;
            Scalar::from_code(code & !0x40).and_then(|element| Type::vector(lanes, element))
        } else {
            Scalar::from_code(code).map(Type::Scalar)
        };
        ty.map_or_else(
            || {
                self.at = start;
                self.fault("unknown-type")
            },
            Ok,
        )
    }

    fn types(&mut self) -> Decoded<Vec<Type>> {
        let count = self.count()?;
        let mut types = Vec::with_capacity(count);
        for _ in 0..count {
            types.push(self.ty()?);
        }
        Ok(types)
    }

    fn code<T>(&mut self, lookup: fn(u8) -> Option<T>) -> Decoded<T> {
        let start = self.at;
        let code = self.byte()?;
        lookup(code).map_or_else(
            || {
                self.at = start;
                self.fault("unknown-code")
            },
            Ok,
        )
    }

    fn signature(&mut self) -> Decoded<Signature> {
        let flags = self.flags(7)?;
        let result = if flags & 1 != 0 {
            Some(self.ty()?)
        } else {
            None
        };
        let convention = if flags & 4 != 0 {
            Some(self.code(Convention::from_code)?)
        } else {
            None
        };
        let parameters = self.types()?;
        Ok(Signature {
            parameters,
            result,
            convention,
            variadic: flags & 2 != 0,
        })
    }

    fn slots(&mut self, functions: usize) -> Decoded<Vec<Option<FunctionId>>> {
        let count = self.count()?;
        let mut slots = Vec::with_capacity(count);
        for _ in 0..count {
            let slot = self.index(functions + 1)?;
            slots.push(if slot == 0 {
                None
            } else {
                Some(FunctionId(slot - 1))
            });
        }
        Ok(slots)
    }

    fn ordering(&mut self) -> Decoded<Option<Ordering>> {
        let start = self.at;
        let code = self.byte()?;
        if code == 0 {
            return Ok(None);
        }
        Ordering::from_code(code - 1).map_or_else(
            || {
                self.at = start;
                self.fault("unknown-code")
            },
            |ordering| Ok(Some(ordering)),
        )
    }

    fn required_ordering(&mut self) -> Decoded<Ordering> {
        let start = self.at;
        match self.ordering()? {
            Some(ordering) => Ok(ordering),
            None => {
                self.at = start;
                self.fault("unknown-code")
            }
        }
    }

    fn operation(&mut self, counts: &Counts) -> Decoded<Operation> {
        let start = self.at;
        let opcode = self.byte()?;
        Ok(match opcode {
            0 => Operation::IntegerConstant(self.signed()?),
            1 => Operation::FloatConstant(self.unsigned()?),
            2 => Operation::Null,
            3 => Operation::Poison,
            4 => Operation::GlobalAddress(GlobalId(self.index(counts.globals)?)),
            5 => Operation::FunctionAddress(FunctionId(self.index(counts.functions)?)),
            6 => Operation::StringConstant(self.index(counts.strings)?),
            7 => Operation::Binary(self.code(BinaryOp::from_code)?),
            8 => Operation::Unary(self.code(UnaryOp::from_code)?),
            9 => Operation::Compare(self.code(Comparison::from_code)?),
            10 => Operation::FloatCompare(self.code(FloatComparison::from_code)?),
            11 => Operation::Convert(self.code(Conversion::from_code)?),
            12 => Operation::Select,
            13 => Operation::Splat,
            14 => Operation::ExtractLane(self.byte()?),
            15 => Operation::InsertLane(self.byte()?),
            16 => Operation::StackSlot {
                size: self.small()?,
                align: self.small()?,
            },
            17 | 18 => {
                let align = self.small()?;
                let volatile = self.flags(1)? != 0;
                let ordering = self.ordering()?;
                if opcode == 17 {
                    Operation::Load {
                        align,
                        volatile,
                        ordering,
                    }
                } else {
                    Operation::Store {
                        align,
                        volatile,
                        ordering,
                    }
                }
            }
            19 => Operation::Offset,
            20 => Operation::FieldAddress(StructId(self.index(counts.structs)?), self.small()?),
            21 => Operation::ElementAddress(self.ty()?),
            22 => {
                Operation::AtomicUpdate(self.code(AtomicOp::from_code)?, self.required_ordering()?)
            }
            23 => Operation::CompareExchange(self.required_ordering()?, self.required_ordering()?),
            24 => Operation::Fence(self.required_ordering()?),
            25 => Operation::MemoryCopy,
            26 => Operation::MemoryMove,
            27 => Operation::MemoryFill,
            28 => Operation::Call(FunctionId(self.index(counts.functions)?)),
            29 => Operation::CallIndirect(self.signature()?),
            30 => Operation::Allocate(ClassId(self.index(counts.classes)?)),
            31 => Operation::AllocateArray(ClassId(self.index(counts.classes)?)),
            32 => Operation::LoadField(ClassId(self.index(counts.classes)?), self.small()?),
            33 => Operation::StoreField(ClassId(self.index(counts.classes)?), self.small()?),
            34 => Operation::LoadElement(self.ty()?),
            35 => Operation::StoreElement(self.ty()?),
            36 => Operation::ArrayLength,
            37 => Operation::InstanceOf(ClassId(self.index(counts.classes)?)),
            38 => Operation::VirtualMethod(ClassId(self.index(counts.classes)?), self.small()?),
            39 => Operation::InterfaceMethod(ClassId(self.index(counts.classes)?), self.small()?),
            40 => Operation::SameObject,
            41 => Operation::Trap(self.small()?),
            42 => Operation::DynamicStackSlot {
                align: self.small()?,
            },
            43 => Operation::StackSave,
            44 => Operation::StackRestore,
            45 => Operation::VaStart,
            46 => Operation::VaArg,
            47 => Operation::VaCopy,
            _ => {
                self.at = start;
                return self.fault("unknown-operation");
            }
        })
    }

    fn values(&mut self, references: &mut Vec<(usize, u64)>) -> Decoded<Vec<Value>> {
        let count = self.count()?;
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(self.value(references)?);
        }
        Ok(values)
    }

    fn value(&mut self, references: &mut Vec<(usize, u64)>) -> Decoded<Value> {
        let start = self.at;
        let number = self.unsigned()?;
        if number >= u64::from(u32::MAX) {
            self.at = start;
            return self.fault("index-out-of-range");
        }
        references.push((start, number));
        Ok(Value(number as u32))
    }

    fn block_call(
        &mut self,
        blocks: usize,
        references: &mut Vec<(usize, u64)>,
    ) -> Decoded<BlockCall> {
        let block = Block(self.index(blocks)?);
        let arguments = self.values(references)?;
        Ok(BlockCall { block, arguments })
    }

    fn instruction(
        &mut self,
        counts: &Counts,
        function: &mut Function,
        references: &mut Vec<(usize, u64)>,
    ) -> Decoded<Instruction> {
        let operation = self.operation(counts)?;
        let flags = self.flags(3)?;
        let result = if flags & 1 != 0 {
            let ty = self.ty()?;
            Some(function.new_value(ty))
        } else {
            None
        };
        let arguments = self.values(references)?;
        let spot = if flags & 2 != 0 {
            Some(SourceSpot {
                file: self.index(counts.files)?,
                line: self.small()?,
                column: self.small()?,
            })
        } else {
            None
        };
        Ok(Instruction {
            operation,
            arguments,
            result,
            spot,
        })
    }

    fn body(&mut self, counts: &Counts, function: &mut Function, blocks: usize) -> Decoded<()> {
        let mut references = Vec::new();
        for _ in 0..blocks {
            let parameter_types = self.types()?;
            let parameters: Vec<Value> = parameter_types
                .into_iter()
                .map(|ty| function.new_value(ty))
                .collect();
            let count = self.count()?;
            let mut instructions = Vec::with_capacity(count);
            for _ in 0..count {
                let instruction = self.instruction(counts, function, &mut references)?;
                function.instructions.push(instruction);
                instructions.push(Inst(function.instructions.len() as u32 - 1));
            }
            let start = self.at;
            let tag = self.byte()?;
            let terminator = match tag {
                0 => Terminator::Jump(self.block_call(blocks, &mut references)?),
                1 => Terminator::Branch {
                    condition: self.value(&mut references)?,
                    taken: self.block_call(blocks, &mut references)?,
                    otherwise: self.block_call(blocks, &mut references)?,
                },
                2 => {
                    let value = self.value(&mut references)?;
                    let count = self.count()?;
                    let mut cases = Vec::with_capacity(count);
                    for _ in 0..count {
                        let case = self.signed()?;
                        cases.push((case, self.block_call(blocks, &mut references)?));
                    }
                    let default = self.block_call(blocks, &mut references)?;
                    Terminator::Switch {
                        value,
                        cases,
                        default,
                    }
                }
                3 => Terminator::Return(None),
                4 => Terminator::Return(Some(self.value(&mut references)?)),
                5 => Terminator::Unreachable,
                6 => Terminator::Throw(self.value(&mut references)?),
                7 => {
                    let call = self.instruction(counts, function, &mut references)?;
                    let normal = self.block_call(blocks, &mut references)?;
                    let unwind = self.block_call(blocks, &mut references)?;
                    Terminator::Invoke {
                        call,
                        normal,
                        unwind,
                    }
                }
                _ => {
                    self.at = start;
                    return self.fault("unknown-terminator");
                }
            };
            function.blocks.push(BlockData {
                parameters,
                instructions,
                terminator,
            });
        }
        let loose = self.types()?;
        for ty in loose {
            function.new_value(ty);
        }
        let defined = function.value_types.len() as u64;
        if let Some((offset, _)) = references.iter().find(|(_, number)| *number >= defined) {
            return Err((*offset, "index-out-of-range"));
        }
        Ok(())
    }

    fn module(&mut self) -> Decoded<Module> {
        if self.bytes.len() < MAGIC.len() || &self.bytes[..MAGIC.len()] != MAGIC {
            return self.fault("not-intermediate-binary");
        }
        self.at = MAGIC.len();
        let version = self.unsigned()?;
        if version != u64::from(FORMAT_VERSION) {
            return Err((MAGIC.len(), "unknown-version"));
        }
        let mut module = Module::new(&self.text()?);
        module.layout = self.code(Layout::from_code)?;
        let files = self.count()?;
        for _ in 0..files {
            module.files.push(self.text()?);
        }
        let strings = self.count()?;
        for _ in 0..strings {
            module.strings.push(self.text()?);
        }
        let counts = Counts {
            files,
            strings,
            structs: self.count()?,
            classes: self.count()?,
            globals: self.count()?,
            functions: self.count()?,
        };
        for _ in 0..counts.structs {
            let name = self.text()?;
            let packed = self.flags(1)? != 0;
            let fields = self.types()?;
            module.structs.push(StructType {
                name,
                fields,
                packed,
            });
        }
        for _ in 0..counts.classes {
            let name = self.text()?;
            let flags = self.flags(15)?;
            let parent = if flags & 4 != 0 {
                Some(ClassId(self.index(counts.classes)?))
            } else {
                None
            };
            let element = if flags & 8 != 0 {
                Some(self.ty()?)
            } else {
                None
            };
            let fields = self.types()?;
            let methods = self.slots(counts.functions)?;
            let implemented = self.count()?;
            let mut implementations = Vec::with_capacity(implemented);
            for _ in 0..implemented {
                let interface = ClassId(self.index(counts.classes)?);
                implementations.push((interface, self.slots(counts.functions)?));
            }
            module.classes.push(ClassType {
                name,
                parent,
                interface: flags & 1 != 0,
                abstract_class: flags & 2 != 0,
                element,
                fields,
                methods,
                implementations,
            });
        }
        for _ in 0..counts.globals {
            let name = self.text()?;
            let size = self.small()?;
            let align = self.small()?;
            let flags = self.flags(7)?;
            let linkage = self.code(Linkage::from_code)?;
            let content = if flags & 4 != 0 {
                let length = self.count()?;
                let bytes = self.bytes[self.at..self.at + length].to_vec();
                self.at += length;
                GlobalContent::Bytes(bytes)
            } else {
                GlobalContent::Zero
            };
            let relocated = self.count()?;
            let mut relocations = Vec::with_capacity(relocated);
            for _ in 0..relocated {
                let offset = self.small()?;
                let start = self.at;
                let target = match self.byte()? {
                    0 => Symbol::Global(GlobalId(self.index(counts.globals)?)),
                    1 => Symbol::Function(FunctionId(self.index(counts.functions)?)),
                    _ => {
                        self.at = start;
                        return self.fault("unknown-code");
                    }
                };
                let start = self.at;
                let addend = i64::try_from(self.signed()?).or_else(|_| {
                    self.at = start;
                    self.fault("number-too-large")
                })?;
                relocations.push(Relocation {
                    offset,
                    target,
                    addend,
                });
            }
            module.globals.push(Global {
                name,
                size,
                align,
                mutable: flags & 1 != 0,
                thread_local: flags & 2 != 0,
                linkage,
                content,
                relocations,
            });
        }
        for _ in 0..counts.functions {
            let name = self.text()?;
            let signature = self.signature()?;
            let linkage = self.code(Linkage::from_code)?;
            let start = self.at;
            let mask = self.unsigned()?;
            if mask >> Attribute::ALL.len() != 0 {
                self.at = start;
                return self.fault("unknown-flags");
            }
            let mut function = Function::new(&name, signature, linkage);
            for attribute in Attribute::ALL {
                if mask & (1 << attribute.code()) != 0 {
                    function.attributes.insert(*attribute);
                }
            }
            let blocks = self.count()?;
            if blocks > 0 {
                self.body(&counts, &mut function, blocks)?;
            }
            module.functions.push(function);
        }
        if self.at != self.bytes.len() {
            return self.fault("trailing-bytes");
        }
        Ok(module)
    }
}

named_enum!(Rule {
    DuplicateName = "duplicate-name",
    EmptyName = "empty-name",
    SymbolRange = "symbol-range",
    StringRange = "string-range",
    FileRange = "file-range",
    ClassHierarchy = "class-hierarchy",
    InterfaceShape = "interface-shape",
    VirtualTable = "virtual-table",
    MethodSignature = "method-signature",
    Implementation = "implementation",
    GlobalShape = "global-shape",
    Alignment = "alignment",
    RelocationRange = "relocation-range",
    Declaration = "declaration",
    EntryFunction = "entry-function",
    EntryBlock = "entry-block",
    BlockTarget = "block-target",
    EdgeArguments = "edge-arguments",
    UnwindBlock = "unwind-block",
    InstructionPlacement = "instruction-placement",
    ValueRange = "value-range",
    ValueDefinedTwice = "value-defined-twice",
    ValueUndefined = "value-undefined",
    Dominance = "dominance",
    OperandCount = "operand-count",
    OperandType = "operand-type",
    ResultType = "result-type",
    ReturnType = "return-type",
    CallSignature = "call-signature",
    InvokeCall = "invoke-call",
    InvokeResult = "invoke-result",
    SwitchCases = "switch-cases",
    ImmediateRange = "immediate-range",
    AtomicOrdering = "atomic-ordering",
    ConversionTypes = "conversion",
    FieldRange = "field-range",
    MethodSlot = "method-slot",
    AbstractAllocation = "abstract-allocation",
    ArrayClass = "array-class",
    LaneRange = "lane-range",
    NoReturn = "noreturn",
    NoUnwind = "nounwind",
    UnterminatedBlock = "unterminated-block",
});

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Violation {
    pub rule: Rule,
    pub subject: String,
    pub block: Option<u32>,
    pub instruction: Option<u32>,
}

impl Violation {
    pub fn to_diagnostic(&self) -> Diagnostic {
        let mut diagnostic = fail("EI003", "The intermediate form broke one of its rules.")
            .with_context(format!("Rule: {}", self.rule.name()));
        if !self.subject.is_empty() {
            diagnostic = diagnostic.with_context(format!("Name: {}", self.subject));
        }
        if let Some(block) = self.block {
            diagnostic = diagnostic.with_context(format!("Block: block{block}"));
        }
        if let Some(instruction) = self.instruction {
            diagnostic = diagnostic.with_context(format!("Instruction: {instruction}"));
        }
        diagnostic
    }
}

fn integer_fits(value: i128, scalar: Scalar) -> bool {
    match scalar {
        Scalar::Bool => value == 0 || value == 1,
        Scalar::I128 => true,
        other if other.is_integer() => {
            let bits = other.bits();
            let low = -(1i128 << (bits - 1));
            let high = (1i128 << bits) - 1;
            (low..=high).contains(&value)
        }
        _ => false,
    }
}

fn is_word(ty: Type) -> bool {
    matches!(ty, Type::Scalar(scalar) if scalar.is_integer() && scalar != Scalar::Bool)
}

fn conversion_allowed(conversion: Conversion, from: Type, to: Type) -> bool {
    if conversion != Conversion::Bitcast && from.lanes() != to.lanes() {
        return false;
    }
    if matches!(from.element(), Scalar::Reference) || matches!(to.element(), Scalar::Reference) {
        return false;
    }
    let source = from.element();
    let target = to.element();
    match conversion {
        Conversion::ZeroExtend | Conversion::SignExtend => {
            source.is_integer() && target.is_integer() && source.bits() < target.bits()
        }
        Conversion::Truncate => {
            source.is_integer() && target.is_integer() && source.bits() > target.bits()
        }
        Conversion::FloatExtend => source == Scalar::F32 && target == Scalar::F64,
        Conversion::FloatTruncate => source == Scalar::F64 && target == Scalar::F32,
        Conversion::SignedToFloat | Conversion::UnsignedToFloat => {
            source.is_integer() && target.is_float()
        }
        Conversion::FloatToSigned
        | Conversion::FloatToUnsigned
        | Conversion::FloatToSignedSaturating
        | Conversion::FloatToUnsignedSaturating => {
            source.is_float() && target.is_integer() && target != Scalar::Bool
        }
        Conversion::Bitcast => {
            let plain = |scalar: Scalar| {
                !matches!(scalar, Scalar::Bool | Scalar::Pointer | Scalar::Reference)
            };
            plain(source)
                && plain(target)
                && source.bits() * from.lanes() == target.bits() * to.lanes()
        }
        Conversion::PointerToInteger => from == Type::POINTER && is_word(to),
        Conversion::IntegerToPointer => is_word(from) && to == Type::POINTER,
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Definition {
    Parameter(u32),
    Instruction(u32, usize),
    InvokeResult(u32),
}

struct Graph {
    predecessors: Vec<Vec<u32>>,
    order: Vec<usize>,
    dominator: Vec<Option<u32>>,
}

impl Graph {
    fn of(function: &Function) -> Graph {
        let count = function.blocks.len();
        let mut successors = vec![Vec::new(); count];
        let mut predecessors = vec![Vec::new(); count];
        for (index, data) in function.blocks.iter().enumerate() {
            for edge in data.terminator.successors() {
                if edge.block.index() < count {
                    successors[index].push(edge.block.0);
                    predecessors[edge.block.index()].push(index as u32);
                }
            }
        }
        let mut order = vec![usize::MAX; count];
        let mut postorder = Vec::with_capacity(count);
        if count > 0 {
            let mut visited = vec![false; count];
            let mut stack: Vec<(u32, usize)> = vec![(0, 0)];
            visited[0] = true;
            while let Some((block, next)) = stack.last().copied() {
                let edges = &successors[block as usize];
                if next < edges.len() {
                    if let Some(top) = stack.last_mut() {
                        top.1 += 1;
                    }
                    let target = edges[next];
                    if !visited[target as usize] {
                        visited[target as usize] = true;
                        stack.push((target, 0));
                    }
                } else {
                    postorder.push(block);
                    stack.pop();
                }
            }
        }
        let reverse: Vec<u32> = postorder.iter().rev().copied().collect();
        for (position, block) in reverse.iter().enumerate() {
            order[*block as usize] = position;
        }
        let mut dominator: Vec<Option<u32>> = vec![None; count];
        if count > 0 {
            dominator[0] = Some(0);
            let mut changed = true;
            while changed {
                changed = false;
                for block in reverse.iter().skip(1) {
                    let mut chosen: Option<u32> = None;
                    for predecessor in &predecessors[*block as usize] {
                        if dominator[*predecessor as usize].is_none() {
                            continue;
                        }
                        chosen = Some(match chosen {
                            None => *predecessor,
                            Some(current) => {
                                let mut left = current;
                                let mut right = *predecessor;
                                while left != right {
                                    while order[left as usize] > order[right as usize] {
                                        left = dominator[left as usize].unwrap_or(0);
                                    }
                                    while order[right as usize] > order[left as usize] {
                                        right = dominator[right as usize].unwrap_or(0);
                                    }
                                }
                                left
                            }
                        });
                    }
                    if chosen.is_some() && dominator[*block as usize] != chosen {
                        dominator[*block as usize] = chosen;
                        changed = true;
                    }
                }
            }
        }
        Graph {
            predecessors,
            order,
            dominator,
        }
    }

    fn reachable(&self, block: u32) -> bool {
        self.order
            .get(block as usize)
            .is_some_and(|position| *position != usize::MAX)
    }

    fn dominates(&self, upper: u32, lower: u32) -> bool {
        let mut current = lower;
        loop {
            if current == upper {
                return true;
            }
            match self.dominator.get(current as usize).copied().flatten() {
                Some(next) if next != current => current = next,
                _ => return false,
            }
        }
    }
}

struct Verifier<'m> {
    module: &'m Module,
    found: Vec<Violation>,
}

fn global_symbol(name: &str) -> String {
    symbol('@', name)
}

fn local_symbol(name: &str) -> String {
    symbol('%', name)
}

impl<'m> Verifier<'m> {
    fn report(
        &mut self,
        rule: Rule,
        subject: String,
        block: Option<u32>,
        instruction: Option<u32>,
    ) {
        self.found.push(Violation {
            rule,
            subject,
            block,
            instruction,
        });
    }

    fn module_rules(&mut self) {
        let module = self.module;
        let mut seen: BTreeSet<(u8, &str)> = BTreeSet::new();
        let named = module
            .functions
            .iter()
            .map(|function| (0u8, function.name.as_str()))
            .chain(
                module
                    .globals
                    .iter()
                    .map(|global| (0u8, global.name.as_str())),
            )
            .chain(
                module
                    .classes
                    .iter()
                    .map(|class| (1u8, class.name.as_str())),
            )
            .chain(
                module
                    .structs
                    .iter()
                    .map(|structure| (1u8, structure.name.as_str())),
            );
        for (space, name) in named {
            let shown = if space == 0 {
                global_symbol(name)
            } else {
                local_symbol(name)
            };
            if name.is_empty() {
                self.report(Rule::EmptyName, shown, None, None);
            } else if !seen.insert((space, name)) {
                self.report(Rule::DuplicateName, shown, None, None);
            }
        }
        let functions = module.functions.len();
        let classes = module.classes.len();
        for (index, class) in module.classes.iter().enumerate() {
            let shown = local_symbol(&class.name);
            if class.parent.is_some_and(|parent| parent.index() >= classes)
                || class
                    .implementations
                    .iter()
                    .any(|(interface, _)| interface.index() >= classes)
                || class
                    .methods
                    .iter()
                    .chain(
                        class
                            .implementations
                            .iter()
                            .flat_map(|(_, slots)| slots.iter()),
                    )
                    .any(|slot| slot.is_some_and(|id| id.index() >= functions))
            {
                self.report(Rule::SymbolRange, shown, None, None);
                continue;
            }
            self.class_rules(ClassId(index as u32), class);
        }
        for global in &module.globals {
            self.global_rules(global);
        }
        let entries: Vec<&Function> = module
            .functions
            .iter()
            .filter(|function| function.attributes.contains(&Attribute::Entry))
            .collect();
        for (index, entry) in entries.iter().enumerate() {
            if index > 0 || entry.is_declaration() {
                self.report(Rule::EntryFunction, global_symbol(&entry.name), None, None);
            }
        }
    }

    fn class_rules(&mut self, id: ClassId, class: &ClassType) {
        let module = self.module;
        let shown = local_symbol(&class.name);
        let mut steps = 0usize;
        let mut current = class.parent;
        while let Some(at) = current {
            steps += 1;
            if steps > module.classes.len() || at == id {
                self.report(Rule::ClassHierarchy, shown.clone(), None, None);
                return;
            }
            current = module
                .classes
                .get(at.index())
                .and_then(|parent| parent.parent);
        }
        if let Some(parent) = class.parent {
            let parent = &module.classes[parent.index()];
            if parent.interface {
                self.report(Rule::ClassHierarchy, shown.clone(), None, None);
                return;
            }
            if class.methods.len() < parent.methods.len() {
                self.report(Rule::VirtualTable, shown.clone(), None, None);
            }
            for (mine, inherited) in class.methods.iter().zip(&parent.methods) {
                if let (Some(mine), Some(inherited)) = (mine, inherited) {
                    if module.functions[mine.index()].signature
                        != module.functions[inherited.index()].signature
                    {
                        self.report(Rule::MethodSignature, shown.clone(), None, None);
                    }
                }
            }
        }
        if class.interface
            && (class.parent.is_some()
                || !class.fields.is_empty()
                || !class.implementations.is_empty())
        {
            self.report(Rule::InterfaceShape, shown.clone(), None, None);
        }
        if class.element.is_some()
            && (class.interface || class.abstract_class || !class.fields.is_empty())
        {
            self.report(Rule::ArrayClass, shown.clone(), None, None);
        }
        let concrete = !class.interface && !class.abstract_class;
        if concrete && self.leaves_a_method_abstract(class) {
            self.report(Rule::VirtualTable, shown.clone(), None, None);
        }
        for slot in class
            .methods
            .iter()
            .chain(
                class
                    .implementations
                    .iter()
                    .flat_map(|(_, slots)| slots.iter()),
            )
            .flatten()
        {
            let method = &module.functions[slot.index()];
            if method.signature.parameters.first() != Some(&Type::REFERENCE) {
                self.report(Rule::MethodSignature, shown.clone(), None, None);
            }
        }
        let mut implemented = BTreeSet::new();
        for (interface, slots) in &class.implementations {
            let target = &module.classes[interface.index()];
            if !target.interface
                || !implemented.insert(*interface)
                || slots.len() != target.methods.len()
            {
                self.report(Rule::Implementation, shown.clone(), None, None);
                continue;
            }
            for (mine, declared) in slots.iter().zip(&target.methods) {
                if let (Some(mine), Some(declared)) = (mine, declared) {
                    if module.functions[mine.index()].signature
                        != module.functions[declared.index()].signature
                    {
                        self.report(Rule::MethodSignature, shown.clone(), None, None);
                    }
                }
            }
        }
    }

    fn leaves_a_method_abstract(&self, class: &ClassType) -> bool {
        let module = self.module;
        if class.methods.iter().any(Option::is_none) {
            return true;
        }
        let mut resolved = BTreeSet::new();
        let mut current = Some(class);
        let mut steps = 0usize;
        while let Some(holder) = current {
            for (interface, slots) in &holder.implementations {
                if !resolved.insert(*interface) {
                    continue;
                }
                let Some(target) = module.classes.get(interface.index()) else {
                    continue;
                };
                let open = slots
                    .iter()
                    .zip(&target.methods)
                    .any(|(mine, declared)| mine.is_none() && declared.is_none());
                if open {
                    return true;
                }
            }
            steps += 1;
            if steps > module.classes.len() {
                break;
            }
            current = holder
                .parent
                .and_then(|parent| module.classes.get(parent.index()));
        }
        false
    }

    fn global_rules(&mut self, global: &Global) {
        let module = self.module;
        let shown = global_symbol(&global.name);
        if global.align == 0 || !global.align.is_power_of_two() {
            self.report(Rule::Alignment, shown.clone(), None, None);
        }
        if let GlobalContent::Bytes(bytes) = &global.content {
            if bytes.len() != global.size as usize {
                self.report(Rule::GlobalShape, shown.clone(), None, None);
            }
        }
        if global.linkage == Linkage::Import
            && (matches!(global.content, GlobalContent::Bytes(_)) || !global.relocations.is_empty())
        {
            self.report(Rule::Declaration, shown.clone(), None, None);
        }
        let mut covered: Vec<(u32, u32)> = Vec::new();
        for relocation in &global.relocations {
            let in_range = match relocation.target {
                Symbol::Global(id) => id.index() < module.globals.len(),
                Symbol::Function(id) => id.index() < module.functions.len(),
            };
            if !in_range {
                self.report(Rule::SymbolRange, shown.clone(), None, None);
                continue;
            }
            let width = module.layout.data().pointer_bytes;
            let end = relocation.offset.checked_add(width);
            let overlaps = covered.iter().any(|(start, stop)| {
                relocation.offset < *stop && end.is_some_and(|end| *start < end)
            });
            if end.is_none_or(|end| end > global.size) || overlaps {
                self.report(Rule::RelocationRange, shown.clone(), None, None);
                continue;
            }
            covered.push((relocation.offset, relocation.offset + width));
        }
    }

    fn function_rules(&mut self, function: &'m Function) {
        let shown = global_symbol(&function.name);
        if function.linkage == Linkage::Import {
            if !function.is_declaration() {
                self.report(Rule::Declaration, shown, None, None);
            }
            return;
        }
        if function.is_declaration() {
            self.report(Rule::Declaration, shown, None, None);
            return;
        }
        FunctionVerifier::new(self, function, shown).run();
    }
}

struct FunctionVerifier<'v, 'm> {
    verifier: &'v mut Verifier<'m>,
    module: &'m Module,
    function: &'m Function,
    shown: String,
    definitions: Vec<Option<Definition>>,
    graph: Graph,
    unwind_targets: BTreeSet<u32>,
    plain_targets: BTreeSet<u32>,
}

impl<'v, 'm> FunctionVerifier<'v, 'm> {
    fn new(
        verifier: &'v mut Verifier<'m>,
        function: &'m Function,
        shown: String,
    ) -> FunctionVerifier<'v, 'm> {
        let module = verifier.module;
        FunctionVerifier {
            verifier,
            module,
            function,
            shown,
            definitions: vec![None; function.value_types.len()],
            graph: Graph::of(function),
            unwind_targets: BTreeSet::new(),
            plain_targets: BTreeSet::new(),
        }
    }

    fn report(&mut self, rule: Rule, block: Option<u32>, instruction: Option<u32>) {
        let subject = self.shown.clone();
        self.verifier.report(rule, subject, block, instruction);
    }

    fn type_of(&self, value: Value) -> Option<Type> {
        self.function.value_types.get(value.index()).copied()
    }

    fn define(&mut self, value: Value, definition: Definition, block: u32, position: Option<u32>) {
        match self.definitions.get_mut(value.index()) {
            None => self.report(Rule::ValueRange, Some(block), position),
            Some(Some(_)) => self.report(Rule::ValueDefinedTwice, Some(block), position),
            Some(slot) => *slot = Some(definition),
        }
    }

    fn run(&mut self) {
        let function = self.function;
        let mut placed = vec![false; function.instructions.len()];
        for (index, data) in function.blocks.iter().enumerate() {
            let block = index as u32;
            for parameter in &data.parameters {
                self.define(*parameter, Definition::Parameter(block), block, None);
            }
            for (position, inst) in data.instructions.iter().enumerate() {
                let Some(slot) = placed.get_mut(inst.index()) else {
                    self.report(
                        Rule::InstructionPlacement,
                        Some(block),
                        Some(position as u32),
                    );
                    continue;
                };
                if *slot {
                    self.report(
                        Rule::InstructionPlacement,
                        Some(block),
                        Some(position as u32),
                    );
                    continue;
                }
                *slot = true;
                if let Some(result) = function.instructions[inst.index()].result {
                    self.define(
                        result,
                        Definition::Instruction(block, position),
                        block,
                        Some(position as u32),
                    );
                }
            }
            if let Terminator::Invoke { call, .. } = &data.terminator {
                if let Some(result) = call.result {
                    let at = data.instructions.len() as u32;
                    self.define(result, Definition::InvokeResult(block), block, Some(at));
                }
            }
        }
        self.entry_rules();
        for (index, data) in function.blocks.iter().enumerate() {
            let block = index as u32;
            for (position, inst) in data.instructions.iter().enumerate() {
                if let Some(instruction) = function.instructions.get(inst.index()) {
                    self.instruction(instruction, block, position as u32);
                }
            }
            self.terminator(&data.terminator, block, data.instructions.len() as u32);
        }
        for target in self.unwind_targets.clone() {
            if self.plain_targets.contains(&target) {
                self.report(Rule::UnwindBlock, Some(target), None);
            }
        }
    }

    fn entry_rules(&mut self) {
        let function = self.function;
        let entry = &function.blocks[0];
        let types: Vec<Option<Type>> = entry
            .parameters
            .iter()
            .map(|value| self.type_of(*value))
            .collect();
        let wanted: Vec<Option<Type>> = function
            .signature
            .parameters
            .iter()
            .map(|ty| Some(*ty))
            .collect();
        if types != wanted {
            self.report(Rule::EntryBlock, Some(0), None);
        }
        if !self.graph.predecessors[0].is_empty() {
            self.report(Rule::EntryBlock, Some(0), None);
        }
    }

    fn uses(&mut self, values: &[Value], block: u32, position: u32, edge_of_invoke: bool) -> bool {
        let mut usable = true;
        for value in values {
            let Some(slot) = self.definitions.get(value.index()) else {
                self.report(Rule::ValueRange, Some(block), Some(position));
                usable = false;
                continue;
            };
            let Some(definition) = *slot else {
                self.report(Rule::ValueUndefined, Some(block), Some(position));
                usable = false;
                continue;
            };
            let reachable = self.graph.reachable(block);
            match definition {
                Definition::InvokeResult(defined) => {
                    if !(edge_of_invoke && defined == block) {
                        self.report(Rule::InvokeResult, Some(block), Some(position));
                    }
                }
                Definition::Parameter(defined) => {
                    if reachable && !self.graph.dominates(defined, block) {
                        self.report(Rule::Dominance, Some(block), Some(position));
                    }
                }
                Definition::Instruction(defined, at) => {
                    let fine = if defined == block {
                        (at as u32) < position
                    } else {
                        self.graph.dominates(defined, block)
                    };
                    if reachable && !fine {
                        self.report(Rule::Dominance, Some(block), Some(position));
                    }
                }
            }
        }
        usable
    }

    fn expect(&mut self, ok: bool, rule: Rule, block: u32, position: u32) {
        if !ok {
            self.report(rule, Some(block), Some(position));
        }
    }

    fn instruction(&mut self, instruction: &Instruction, block: u32, position: u32) {
        if let Some(spot) = &instruction.spot {
            if spot.file as usize >= self.module.files.len() {
                self.report(Rule::FileRange, Some(block), Some(position));
            }
        }
        if !self.uses(&instruction.arguments, block, position, false) {
            return;
        }
        if let Some(result) = instruction.result {
            if result.index() >= self.function.value_types.len() {
                return;
            }
        }
        self.operation(instruction, block, position);
    }

    fn operation(&mut self, instruction: &Instruction, block: u32, position: u32) {
        let module = self.module;
        let arguments: Vec<Type> = instruction
            .arguments
            .iter()
            .map(|value| self.type_of(*value).unwrap_or(Type::BOOL))
            .collect();
        let result = instruction.result.and_then(|value| self.type_of(value));
        let count = |wanted: usize| arguments.len() == wanted;
        let produces = |ty: Option<Type>| result == ty;
        let (wanted_count, rule_ok): (usize, bool);
        match &instruction.operation {
            Operation::IntegerConstant(value) => {
                wanted_count = 0;
                rule_ok = matches!(result, Some(Type::Scalar(scalar)) if scalar.is_integer());
                if let Some(Type::Scalar(scalar)) = result {
                    if rule_ok && !integer_fits(*value, scalar) {
                        self.report(Rule::ImmediateRange, Some(block), Some(position));
                    }
                }
            }
            Operation::FloatConstant(bits) => {
                wanted_count = 0;
                rule_ok = matches!(result, Some(ty) if ty.is_scalar_float());
                if result == Some(Type::F32) && *bits > u64::from(u32::MAX) {
                    self.report(Rule::ImmediateRange, Some(block), Some(position));
                }
            }
            Operation::Null => {
                wanted_count = 0;
                rule_ok = produces(Some(Type::POINTER)) || produces(Some(Type::REFERENCE));
            }
            Operation::Poison => {
                wanted_count = 0;
                rule_ok = result.is_some();
            }
            Operation::GlobalAddress(id) => {
                wanted_count = 0;
                rule_ok = produces(Some(Type::POINTER));
                if id.index() >= module.globals.len() {
                    self.report(Rule::SymbolRange, Some(block), Some(position));
                }
            }
            Operation::FunctionAddress(id) => {
                wanted_count = 0;
                rule_ok = produces(Some(Type::POINTER));
                if id.index() >= module.functions.len() {
                    self.report(Rule::SymbolRange, Some(block), Some(position));
                }
            }
            Operation::StringConstant(index) => {
                wanted_count = 0;
                rule_ok = produces(Some(Type::REFERENCE));
                if *index as usize >= module.strings.len() {
                    self.report(Rule::StringRange, Some(block), Some(position));
                }
            }
            Operation::Binary(op) => {
                wanted_count = 2;
                if count(2) {
                    let ty = arguments[0];
                    let fits = if op.is_float() {
                        ty.is_float()
                    } else {
                        ty.is_integer()
                    };
                    let scalar_needed = op.yields_bool();
                    self.expect(
                        arguments[1] == ty && fits && (!scalar_needed || ty.is_scalar_integer()),
                        Rule::OperandType,
                        block,
                        position,
                    );
                    let wanted = if op.yields_bool() { Type::BOOL } else { ty };
                    rule_ok = produces(Some(wanted));
                } else {
                    rule_ok = true;
                }
            }
            Operation::Unary(op) => {
                wanted_count = 1;
                if count(1) {
                    let ty = arguments[0];
                    let fits = if op.is_float() {
                        ty.is_float()
                    } else {
                        ty.is_integer()
                    };
                    self.expect(fits, Rule::OperandType, block, position);
                    rule_ok = produces(Some(ty));
                } else {
                    rule_ok = true;
                }
            }
            Operation::Compare(comparison) => {
                wanted_count = 2;
                if count(2) {
                    let ty = arguments[0];
                    let fits = match ty {
                        Type::Scalar(Scalar::Reference) => {
                            matches!(comparison, Comparison::Equal | Comparison::NotEqual)
                        }
                        Type::Scalar(scalar) => scalar.is_integer() || scalar == Scalar::Pointer,
                        Type::Vector { .. } => false,
                    };
                    self.expect(
                        fits && arguments[1] == ty,
                        Rule::OperandType,
                        block,
                        position,
                    );
                }
                rule_ok = produces(Some(Type::BOOL));
            }
            Operation::FloatCompare(_) => {
                wanted_count = 2;
                if count(2) {
                    self.expect(
                        arguments[0].is_scalar_float() && arguments[1] == arguments[0],
                        Rule::OperandType,
                        block,
                        position,
                    );
                }
                rule_ok = produces(Some(Type::BOOL));
            }
            Operation::Convert(conversion) => {
                wanted_count = 1;
                rule_ok = result.is_some();
                if let (true, Some(to)) = (count(1), result) {
                    if !conversion_allowed(*conversion, arguments[0], to) {
                        self.report(Rule::ConversionTypes, Some(block), Some(position));
                    }
                }
            }
            Operation::Select => {
                wanted_count = 3;
                if count(3) {
                    self.expect(
                        arguments[0] == Type::BOOL && arguments[1] == arguments[2],
                        Rule::OperandType,
                        block,
                        position,
                    );
                    rule_ok = produces(Some(arguments[1]));
                } else {
                    rule_ok = true;
                }
            }
            Operation::Splat => {
                wanted_count = 1;
                rule_ok = matches!(result, Some(Type::Vector { .. }));
                if let (true, Some(ty)) = (count(1), result) {
                    self.expect(
                        arguments[0] == Type::Scalar(ty.element()),
                        Rule::OperandType,
                        block,
                        position,
                    );
                }
            }
            Operation::ExtractLane(lane) | Operation::InsertLane(lane) => {
                let inserting = matches!(instruction.operation, Operation::InsertLane(_));
                wanted_count = if inserting { 2 } else { 1 };
                if count(wanted_count) {
                    let vector = arguments[0];
                    let is_vector = matches!(vector, Type::Vector { .. });
                    self.expect(is_vector, Rule::OperandType, block, position);
                    if is_vector && u32::from(*lane) >= vector.lanes() {
                        self.report(Rule::LaneRange, Some(block), Some(position));
                    }
                    if inserting {
                        self.expect(
                            arguments[1] == Type::Scalar(vector.element()),
                            Rule::OperandType,
                            block,
                            position,
                        );
                        rule_ok = produces(Some(vector));
                    } else {
                        rule_ok = produces(Some(Type::Scalar(vector.element())));
                    }
                } else {
                    rule_ok = true;
                }
            }
            Operation::StackSlot { align, .. } => {
                wanted_count = 0;
                rule_ok = produces(Some(Type::POINTER));
                if *align == 0 || !align.is_power_of_two() {
                    self.report(Rule::Alignment, Some(block), Some(position));
                }
            }
            Operation::DynamicStackSlot { align } => {
                wanted_count = 1;
                rule_ok = produces(Some(Type::POINTER));
                if *align == 0 || !align.is_power_of_two() {
                    self.report(Rule::Alignment, Some(block), Some(position));
                }
                if count(1) {
                    self.expect(is_word(arguments[0]), Rule::OperandType, block, position);
                }
            }
            Operation::StackSave => {
                wanted_count = 0;
                rule_ok = produces(Some(Type::POINTER));
            }
            Operation::VaStart => {
                wanted_count = 1;
                rule_ok = result.is_none();
                if count(1) {
                    self.expect(
                        arguments[0] == Type::POINTER,
                        Rule::OperandType,
                        block,
                        position,
                    );
                }
                self.expect(
                    self.function.signature.variadic,
                    Rule::CallSignature,
                    block,
                    position,
                );
            }
            Operation::VaArg => {
                wanted_count = 1;
                rule_ok = result.is_some_and(|ty| {
                    matches!(ty, Type::Scalar(scalar) if scalar != Scalar::I128 && scalar != Scalar::Reference)
                });
                if count(1) {
                    self.expect(
                        arguments[0] == Type::POINTER,
                        Rule::OperandType,
                        block,
                        position,
                    );
                }
            }
            Operation::VaCopy => {
                wanted_count = 2;
                rule_ok = result.is_none();
                if count(2) {
                    self.expect(
                        arguments[0] == Type::POINTER && arguments[1] == Type::POINTER,
                        Rule::OperandType,
                        block,
                        position,
                    );
                }
            }
            Operation::StackRestore => {
                wanted_count = 1;
                rule_ok = result.is_none();
                if count(1) {
                    self.expect(
                        arguments[0] == Type::POINTER,
                        Rule::OperandType,
                        block,
                        position,
                    );
                }
            }
            Operation::Load {
                align, ordering, ..
            } => {
                wanted_count = 1;
                rule_ok = result.is_some();
                if count(1) {
                    self.expect(
                        arguments[0] == Type::POINTER,
                        Rule::OperandType,
                        block,
                        position,
                    );
                }
                if *align == 0 || !align.is_power_of_two() {
                    self.report(Rule::Alignment, Some(block), Some(position));
                }
                if let Some(ordering) = ordering {
                    let fine = !matches!(ordering, Ordering::Release | Ordering::AcquireRelease)
                        && matches!(result, Some(Type::Scalar(scalar)) if scalar != Scalar::Bool);
                    self.expect(fine, Rule::AtomicOrdering, block, position);
                }
            }
            Operation::Store {
                align, ordering, ..
            } => {
                wanted_count = 2;
                rule_ok = result.is_none();
                if count(2) {
                    self.expect(
                        arguments[1] == Type::POINTER,
                        Rule::OperandType,
                        block,
                        position,
                    );
                    if let Some(ordering) = ordering {
                        let fine = !matches!(
                            ordering,
                            Ordering::Acquire | Ordering::AcquireRelease
                        ) && matches!(arguments[0], Type::Scalar(scalar) if scalar != Scalar::Bool);
                        self.expect(fine, Rule::AtomicOrdering, block, position);
                    }
                }
                if *align == 0 || !align.is_power_of_two() {
                    self.report(Rule::Alignment, Some(block), Some(position));
                }
            }
            Operation::Offset | Operation::ElementAddress(_) => {
                wanted_count = 2;
                if count(2) {
                    self.expect(
                        arguments[0] == Type::POINTER && is_word(arguments[1]),
                        Rule::OperandType,
                        block,
                        position,
                    );
                }
                rule_ok = produces(Some(Type::POINTER));
            }
            Operation::FieldAddress(structure, index) => {
                wanted_count = 1;
                if count(1) {
                    self.expect(
                        arguments[0] == Type::POINTER,
                        Rule::OperandType,
                        block,
                        position,
                    );
                }
                rule_ok = produces(Some(Type::POINTER));
                match module.structs.get(structure.index()) {
                    None => self.report(Rule::SymbolRange, Some(block), Some(position)),
                    Some(data) if *index as usize >= data.fields.len() => {
                        self.report(Rule::FieldRange, Some(block), Some(position))
                    }
                    Some(_) => {}
                }
            }
            Operation::AtomicUpdate(op, _) => {
                wanted_count = 2;
                if count(2) {
                    let ty = arguments[1];
                    let fine = is_word(ty) || (ty == Type::POINTER && *op == AtomicOp::Exchange);
                    self.expect(
                        arguments[0] == Type::POINTER && fine,
                        Rule::OperandType,
                        block,
                        position,
                    );
                    rule_ok = produces(Some(ty));
                } else {
                    rule_ok = true;
                }
            }
            Operation::CompareExchange(success, failure) => {
                wanted_count = 3;
                if count(3) {
                    let ty = arguments[1];
                    let fine = is_word(ty) || ty == Type::POINTER;
                    self.expect(
                        arguments[0] == Type::POINTER && fine && arguments[2] == ty,
                        Rule::OperandType,
                        block,
                        position,
                    );
                    rule_ok = produces(Some(ty));
                } else {
                    rule_ok = true;
                }
                let fine = !matches!(failure, Ordering::Release | Ordering::AcquireRelease)
                    && failure.strength() <= success.strength();
                self.expect(fine, Rule::AtomicOrdering, block, position);
            }
            Operation::Fence(ordering) => {
                wanted_count = 0;
                rule_ok = result.is_none();
                self.expect(
                    *ordering != Ordering::Relaxed,
                    Rule::AtomicOrdering,
                    block,
                    position,
                );
            }
            Operation::MemoryCopy | Operation::MemoryMove | Operation::MemoryFill => {
                wanted_count = 3;
                rule_ok = result.is_none();
                if count(3) {
                    let middle = if matches!(instruction.operation, Operation::MemoryFill) {
                        Type::I8
                    } else {
                        Type::POINTER
                    };
                    self.expect(
                        arguments[0] == Type::POINTER
                            && arguments[1] == middle
                            && is_word(arguments[2]),
                        Rule::OperandType,
                        block,
                        position,
                    );
                }
            }
            Operation::Call(callee) => {
                let Some(target) = module.functions.get(callee.index()) else {
                    self.report(Rule::SymbolRange, Some(block), Some(position));
                    return;
                };
                self.call(&target.signature, &arguments, result, block, position);
                return;
            }
            Operation::CallIndirect(signature) => {
                if arguments.first() != Some(&Type::POINTER) {
                    self.report(Rule::OperandType, Some(block), Some(position));
                    return;
                }
                self.call(signature, &arguments[1..], result, block, position);
                return;
            }
            Operation::Allocate(class) => {
                wanted_count = 0;
                rule_ok = produces(Some(Type::REFERENCE));
                match module.classes.get(class.index()) {
                    None => self.report(Rule::SymbolRange, Some(block), Some(position)),
                    Some(data) if data.interface || data.abstract_class => {
                        self.report(Rule::AbstractAllocation, Some(block), Some(position))
                    }
                    Some(data) if data.element.is_some() => {
                        self.report(Rule::ArrayClass, Some(block), Some(position))
                    }
                    Some(_) => {}
                }
            }
            Operation::AllocateArray(class) => {
                wanted_count = 1;
                if count(1) {
                    self.expect(is_word(arguments[0]), Rule::OperandType, block, position);
                }
                rule_ok = produces(Some(Type::REFERENCE));
                match module.classes.get(class.index()) {
                    None => self.report(Rule::SymbolRange, Some(block), Some(position)),
                    Some(data) if data.element.is_none() => {
                        self.report(Rule::ArrayClass, Some(block), Some(position))
                    }
                    Some(_) => {}
                }
            }
            Operation::LoadField(class, index) | Operation::StoreField(class, index) => {
                let storing = matches!(instruction.operation, Operation::StoreField(..));
                wanted_count = if storing { 2 } else { 1 };
                let field = match module.classes.get(class.index()) {
                    None => {
                        self.report(Rule::SymbolRange, Some(block), Some(position));
                        return;
                    }
                    Some(data) if data.interface => {
                        self.report(Rule::FieldRange, Some(block), Some(position));
                        return;
                    }
                    Some(_) => module.field_types(*class).get(*index as usize).copied(),
                };
                let Some(field) = field else {
                    self.report(Rule::FieldRange, Some(block), Some(position));
                    return;
                };
                if count(wanted_count) {
                    self.expect(
                        arguments[0] == Type::REFERENCE,
                        Rule::OperandType,
                        block,
                        position,
                    );
                    if storing {
                        self.expect(arguments[1] == field, Rule::OperandType, block, position);
                    }
                }
                rule_ok = if storing {
                    result.is_none()
                } else {
                    produces(Some(field))
                };
            }
            Operation::LoadElement(ty) | Operation::StoreElement(ty) => {
                let storing = matches!(instruction.operation, Operation::StoreElement(_));
                wanted_count = if storing { 3 } else { 2 };
                if count(wanted_count) {
                    self.expect(
                        arguments[0] == Type::REFERENCE && is_word(arguments[1]),
                        Rule::OperandType,
                        block,
                        position,
                    );
                    if storing {
                        self.expect(arguments[2] == *ty, Rule::OperandType, block, position);
                    }
                }
                rule_ok = if storing {
                    result.is_none()
                } else {
                    produces(Some(*ty))
                };
            }
            Operation::ArrayLength => {
                wanted_count = 1;
                if count(1) {
                    self.expect(
                        arguments[0] == Type::REFERENCE,
                        Rule::OperandType,
                        block,
                        position,
                    );
                }
                rule_ok = result.is_some_and(is_word);
            }
            Operation::InstanceOf(class) => {
                wanted_count = 1;
                if count(1) {
                    self.expect(
                        arguments[0] == Type::REFERENCE,
                        Rule::OperandType,
                        block,
                        position,
                    );
                }
                rule_ok = produces(Some(Type::BOOL));
                if class.index() >= module.classes.len() {
                    self.report(Rule::SymbolRange, Some(block), Some(position));
                }
            }
            Operation::VirtualMethod(class, slot) | Operation::InterfaceMethod(class, slot) => {
                let through_interface =
                    matches!(instruction.operation, Operation::InterfaceMethod(..));
                wanted_count = 1;
                if count(1) {
                    self.expect(
                        arguments[0] == Type::REFERENCE,
                        Rule::OperandType,
                        block,
                        position,
                    );
                }
                rule_ok = produces(Some(Type::POINTER));
                match module.classes.get(class.index()) {
                    None => self.report(Rule::SymbolRange, Some(block), Some(position)),
                    Some(data) => {
                        if data.interface != through_interface
                            || *slot as usize >= data.methods.len()
                        {
                            self.report(Rule::MethodSlot, Some(block), Some(position));
                        }
                    }
                }
            }
            Operation::SameObject => {
                wanted_count = 2;
                if count(2) {
                    self.expect(
                        arguments[0] == Type::REFERENCE && arguments[1] == Type::REFERENCE,
                        Rule::OperandType,
                        block,
                        position,
                    );
                }
                rule_ok = produces(Some(Type::BOOL));
            }
            Operation::Trap(_) => {
                wanted_count = 0;
                rule_ok = result.is_none();
            }
        }
        if !count(wanted_count) {
            self.report(Rule::OperandCount, Some(block), Some(position));
        }
        if !rule_ok {
            self.report(Rule::ResultType, Some(block), Some(position));
        }
    }

    fn call(
        &mut self,
        signature: &Signature,
        arguments: &[Type],
        result: Option<Type>,
        block: u32,
        position: u32,
    ) {
        let fixed = signature.parameters.len();
        let count_fine = if signature.variadic {
            arguments.len() >= fixed
        } else {
            arguments.len() == fixed
        };
        let types_fine = arguments
            .iter()
            .zip(&signature.parameters)
            .all(|(given, wanted)| given == wanted);
        if !count_fine || !types_fine || result != signature.result {
            self.report(Rule::CallSignature, Some(block), Some(position));
        }
    }

    fn edge(&mut self, call: &BlockCall, block: u32, position: u32, unwind: bool, of_invoke: bool) {
        let Some(target) = self.function.blocks.get(call.block.index()) else {
            self.report(Rule::BlockTarget, Some(block), Some(position));
            return;
        };
        if unwind {
            self.unwind_targets.insert(call.block.0);
        } else {
            self.plain_targets.insert(call.block.0);
        }
        if !self.uses(&call.arguments, block, position, of_invoke && !unwind) {
            return;
        }
        let parameters: &[Value] = if unwind {
            match target.parameters.split_first() {
                Some((first, rest)) if self.type_of(*first) == Some(Type::REFERENCE) => rest,
                _ => {
                    self.report(Rule::UnwindBlock, Some(call.block.0), None);
                    return;
                }
            }
        } else {
            &target.parameters
        };
        let wanted: Vec<Option<Type>> = parameters
            .iter()
            .map(|value| self.type_of(*value))
            .collect();
        let given: Vec<Option<Type>> = call
            .arguments
            .iter()
            .map(|value| self.type_of(*value))
            .collect();
        if wanted != given {
            self.report(Rule::EdgeArguments, Some(block), Some(position));
        }
    }

    fn terminator(&mut self, terminator: &Terminator, block: u32, position: u32) {
        let function = self.function;
        match terminator {
            Terminator::Jump(call) => self.edge(call, block, position, false, false),
            Terminator::Branch {
                condition,
                taken,
                otherwise,
            } => {
                if self.uses(&[*condition], block, position, false)
                    && self.type_of(*condition) != Some(Type::BOOL)
                {
                    self.report(Rule::OperandType, Some(block), Some(position));
                }
                self.edge(taken, block, position, false, false);
                self.edge(otherwise, block, position, false, false);
            }
            Terminator::Switch {
                value,
                cases,
                default,
            } => {
                if self.uses(&[*value], block, position, false) {
                    match self.type_of(*value) {
                        Some(Type::Scalar(scalar)) if scalar.is_integer() => {
                            if cases.iter().any(|(case, _)| !integer_fits(*case, scalar)) {
                                self.report(Rule::ImmediateRange, Some(block), Some(position));
                            }
                            let mut seen = BTreeSet::new();
                            let mask = if scalar == Scalar::I128 {
                                u128::MAX
                            } else {
                                (1u128 << scalar.bits()) - 1
                            };
                            if cases
                                .iter()
                                .any(|(case, _)| !seen.insert((*case as u128) & mask))
                            {
                                self.report(Rule::SwitchCases, Some(block), Some(position));
                            }
                        }
                        _ => self.report(Rule::OperandType, Some(block), Some(position)),
                    }
                }
                for (_, call) in cases {
                    self.edge(call, block, position, false, false);
                }
                self.edge(default, block, position, false, false);
            }
            Terminator::Return(value) => {
                if function.attributes.contains(&Attribute::NoReturn) {
                    self.report(Rule::NoReturn, Some(block), Some(position));
                }
                match value {
                    Some(value) => {
                        if self.uses(&[*value], block, position, false)
                            && self.type_of(*value) != function.signature.result
                        {
                            self.report(Rule::ReturnType, Some(block), Some(position));
                        }
                    }
                    None => {
                        if function.signature.result.is_some() {
                            self.report(Rule::ReturnType, Some(block), Some(position));
                        }
                    }
                }
            }
            Terminator::Unreachable => {}
            Terminator::Throw(value) => {
                if function.attributes.contains(&Attribute::NoUnwind) {
                    self.report(Rule::NoUnwind, Some(block), Some(position));
                }
                if self.uses(&[*value], block, position, false)
                    && self.type_of(*value) != Some(Type::REFERENCE)
                {
                    self.report(Rule::OperandType, Some(block), Some(position));
                }
            }
            Terminator::Invoke {
                call,
                normal,
                unwind,
            } => {
                if !matches!(
                    call.operation,
                    Operation::Call(_) | Operation::CallIndirect(_)
                ) {
                    self.report(Rule::InvokeCall, Some(block), Some(position));
                } else {
                    self.instruction(call, block, position);
                }
                self.edge(normal, block, position, false, true);
                self.edge(unwind, block, position, true, true);
            }
        }
    }
}

impl Module {
    pub fn violations(&self) -> Vec<Violation> {
        let mut verifier = Verifier {
            module: self,
            found: Vec::new(),
        };
        verifier.module_rules();
        for function in &self.functions {
            verifier.function_rules(function);
        }
        verifier.found
    }

    pub fn verify(&self) -> Result<(), Diagnostic> {
        match self.violations().first() {
            Some(violation) => Err(violation.to_diagnostic()),
            None => Ok(()),
        }
    }
}

named_enum!(Trap {
    Explicit = "explicit",
    Unreachable = "unreachable",
    Abort = "abort",
    DivisionByZero = "division-by-zero",
    NullAccess = "null-access",
    OutOfBounds = "out-of-bounds",
    UseAfterFree = "use-after-free",
    Misaligned = "misaligned",
    ReadOnly = "read-only",
    OverlappingCopy = "overlapping-copy",
    InvalidFree = "invalid-free",
    PoisonObserved = "poison-observed",
    ConversionRange = "conversion-range",
    BadFunctionPointer = "bad-function-pointer",
    SignatureMismatch = "signature-mismatch",
    NullReference = "null-reference",
    TypeConfusion = "type-confusion",
    IndexOutOfBounds = "index-out-of-bounds",
    NegativeLength = "negative-length",
    AbstractMethod = "abstract-method",
    MissingImplementation = "missing-implementation",
    ForgedReference = "forged-reference",
    MemoryExhausted = "memory-exhausted",
});

#[derive(Clone, PartialEq, Debug)]
pub enum Datum {
    Integer(u128),
    Float(u64),
    Pointer(u64),
    Reference(u32),
    Vector(Vec<Datum>),
    Poison,
}

impl Datum {
    pub fn integer(ty: Type, value: i128) -> Datum {
        Datum::Integer((value as u128) & mask(ty.element().bits()))
    }

    pub fn f32(value: f32) -> Datum {
        Datum::Float(u64::from(value.to_bits()))
    }

    pub fn f64(value: f64) -> Datum {
        Datum::Float(value.to_bits())
    }

    pub fn as_integer(&self) -> Option<u128> {
        match self {
            Datum::Integer(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_signed(&self, ty: Type) -> Option<i128> {
        self.as_integer()
            .map(|value| signed(value, ty.element().bits()))
    }

    pub fn as_pointer(&self) -> Option<u64> {
        match self {
            Datum::Pointer(address) => Some(*address),
            _ => None,
        }
    }

    pub fn as_reference(&self) -> Option<u32> {
        match self {
            Datum::Reference(handle) => Some(*handle),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Datum::Float(bits) => Some(f64::from_bits(*bits)),
            _ => None,
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        match self {
            Datum::Float(bits) => Some(f32::from_bits(*bits as u32)),
            _ => None,
        }
    }

    pub fn zero(ty: Type) -> Datum {
        match ty {
            Type::Scalar(scalar) => match scalar {
                Scalar::F32 | Scalar::F64 => Datum::Float(0),
                Scalar::Pointer => Datum::Pointer(0),
                Scalar::Reference => Datum::Reference(0),
                _ => Datum::Integer(0),
            },
            Type::Vector { lanes, element } => {
                Datum::Vector(vec![Datum::zero(Type::Scalar(element)); usize::from(lanes)])
            }
        }
    }

    fn fits(&self, ty: Type, pointer_bytes: u32) -> bool {
        match (self, ty) {
            (Datum::Poison, _) => true,
            (Datum::Integer(value), Type::Scalar(scalar)) if scalar.is_integer() => {
                *value <= mask(scalar.bits())
            }
            (Datum::Float(bits), Type::Scalar(Scalar::F32)) => *bits <= u64::from(u32::MAX),
            (Datum::Float(_), Type::Scalar(Scalar::F64)) => true,
            (Datum::Pointer(address), Type::Scalar(Scalar::Pointer)) => {
                pointer_bytes >= 8 || *address <= u64::from(u32::MAX)
            }
            (Datum::Reference(_), Type::Scalar(Scalar::Reference)) => true,
            (Datum::Vector(items), Type::Vector { lanes, element }) => {
                items.len() == usize::from(lanes)
                    && items.iter().all(|item| {
                        !matches!(item, Datum::Poison)
                            && item.fits(Type::Scalar(element), pointer_bytes)
                    })
            }
            _ => false,
        }
    }
}

fn mask(bits: u32) -> u128 {
    if bits >= 128 {
        u128::MAX
    } else {
        (1u128 << bits) - 1
    }
}

fn signed(value: u128, bits: u32) -> i128 {
    if bits >= 128 {
        value as i128
    } else {
        let shift = 128 - bits;
        ((value << shift) as i128) >> shift
    }
}

fn wide_multiply(left: u128, right: u128) -> (u128, u128) {
    let low_mask = u128::from(u64::MAX);
    let (left_high, left_low) = (left >> 64, left & low_mask);
    let (right_high, right_low) = (right >> 64, right & low_mask);
    let low_low = left_low * right_low;
    let low_high = left_low * right_high;
    let high_low = left_high * right_low;
    let high_high = left_high * right_high;
    let middle = (low_low >> 64) + (low_high & low_mask) + (high_low & low_mask);
    let low = (low_low & low_mask) | (middle << 64);
    let high = high_high + (low_high >> 64) + (high_low >> 64) + (middle >> 64);
    (high, low)
}

fn integer_binary(op: BinaryOp, left: u128, right: u128, bits: u32) -> Result<u128, Trap> {
    let all = mask(bits);
    let (left_signed, right_signed) = (signed(left, bits), signed(right, bits));
    let lowest = if bits >= 128 {
        i128::MIN
    } else {
        -(1i128 << (bits - 1))
    };
    let highest = if bits >= 128 {
        i128::MAX
    } else {
        (1i128 << (bits - 1)) - 1
    };
    let amount = (right % u128::from(bits)) as u32;
    let value = match op {
        BinaryOp::Add => left.wrapping_add(right),
        BinaryOp::Subtract => left.wrapping_sub(right),
        BinaryOp::Multiply => left.wrapping_mul(right),
        BinaryOp::DivideSigned => {
            if right == 0 {
                return Err(Trap::DivisionByZero);
            }
            if left_signed == lowest && right_signed == -1 {
                left
            } else {
                (left_signed / right_signed) as u128
            }
        }
        BinaryOp::DivideUnsigned => {
            if right == 0 {
                return Err(Trap::DivisionByZero);
            }
            left / right
        }
        BinaryOp::RemainderSigned => {
            if right == 0 {
                return Err(Trap::DivisionByZero);
            }
            if right_signed == -1 {
                0
            } else {
                (left_signed % right_signed) as u128
            }
        }
        BinaryOp::RemainderUnsigned => {
            if right == 0 {
                return Err(Trap::DivisionByZero);
            }
            left % right
        }
        BinaryOp::And => left & right,
        BinaryOp::Or => left | right,
        BinaryOp::Xor => left ^ right,
        BinaryOp::ShiftLeft => left << amount,
        BinaryOp::ShiftRightLogical => left >> amount,
        BinaryOp::ShiftRightArithmetic => (left_signed >> amount) as u128,
        BinaryOp::RotateLeft => {
            if amount == 0 {
                left
            } else {
                (left << amount) | (left >> (bits - amount))
            }
        }
        BinaryOp::RotateRight => {
            if amount == 0 {
                left
            } else {
                (left >> amount) | (left << (bits - amount))
            }
        }
        BinaryOp::MinimumSigned => {
            if left_signed <= right_signed {
                left
            } else {
                right
            }
        }
        BinaryOp::MaximumSigned => {
            if left_signed >= right_signed {
                left
            } else {
                right
            }
        }
        BinaryOp::MinimumUnsigned => left.min(right),
        BinaryOp::MaximumUnsigned => left.max(right),
        BinaryOp::AddSaturatingSigned => {
            if bits >= 128 {
                left_signed.saturating_add(right_signed) as u128
            } else {
                (left_signed + right_signed).clamp(lowest, highest) as u128
            }
        }
        BinaryOp::AddSaturatingUnsigned => left.saturating_add(right).min(all),
        BinaryOp::SubtractSaturatingSigned => {
            if bits >= 128 {
                left_signed.saturating_sub(right_signed) as u128
            } else {
                (left_signed - right_signed).clamp(lowest, highest) as u128
            }
        }
        BinaryOp::SubtractSaturatingUnsigned => left.saturating_sub(right),
        BinaryOp::AddOverflowsSigned => u128::from(
            left_signed
                .checked_add(right_signed)
                .is_none_or(|sum| !(lowest..=highest).contains(&sum)),
        ),
        BinaryOp::AddOverflowsUnsigned => {
            u128::from(left.checked_add(right).is_none_or(|sum| sum > all))
        }
        BinaryOp::SubtractOverflowsSigned => u128::from(
            left_signed
                .checked_sub(right_signed)
                .is_none_or(|difference| !(lowest..=highest).contains(&difference)),
        ),
        BinaryOp::SubtractOverflowsUnsigned => u128::from(right > left),
        BinaryOp::MultiplyOverflowsSigned => u128::from(
            left_signed
                .checked_mul(right_signed)
                .is_none_or(|product| !(lowest..=highest).contains(&product)),
        ),
        BinaryOp::MultiplyOverflowsUnsigned => {
            u128::from(left.checked_mul(right).is_none_or(|product| product > all))
        }
        BinaryOp::MultiplyHighSigned => {
            if bits <= 64 {
                ((left_signed * right_signed) >> bits) as u128
            } else {
                let (high, _) = wide_multiply(left, right);
                let mut adjusted = high;
                if left_signed < 0 {
                    adjusted = adjusted.wrapping_sub(right);
                }
                if right_signed < 0 {
                    adjusted = adjusted.wrapping_sub(left);
                }
                adjusted
            }
        }
        BinaryOp::MultiplyHighUnsigned => {
            if bits <= 64 {
                (left * right) >> bits
            } else {
                wide_multiply(left, right).0
            }
        }
        _ => left,
    };
    Ok(value & all)
}

fn integer_unary(op: UnaryOp, value: u128, bits: u32) -> u128 {
    let all = mask(bits);
    let result = match op {
        UnaryOp::Negate => value.wrapping_neg(),
        UnaryOp::Not => !value,
        UnaryOp::PopulationCount => u128::from(value.count_ones()),
        UnaryOp::LeadingZeros => u128::from(value.leading_zeros() - (128 - bits)),
        UnaryOp::TrailingZeros => {
            if value == 0 {
                u128::from(bits)
            } else {
                u128::from(value.trailing_zeros())
            }
        }
        UnaryOp::ByteSwap => {
            if bits < 16 {
                value
            } else {
                value.swap_bytes() >> (128 - bits)
            }
        }
        UnaryOp::BitReverse => value.reverse_bits() >> (128 - bits),
        UnaryOp::AbsoluteSigned => signed(value, bits).unsigned_abs(),
        _ => value,
    };
    result & all
}

const SINGLE_NAN: u32 = 0x7fc0_0000;
const DOUBLE_NAN: u64 = 0x7ff8_0000_0000_0000;

macro_rules! float_family {
    ($float:ty, $bits:ty, $nan:expr, $binary:ident, $unary:ident, $compare:ident) => {
        fn $binary(op: BinaryOp, left: $float, right: $float) -> $bits {
            let canonical = |value: $float| {
                if value.is_nan() {
                    $nan
                } else {
                    value.to_bits()
                }
            };
            match op {
                BinaryOp::FloatAdd => canonical(left + right),
                BinaryOp::FloatSubtract => canonical(left - right),
                BinaryOp::FloatMultiply => canonical(left * right),
                BinaryOp::FloatDivide => canonical(left / right),
                BinaryOp::FloatRemainder => canonical(left % right),
                BinaryOp::FloatMinimum | BinaryOp::FloatMaximum => {
                    if left.is_nan() || right.is_nan() {
                        return $nan;
                    }
                    let lower = op == BinaryOp::FloatMinimum;
                    if left == right {
                        let either_negative = left.is_sign_negative() || right.is_sign_negative();
                        let both_negative = left.is_sign_negative() && right.is_sign_negative();
                        let negative = if lower {
                            either_negative
                        } else {
                            both_negative
                        };
                        let magnitude = left.abs();
                        return if negative {
                            (-magnitude).to_bits()
                        } else {
                            magnitude.to_bits()
                        };
                    }
                    let chosen = if (left < right) == lower { left } else { right };
                    chosen.to_bits()
                }
                BinaryOp::FloatMinimumNumber | BinaryOp::FloatMaximumNumber => {
                    if left.is_nan() && right.is_nan() {
                        return $nan;
                    }
                    if left.is_nan() {
                        return right.to_bits();
                    }
                    if right.is_nan() {
                        return left.to_bits();
                    }
                    let lower = op == BinaryOp::FloatMinimumNumber;
                    if left == right {
                        let either_negative = left.is_sign_negative() || right.is_sign_negative();
                        let both_negative = left.is_sign_negative() && right.is_sign_negative();
                        let negative = if lower {
                            either_negative
                        } else {
                            both_negative
                        };
                        let magnitude = left.abs();
                        return if negative {
                            (-magnitude).to_bits()
                        } else {
                            magnitude.to_bits()
                        };
                    }
                    let chosen = if (left < right) == lower { left } else { right };
                    chosen.to_bits()
                }
                BinaryOp::CopySign => left.copysign(right).to_bits(),
                _ => left.to_bits(),
            }
        }

        fn $unary(op: UnaryOp, value: $float) -> $bits {
            let canonical = |result: $float| {
                if result.is_nan() {
                    $nan
                } else {
                    result.to_bits()
                }
            };
            match op {
                UnaryOp::FloatNegate => (-value).to_bits(),
                UnaryOp::FloatAbsolute => value.abs().to_bits(),
                UnaryOp::SquareRoot => canonical(value.sqrt()),
                UnaryOp::Floor => canonical(value.floor()),
                UnaryOp::Ceiling => canonical(value.ceil()),
                UnaryOp::TruncateFloat => canonical(value.trunc()),
                UnaryOp::RoundNearestEven => canonical(value.round_ties_even()),
                _ => value.to_bits(),
            }
        }

        fn $compare(comparison: FloatComparison, left: $float, right: $float) -> bool {
            let unordered = left.is_nan() || right.is_nan();
            match comparison {
                FloatComparison::OrderedEqual => !unordered && left == right,
                FloatComparison::OrderedNotEqual => !unordered && left != right,
                FloatComparison::OrderedLess => !unordered && left < right,
                FloatComparison::OrderedLessEqual => !unordered && left <= right,
                FloatComparison::OrderedGreater => !unordered && left > right,
                FloatComparison::OrderedGreaterEqual => !unordered && left >= right,
                FloatComparison::Ordered => !unordered,
                FloatComparison::Unordered => unordered,
                FloatComparison::UnorderedEqual => unordered || left == right,
                FloatComparison::UnorderedNotEqual => unordered || left != right,
                FloatComparison::UnorderedLess => unordered || left < right,
                FloatComparison::UnorderedLessEqual => unordered || left <= right,
                FloatComparison::UnorderedGreater => unordered || left > right,
                FloatComparison::UnorderedGreaterEqual => unordered || left >= right,
            }
        }
    };
}

float_family!(
    f32,
    u32,
    SINGLE_NAN,
    single_binary,
    single_unary,
    single_compare
);
float_family!(
    f64,
    u64,
    DOUBLE_NAN,
    double_binary,
    double_unary,
    double_compare
);

fn float_value(bits: u64, scalar: Scalar) -> f64 {
    if scalar == Scalar::F32 {
        f64::from(f32::from_bits(bits as u32))
    } else {
        f64::from_bits(bits)
    }
}

fn float_bits(value: f64, scalar: Scalar) -> u64 {
    if scalar == Scalar::F32 {
        let narrowed = value as f32;
        if narrowed.is_nan() {
            u64::from(SINGLE_NAN)
        } else {
            u64::from(narrowed.to_bits())
        }
    } else if value.is_nan() {
        DOUBLE_NAN
    } else {
        value.to_bits()
    }
}

fn integer_from_float(
    value: f64,
    bits: u32,
    signed_target: bool,
    saturating: bool,
) -> Result<u128, Trap> {
    let power = |exponent: u32| -> f64 { 2f64.powi(exponent as i32) };
    let (lowest, limit) = if signed_target {
        (-power(bits - 1), power(bits - 1))
    } else {
        (0.0, power(bits))
    };
    if value.is_nan() {
        return if saturating {
            Ok(0)
        } else {
            Err(Trap::ConversionRange)
        };
    }
    let whole = value.trunc();
    if whole < lowest || whole >= limit {
        if !saturating {
            return Err(Trap::ConversionRange);
        }
        return Ok(if whole < lowest {
            if signed_target {
                (1u128 << (bits - 1)) & mask(bits)
            } else {
                0
            }
        } else if signed_target {
            mask(bits - 1)
        } else {
            mask(bits)
        });
    }
    let result = if signed_target {
        (whole as i128) as u128
    } else {
        whole as u128
    };
    Ok(result & mask(bits))
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Region {
    Global,
    Constant,
    Stack,
    Heap,
}

#[derive(Debug)]
struct Allocation {
    base: u64,
    size: u64,
    bytes: Vec<u8>,
    live: bool,
    region: Region,
}

#[derive(Clone, Debug)]
pub enum Object {
    Instance {
        class: ClassId,
        fields: Vec<Datum>,
    },
    Array {
        class: ClassId,
        element: Type,
        items: Vec<Datum>,
    },
    Text(String),
    Released,
}

const LOWEST_ADDRESS: u64 = 0x1_0000;
const GUARD: u64 = 16;
const FUNCTION_STRIDE: u64 = 16;

pub struct Machine {
    pointer_bytes: u32,
    layout: DataLayout,
    allocations: Vec<Allocation>,
    starts: BTreeMap<u64, usize>,
    next: u64,
    top: u64,
    live: u64,
    memory_limit: u64,
    references: BTreeMap<u64, u32>,
    objects: Vec<Object>,
    charges: Vec<u64>,
    released: Vec<u32>,
    object_limit: usize,
    texts: BTreeMap<u32, u32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Machine {
    fn new(layout: DataLayout, limits: &Limits) -> Machine {
        let top = if layout.pointer_bytes >= 8 {
            0x7f00_0000_0000
        } else {
            0xf000_0000
        };
        Machine {
            pointer_bytes: layout.pointer_bytes,
            layout,
            allocations: Vec::new(),
            starts: BTreeMap::new(),
            next: LOWEST_ADDRESS,
            top,
            live: 0,
            memory_limit: limits.memory,
            references: BTreeMap::new(),
            objects: Vec::new(),
            charges: Vec::new(),
            released: Vec::new(),
            object_limit: limits.objects,
            texts: BTreeMap::new(),
            stdout: Vec::new(),
            stderr: Vec::new(),
        }
    }

    fn function_base(&self) -> u64 {
        self.top
    }

    pub fn function_address(&self, function: FunctionId) -> u64 {
        self.top + u64::from(function.0) * FUNCTION_STRIDE
    }

    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }

    pub fn emit(&mut self, descriptor: u32, bytes: &[u8]) -> bool {
        match descriptor {
            1 => self.stdout.extend_from_slice(bytes),
            2 => self.stderr.extend_from_slice(bytes),
            _ => return false,
        }
        true
    }

    fn allocate_in(&mut self, size: u64, align: u64, region: Region) -> Result<u64, Trap> {
        let align = align.max(16);
        let base = self
            .next
            .checked_next_multiple_of(align)
            .ok_or(Trap::MemoryExhausted)?;
        let end = base
            .checked_add(size)
            .and_then(|end| end.checked_add(GUARD))
            .ok_or(Trap::MemoryExhausted)?;
        if end > self.top || self.live.saturating_add(size) > self.memory_limit {
            return Err(Trap::MemoryExhausted);
        }
        self.next = end;
        self.live += size;
        self.starts.insert(base, self.allocations.len());
        self.allocations.push(Allocation {
            base,
            size,
            bytes: vec![0; size as usize],
            live: true,
            region,
        });
        Ok(base)
    }

    pub fn allocate(&mut self, size: u64, align: u64) -> Result<u64, Trap> {
        self.allocate_in(size, align, Region::Heap)
    }

    fn retire(&mut self, index: usize) {
        let allocation = &mut self.allocations[index];
        allocation.live = false;
        allocation.bytes = Vec::new();
        self.live -= allocation.size;
        let (base, size) = (allocation.base, allocation.size);
        let doomed: Vec<u64> = self
            .references
            .range(base..base + size)
            .map(|(at, _)| *at)
            .collect();
        for at in doomed {
            self.references.remove(&at);
        }
    }

    pub fn release(&mut self, address: u64) -> Result<(), Trap> {
        let Some(index) = self.starts.get(&address).copied() else {
            return Err(Trap::InvalidFree);
        };
        let allocation = &self.allocations[index];
        if allocation.region != Region::Heap || !allocation.live {
            return Err(Trap::InvalidFree);
        }
        self.retire(index);
        Ok(())
    }

    pub fn allocation_size(&self, address: u64) -> Option<u64> {
        let index = *self.starts.get(&address)?;
        let allocation = &self.allocations[index];
        (allocation.live && allocation.region == Region::Heap).then_some(allocation.size)
    }

    fn find(&self, address: u64, length: u64) -> Result<(usize, usize), Trap> {
        if address < LOWEST_ADDRESS {
            return Err(Trap::NullAccess);
        }
        let Some((_, index)) = self.starts.range(..=address).next_back() else {
            return Err(Trap::OutOfBounds);
        };
        let allocation = &self.allocations[*index];
        let offset = address - allocation.base;
        if offset
            .checked_add(length)
            .is_none_or(|end| end > allocation.size)
        {
            return Err(Trap::OutOfBounds);
        }
        if !allocation.live {
            return Err(Trap::UseAfterFree);
        }
        Ok((*index, offset as usize))
    }

    pub fn read(&self, address: u64, length: u64) -> Result<&[u8], Trap> {
        if length == 0 {
            return Ok(&[]);
        }
        let (index, offset) = self.find(address, length)?;
        Ok(&self.allocations[index].bytes[offset..offset + length as usize])
    }

    fn write_bytes(&mut self, address: u64, bytes: &[u8], initializing: bool) -> Result<(), Trap> {
        if bytes.is_empty() {
            return Ok(());
        }
        let (index, offset) = self.find(address, bytes.len() as u64)?;
        if !initializing && self.allocations[index].region == Region::Constant {
            return Err(Trap::ReadOnly);
        }
        self.allocations[index].bytes[offset..offset + bytes.len()].copy_from_slice(bytes);
        let width = u64::from(self.pointer_bytes);
        let start = address.saturating_sub(width - 1);
        let end = address + bytes.len() as u64;
        let doomed: Vec<u64> = self
            .references
            .range(start..end)
            .map(|(at, _)| *at)
            .collect();
        for at in doomed {
            self.references.remove(&at);
        }
        Ok(())
    }

    pub fn write(&mut self, address: u64, bytes: &[u8]) -> Result<(), Trap> {
        self.write_bytes(address, bytes, false)
    }

    pub fn c_string(&self, address: u64) -> Result<Vec<u8>, Trap> {
        let (index, offset) = self.find(address, 1)?;
        let bytes = &self.allocations[index].bytes[offset..];
        match bytes.iter().position(|byte| *byte == 0) {
            Some(end) => Ok(bytes[..end].to_vec()),
            None => Err(Trap::OutOfBounds),
        }
    }

    fn scalar_bytes(&self, scalar: Scalar) -> u64 {
        u64::from(self.layout.size_of(Type::Scalar(scalar)))
    }

    fn encode(&self, datum: &Datum, ty: Type, out: &mut Vec<u8>) -> Result<(), Trap> {
        match (datum, ty) {
            (Datum::Poison, _) => Err(Trap::PoisonObserved),
            (Datum::Vector(items), Type::Vector { element, .. }) => {
                for item in items {
                    self.encode(item, Type::Scalar(element), out)?;
                }
                Ok(())
            }
            (_, Type::Scalar(scalar)) => {
                let width = self.scalar_bytes(scalar) as usize;
                let raw: u128 = match datum {
                    Datum::Integer(value) => *value,
                    Datum::Float(bits) => u128::from(*bits),
                    Datum::Pointer(address) => u128::from(*address),
                    Datum::Reference(handle) => u128::from(*handle),
                    _ => return Err(Trap::TypeConfusion),
                };
                out.extend_from_slice(&raw.to_le_bytes()[..width]);
                Ok(())
            }
            _ => Err(Trap::TypeConfusion),
        }
    }

    fn decode(&self, bytes: &[u8], ty: Type) -> Datum {
        match ty {
            Type::Vector { lanes, element } => {
                let width = self.scalar_bytes(element) as usize;
                Datum::Vector(
                    (0..usize::from(lanes))
                        .map(|lane| {
                            self.decode(
                                &bytes[lane * width..(lane + 1) * width],
                                Type::Scalar(element),
                            )
                        })
                        .collect(),
                )
            }
            Type::Scalar(scalar) => {
                let mut raw = [0u8; 16];
                raw[..bytes.len()].copy_from_slice(bytes);
                let value = u128::from_le_bytes(raw);
                match scalar {
                    Scalar::Bool => Datum::Integer(u128::from(value != 0)),
                    Scalar::F32 | Scalar::F64 => Datum::Float(value as u64),
                    Scalar::Pointer => Datum::Pointer(value as u64),
                    Scalar::Reference => Datum::Reference(value as u32),
                    _ => Datum::Integer(value),
                }
            }
        }
    }

    fn load(&self, ty: Type, address: u64, align: u64) -> Result<Datum, Trap> {
        if align > 1 && !address.is_multiple_of(align) {
            return Err(Trap::Misaligned);
        }
        let size = u64::from(self.layout.size_of(ty));
        let bytes = self.read(address, size)?;
        if ty == Type::REFERENCE {
            if let Some(handle) = self.references.get(&address) {
                return Ok(Datum::Reference(*handle));
            }
            if bytes.iter().all(|byte| *byte == 0) {
                return Ok(Datum::Reference(0));
            }
            return Err(Trap::ForgedReference);
        }
        Ok(self.decode(bytes, ty))
    }

    fn store(
        &mut self,
        datum: &Datum,
        ty: Type,
        address: u64,
        align: u64,
        initializing: bool,
    ) -> Result<(), Trap> {
        if align > 1 && !address.is_multiple_of(align) {
            return Err(Trap::Misaligned);
        }
        let mut bytes = Vec::with_capacity(16);
        self.encode(datum, ty, &mut bytes)?;
        self.write_bytes(address, &bytes, initializing)?;
        if let Datum::Reference(handle) = datum {
            if *handle != 0 {
                self.references.insert(address, *handle);
            }
        }
        Ok(())
    }

    fn copy(&mut self, target: u64, source: u64, length: u64) -> Result<(), Trap> {
        if length == 0 {
            return Ok(());
        }
        let bytes = self.read(source, length)?.to_vec();
        let width = u64::from(self.pointer_bytes);
        let carried: Vec<(u64, u32)> = self
            .references
            .range(source..source + length)
            .filter(|(at, _)| **at + width <= source + length)
            .map(|(at, handle)| (*at - source, *handle))
            .collect();
        self.write(target, &bytes)?;
        for (offset, handle) in carried {
            self.references.insert(target + offset, handle);
        }
        Ok(())
    }

    fn pressed(&self, bytes: u64) -> bool {
        self.objects.len() - self.released.len() >= self.object_limit
            || self.live.saturating_add(bytes) > self.memory_limit
    }

    fn new_object(&mut self, object: Object, bytes: u64) -> Result<u32, Trap> {
        if self.pressed(bytes) {
            return Err(Trap::MemoryExhausted);
        }
        self.live += bytes;
        if let Some(handle) = self.released.pop() {
            let at = handle as usize - 1;
            self.objects[at] = object;
            self.charges[at] = bytes;
            return Ok(handle);
        }
        self.objects.push(object);
        self.charges.push(bytes);
        Ok(self.objects.len() as u32)
    }

    fn gather(datum: &Datum, out: &mut Vec<u32>) {
        match datum {
            Datum::Reference(handle) if *handle != 0 => out.push(*handle),
            Datum::Vector(items) => {
                for item in items {
                    Self::gather(item, out);
                }
            }
            _ => {}
        }
    }

    fn collect(&mut self, mut waiting: Vec<u32>) {
        waiting.extend(self.references.values().copied());
        waiting.extend(self.texts.values().copied());
        let mut marked = vec![false; self.objects.len()];
        while let Some(handle) = waiting.pop() {
            let Some(at) = (handle as usize).checked_sub(1) else {
                continue;
            };
            if at >= marked.len() || marked[at] {
                continue;
            }
            marked[at] = true;
            match &self.objects[at] {
                Object::Instance { fields, .. } => {
                    for field in fields {
                        Self::gather(field, &mut waiting);
                    }
                }
                Object::Array { items, .. } => {
                    for item in items {
                        Self::gather(item, &mut waiting);
                    }
                }
                Object::Text(_) | Object::Released => {}
            }
        }
        for (at, kept) in marked.iter().enumerate() {
            if *kept || matches!(self.objects[at], Object::Released) {
                continue;
            }
            self.live = self.live.saturating_sub(self.charges[at]);
            self.objects[at] = Object::Released;
            self.released.push(at as u32 + 1);
        }
        self.released
            .sort_unstable_by(|left, right| right.cmp(left));
    }

    pub fn object(&self, handle: u32) -> Option<&Object> {
        handle
            .checked_sub(1)
            .and_then(|at| self.objects.get(at as usize))
    }

    fn object_mut(&mut self, handle: u32) -> Option<&mut Object> {
        handle
            .checked_sub(1)
            .and_then(|at| self.objects.get_mut(at as usize))
    }

    pub fn class_of(&self, handle: u32) -> Option<ClassId> {
        match self.object(handle)? {
            Object::Instance { class, .. } | Object::Array { class, .. } => Some(*class),
            Object::Text(_) | Object::Released => None,
        }
    }

    pub fn text(&self, handle: u32) -> Option<&str> {
        match self.object(handle)? {
            Object::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn make_text(&mut self, text: &str) -> Result<Datum, Trap> {
        let bytes = text.len() as u64 + 16;
        self.new_object(Object::Text(text.to_string()), bytes)
            .map(Datum::Reference)
    }

    pub fn items(&self, handle: u32) -> Option<&[Datum]> {
        match self.object(handle)? {
            Object::Array { items, .. } => Some(items),
            _ => None,
        }
    }

    pub fn items_mut(&mut self, handle: u32) -> Option<&mut [Datum]> {
        match self.object_mut(handle)? {
            Object::Array { items, .. } => Some(items),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Limits {
    pub fuel: u64,
    pub depth: usize,
    pub memory: u64,
    pub objects: usize,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            fuel: 100_000_000,
            depth: 10_000,
            memory: 256 << 20,
            objects: 4_000_000,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum HostAnswer {
    Returned(Option<Datum>),
    Missing,
    Trapped(Trap),
    Exited(i32),
}

pub trait Host {
    fn global(&mut self, name: &str, size: u32) -> Option<Vec<u8>>;

    fn call(&mut self, name: &str, arguments: &[Datum], machine: &mut Machine) -> HostAnswer;
}

pub struct StandardHost;

impl StandardHost {
    fn number(datum: Option<&Datum>) -> Option<u64> {
        match datum? {
            Datum::Integer(value) => Some(*value as u64),
            _ => None,
        }
    }

    fn address(datum: Option<&Datum>) -> Option<u64> {
        datum?.as_pointer()
    }

    fn handle(datum: Option<&Datum>) -> Result<u32, Trap> {
        match datum {
            Some(Datum::Reference(0)) => Err(Trap::NullReference),
            Some(Datum::Reference(handle)) => Ok(*handle),
            _ => Err(Trap::TypeConfusion),
        }
    }

    fn float(datum: Option<&Datum>) -> Option<f64> {
        match datum? {
            Datum::Float(bits) => Some(f64::from_bits(*bits)),
            _ => None,
        }
    }

    fn units(machine: &Machine, handle: u32, from: u64, count: u64) -> Result<Vec<u16>, Trap> {
        let items = machine.items(handle).ok_or(Trap::TypeConfusion)?;
        let end = from.checked_add(count).ok_or(Trap::IndexOutOfBounds)?;
        if end > items.len() as u64 {
            return Err(Trap::IndexOutOfBounds);
        }
        items[from as usize..end as usize]
            .iter()
            .map(|item| match item {
                Datum::Integer(value) => Ok(*value as u16),
                _ => Err(Trap::TypeConfusion),
            })
            .collect()
    }

    fn put_units(machine: &mut Machine, handle: u32, text: &str) -> Result<usize, Trap> {
        let units: Vec<u16> = text.encode_utf16().collect();
        let items = machine.items_mut(handle).ok_or(Trap::TypeConfusion)?;
        if units.len() > items.len() {
            return Err(Trap::IndexOutOfBounds);
        }
        for (slot, unit) in items.iter_mut().zip(&units) {
            *slot = Datum::Integer(u128::from(*unit));
        }
        Ok(units.len())
    }

    fn scientific(text: &str) -> (String, i32) {
        let (mantissa, exponent) = text.split_once('e').unwrap_or((text, "0"));
        let digits: String = mantissa.chars().filter(char::is_ascii_digit).collect();
        (digits, exponent.parse().unwrap_or(0))
    }

    fn shortest_digits(value: f64, single: bool) -> (String, i32) {
        let narrow = value as f32;
        let text = if single {
            format!("{narrow:e}")
        } else {
            format!("{value:e}")
        };
        let (digits, exponent) = Self::scientific(&text);
        if digits.len() != 1 {
            return (digits, exponent);
        }
        let wider = if single {
            format!("{narrow:.1e}")
        } else {
            format!("{value:.1e}")
        };
        let faithful = if single {
            wider.parse::<f32>() == Ok(narrow)
        } else {
            wider.parse::<f64>() == Ok(value)
        };
        if !faithful {
            return (digits, exponent);
        }
        let (wide, at) = Self::scientific(&wider);
        let trimmed = wide.trim_end_matches('0');
        if trimmed.is_empty() {
            return (digits, exponent);
        }
        (trimmed.to_string(), at)
    }

    pub fn decimal_text(value: f64, single: bool) -> String {
        if value.is_nan() {
            return "NaN".to_string();
        }
        if value.is_infinite() {
            return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
        }
        let mut made = String::new();
        if value.is_sign_negative() {
            made.push('-');
        }
        if value == 0.0 {
            made.push_str("0.0");
            return made;
        }
        let (digits, exponent) = Self::shortest_digits(value.abs(), single);
        let count = digits.len() as i32;
        if (-3..7).contains(&exponent) {
            if exponent >= 0 {
                let whole = (exponent + 1) as usize;
                if digits.len() <= whole {
                    made.push_str(&digits);
                    made.extend(std::iter::repeat_n('0', whole - digits.len()));
                    made.push_str(".0");
                } else {
                    made.push_str(&digits[..whole]);
                    made.push('.');
                    made.push_str(&digits[whole..]);
                }
            } else {
                made.push_str("0.");
                made.extend(std::iter::repeat_n('0', (-exponent - 1) as usize));
                made.push_str(&digits);
            }
            return made;
        }
        made.push_str(&digits[..1]);
        made.push('.');
        if count > 1 {
            made.push_str(&digits[1..]);
        } else {
            made.push('0');
        }
        made.push('E');
        made.push_str(&exponent.to_string());
        made
    }

    fn mathematics(name: &str, first: f64, second: f64) -> Option<f64> {
        Some(match name {
            "sin" => first.sin(),
            "cos" => first.cos(),
            "tan" => first.tan(),
            "asin" => first.asin(),
            "acos" => first.acos(),
            "atan" => first.atan(),
            "atan2" => first.atan2(second),
            "exp" => first.exp(),
            "log" => first.ln(),
            "log10" => first.log10(),
            "sinh" => first.sinh(),
            "cosh" => first.cosh(),
            "tanh" => first.tanh(),
            "cbrt" => first.cbrt(),
            "hypot" => first.hypot(second),
            "expm1" => first.exp_m1(),
            "log1p" => first.ln_1p(),
            "pow" => {
                if second == 0.0 {
                    1.0
                } else if second.is_nan() || (first.abs() == 1.0 && second.is_infinite()) {
                    f64::NAN
                } else {
                    first.powf(second)
                }
            }
            _ => return None,
        })
    }

    fn runtime(
        name: &str,
        arguments: &[Datum],
        machine: &mut Machine,
    ) -> Result<Option<Datum>, Trap> {
        let first = arguments.first();
        let second = arguments.get(1);
        match name {
            "ir.identity" => {
                let handle = Self::handle(first)?;
                Ok(Some(Datum::integer(Type::I64, i128::from(handle))))
            }
            "ir.write_chars" => {
                let descriptor = Self::number(first).ok_or(Trap::TypeConfusion)?;
                let handle = Self::handle(second)?;
                let from = Self::number(arguments.get(2)).ok_or(Trap::TypeConfusion)?;
                let count = Self::number(arguments.get(3)).ok_or(Trap::TypeConfusion)?;
                let units = Self::units(machine, handle, from, count)?;
                let text: String = char::decode_utf16(units)
                    .map(|unit| unit.unwrap_or('?'))
                    .collect();
                machine.emit(descriptor as u32, text.as_bytes());
                Ok(None)
            }
            "ir.format_double" | "ir.format_float" => {
                let single = name == "ir.format_float";
                let value = match first {
                    Some(Datum::Float(bits)) if single => f64::from(f32::from_bits(*bits as u32)),
                    Some(Datum::Float(bits)) => f64::from_bits(*bits),
                    _ => return Err(Trap::TypeConfusion),
                };
                let handle = Self::handle(second)?;
                let text = Self::decimal_text(value, single);
                let written = Self::put_units(machine, handle, &text)?;
                Ok(Some(Datum::integer(Type::I32, written as i128)))
            }
            "ir.parse_double" => {
                let handle = Self::handle(first)?;
                let count = Self::number(second).ok_or(Trap::TypeConfusion)?;
                let units = Self::units(machine, handle, 0, count)?;
                let text = String::from_utf16_lossy(&units);
                let value = text.trim().parse::<f64>().unwrap_or(f64::NAN);
                Ok(Some(Datum::f64(value)))
            }
            "ir.clock" => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|elapsed| elapsed.as_nanos() as i128)
                    .unwrap_or(0);
                Ok(Some(Datum::integer(Type::I64, now)))
            }
            _ => {
                let Some(operation) = name.strip_prefix("ir.math.") else {
                    return Err(Trap::MissingImplementation);
                };
                let left = Self::float(first).ok_or(Trap::TypeConfusion)?;
                let right = Self::float(second).unwrap_or(0.0);
                Self::mathematics(operation, left, right)
                    .map(|value| Some(Datum::f64(value)))
                    .ok_or(Trap::MissingImplementation)
            }
        }
    }
}

impl Host for StandardHost {
    fn global(&mut self, _name: &str, _size: u32) -> Option<Vec<u8>> {
        None
    }

    fn call(&mut self, name: &str, arguments: &[Datum], machine: &mut Machine) -> HostAnswer {
        let first = arguments.first();
        let second = arguments.get(1);
        let third = arguments.get(2);
        let pointer = |address: u64| HostAnswer::Returned(Some(Datum::Pointer(address)));
        let integer = |value: u128| HostAnswer::Returned(Some(Datum::Integer(value)));
        match name {
            "malloc" => {
                let Some(size) = Self::number(first) else {
                    return HostAnswer::Trapped(Trap::TypeConfusion);
                };
                pointer(machine.allocate(size.max(1), 16).unwrap_or(0))
            }
            "calloc" => {
                let (Some(count), Some(size)) = (Self::number(first), Self::number(second)) else {
                    return HostAnswer::Trapped(Trap::TypeConfusion);
                };
                match count.checked_mul(size) {
                    Some(total) => pointer(machine.allocate(total.max(1), 16).unwrap_or(0)),
                    None => pointer(0),
                }
            }
            "realloc" => {
                let (Some(old), Some(size)) = (Self::address(first), Self::number(second)) else {
                    return HostAnswer::Trapped(Trap::TypeConfusion);
                };
                if old == 0 {
                    return pointer(machine.allocate(size.max(1), 16).unwrap_or(0));
                }
                let Some(held) = machine.allocation_size(old) else {
                    return HostAnswer::Trapped(Trap::InvalidFree);
                };
                let Ok(fresh) = machine.allocate(size.max(1), 16) else {
                    return pointer(0);
                };
                if let Err(trap) = machine.copy(fresh, old, held.min(size)) {
                    return HostAnswer::Trapped(trap);
                }
                if let Err(trap) = machine.release(old) {
                    return HostAnswer::Trapped(trap);
                }
                pointer(fresh)
            }
            "free" => {
                let Some(address) = Self::address(first) else {
                    return HostAnswer::Trapped(Trap::TypeConfusion);
                };
                if address != 0 {
                    if let Err(trap) = machine.release(address) {
                        return HostAnswer::Trapped(trap);
                    }
                }
                HostAnswer::Returned(None)
            }
            "memcpy" | "memmove" => {
                let (Some(target), Some(source), Some(length)) = (
                    Self::address(first),
                    Self::address(second),
                    Self::number(third),
                ) else {
                    return HostAnswer::Trapped(Trap::TypeConfusion);
                };
                if name == "memcpy"
                    && length > 0
                    && target < source + length
                    && source < target + length
                {
                    return HostAnswer::Trapped(Trap::OverlappingCopy);
                }
                if let Err(trap) = machine.copy(target, source, length) {
                    return HostAnswer::Trapped(trap);
                }
                pointer(target)
            }
            "memset" => {
                let (Some(target), Some(value), Some(length)) = (
                    Self::address(first),
                    Self::number(second),
                    Self::number(third),
                ) else {
                    return HostAnswer::Trapped(Trap::TypeConfusion);
                };
                if let Err(trap) = machine.read(target, length) {
                    return HostAnswer::Trapped(trap);
                }
                let filled = vec![value as u8; length as usize];
                if let Err(trap) = machine.write(target, &filled) {
                    return HostAnswer::Trapped(trap);
                }
                pointer(target)
            }
            "strlen" => {
                let Some(address) = Self::address(first) else {
                    return HostAnswer::Trapped(Trap::TypeConfusion);
                };
                match machine.c_string(address) {
                    Ok(bytes) => integer(bytes.len() as u128),
                    Err(trap) => HostAnswer::Trapped(trap),
                }
            }
            "putchar" => {
                let Some(value) = Self::number(first) else {
                    return HostAnswer::Trapped(Trap::TypeConfusion);
                };
                machine.emit(1, &[value as u8]);
                integer(u128::from(value as u8))
            }
            "puts" => {
                let Some(address) = Self::address(first) else {
                    return HostAnswer::Trapped(Trap::TypeConfusion);
                };
                match machine.c_string(address) {
                    Ok(mut bytes) => {
                        bytes.push(b'\n');
                        machine.emit(1, &bytes);
                        integer(1)
                    }
                    Err(trap) => HostAnswer::Trapped(trap),
                }
            }
            "write" => {
                let (Some(descriptor), Some(address), Some(length)) = (
                    Self::number(first),
                    Self::address(second),
                    Self::number(third),
                ) else {
                    return HostAnswer::Trapped(Trap::TypeConfusion);
                };
                let bytes = match machine.read(address, length) {
                    Ok(bytes) => bytes.to_vec(),
                    Err(trap) => return HostAnswer::Trapped(trap),
                };
                if machine.emit(descriptor as u32, &bytes) {
                    integer(u128::from(length))
                } else {
                    integer(u128::MAX)
                }
            }
            "ir.print" | "ir.print_line" => {
                let Some(handle) = first.and_then(Datum::as_reference) else {
                    return HostAnswer::Trapped(Trap::TypeConfusion);
                };
                let Some(text) = machine.text(handle) else {
                    return HostAnswer::Trapped(if handle == 0 {
                        Trap::NullReference
                    } else {
                        Trap::TypeConfusion
                    });
                };
                let mut bytes = text.as_bytes().to_vec();
                if name == "ir.print_line" {
                    bytes.push(b'\n');
                }
                machine.emit(1, &bytes);
                HostAnswer::Returned(None)
            }
            "abort" => HostAnswer::Trapped(Trap::Abort),
            "exit" => match Self::number(first) {
                Some(code) => HostAnswer::Exited(code as u32 as i32),
                None => HostAnswer::Trapped(Trap::TypeConfusion),
            },
            _ => match Self::runtime(name, arguments, machine) {
                Ok(answer) => HostAnswer::Returned(answer),
                Err(Trap::MissingImplementation) => HostAnswer::Missing,
                Err(trap) => HostAnswer::Trapped(trap),
            },
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Finish {
    Returned(Option<Datum>),
    Exited(i32),
}

#[derive(Debug)]
enum Stop {
    Trap(Trap, u32),
    Fuel,
    Depth,
    Uncaught(u32),
    MissingHost(FunctionId),
    Exit(i32),
}

impl From<Trap> for Stop {
    fn from(trap: Trap) -> Stop {
        Stop::Trap(trap, 0)
    }
}

enum Resume<'m> {
    Top,
    Plain(Option<Value>),
    Invoke {
        result: Option<Value>,
        normal: &'m BlockCall,
        unwind: &'m BlockCall,
    },
}

struct Frame<'m> {
    function: FunctionId,
    block: usize,
    position: usize,
    values: Vec<Option<Datum>>,
    slots: Vec<u64>,
    varargs: Vec<Datum>,
    resume: Resume<'m>,
}

pub struct Interpreter<'m, 'h> {
    module: &'m Module,
    host: &'h mut dyn Host,
    limits: Limits,
    machine: Machine,
    globals: Vec<u64>,
    argument_sets: Vec<Vec<Datum>>,
    frames: Vec<Frame<'m>>,
    fuel: u64,
    liveness: BTreeMap<u32, Vec<Vec<Value>>>,
}

fn terminator_operands(terminator: &Terminator) -> Vec<Value> {
    let mut out = Vec::new();
    match terminator {
        Terminator::Branch { condition, .. } => out.push(*condition),
        Terminator::Switch { value, .. } => out.push(*value),
        Terminator::Return(Some(value)) | Terminator::Throw(value) => out.push(*value),
        Terminator::Invoke { call, .. } => out.extend(call.arguments.iter().copied()),
        _ => {}
    }
    for edge in terminator.successors() {
        out.extend(edge.arguments.iter().copied());
    }
    out
}

fn live_after_blocks(function: &Function) -> Vec<Vec<Value>> {
    let count = function.blocks.len();
    let mut defined: Vec<BTreeSet<Value>> = Vec::with_capacity(count);
    let mut used: Vec<BTreeSet<Value>> = Vec::with_capacity(count);
    for data in &function.blocks {
        let mut made: BTreeSet<Value> = data.parameters.iter().copied().collect();
        let mut wanted = BTreeSet::new();
        for inst in &data.instructions {
            let instruction = &function.instructions[inst.index()];
            for argument in &instruction.arguments {
                if !made.contains(argument) {
                    wanted.insert(*argument);
                }
            }
            if let Some(result) = instruction.result {
                made.insert(result);
            }
        }
        for operand in terminator_operands(&data.terminator) {
            if !made.contains(&operand) {
                wanted.insert(operand);
            }
        }
        if let Terminator::Invoke { call, .. } = &data.terminator {
            if let Some(result) = call.result {
                made.insert(result);
                wanted.remove(&result);
            }
        }
        defined.push(made);
        used.push(wanted);
    }
    let mut live_in: Vec<BTreeSet<Value>> = used.clone();
    let mut live_out: Vec<BTreeSet<Value>> = vec![BTreeSet::new(); count];
    let mut changed = true;
    while changed {
        changed = false;
        for block in (0..count).rev() {
            let mut out = BTreeSet::new();
            for edge in function.blocks[block].terminator.successors() {
                if let Some(seen) = live_in.get(edge.block.index()) {
                    out.extend(seen.iter().copied());
                }
            }
            let mut fresh = used[block].clone();
            for value in &out {
                if !defined[block].contains(value) {
                    fresh.insert(*value);
                }
            }
            if fresh != live_in[block] {
                live_in[block] = fresh;
                changed = true;
            }
            if out != live_out[block] {
                live_out[block] = out;
                changed = true;
            }
        }
    }
    live_out
        .into_iter()
        .map(|set| set.into_iter().collect())
        .collect()
}

fn lane_wise(
    left: &Datum,
    right: &Datum,
    apply: &mut dyn FnMut(&Datum, &Datum) -> Result<Datum, Stop>,
) -> Result<Datum, Stop> {
    match (left, right) {
        (Datum::Poison, _) | (_, Datum::Poison) => Ok(Datum::Poison),
        (Datum::Vector(lefts), Datum::Vector(rights)) => {
            let mut lanes = Vec::with_capacity(lefts.len());
            for (one, other) in lefts.iter().zip(rights) {
                lanes.push(apply(one, other)?);
            }
            Ok(Datum::Vector(lanes))
        }
        _ => apply(left, right),
    }
}

fn lane_map(
    value: &Datum,
    apply: &mut dyn FnMut(&Datum) -> Result<Datum, Stop>,
) -> Result<Datum, Stop> {
    match value {
        Datum::Poison => Ok(Datum::Poison),
        Datum::Vector(lanes) => {
            let mut out = Vec::with_capacity(lanes.len());
            for lane in lanes {
                out.push(apply(lane)?);
            }
            Ok(Datum::Vector(out))
        }
        _ => apply(value),
    }
}

fn integer_of(datum: &Datum) -> Result<u128, Stop> {
    match datum {
        Datum::Integer(value) => Ok(*value),
        Datum::Poison => Err(Stop::from(Trap::PoisonObserved)),
        _ => Err(Stop::from(Trap::TypeConfusion)),
    }
}

fn pointer_of(datum: &Datum) -> Result<u64, Stop> {
    match datum {
        Datum::Pointer(address) => Ok(*address),
        Datum::Poison => Err(Stop::from(Trap::PoisonObserved)),
        _ => Err(Stop::from(Trap::TypeConfusion)),
    }
}

fn reference_of(datum: &Datum) -> Result<u32, Stop> {
    match datum {
        Datum::Reference(handle) => Ok(*handle),
        Datum::Poison => Err(Stop::from(Trap::PoisonObserved)),
        _ => Err(Stop::from(Trap::TypeConfusion)),
    }
}

impl<'m, 'h> Interpreter<'m, 'h> {
    pub fn new(
        module: &'m Module,
        host: &'h mut dyn Host,
        limits: Limits,
    ) -> Result<Interpreter<'m, 'h>, Diagnostic> {
        module.verify()?;
        let layout = module.layout.data();
        let mut machine = Machine::new(layout, &limits);
        let trapped = |trap: Trap| {
            fail("EI004", "The program stopped at a trap.")
                .with_context(format!("Reason: {}", trap.name()))
        };
        let mut globals = Vec::with_capacity(module.globals.len());
        for global in &module.globals {
            let region = if global.mutable || global.linkage == Linkage::Import {
                Region::Global
            } else {
                Region::Constant
            };
            let address = machine
                .allocate_in(u64::from(global.size), u64::from(global.align), region)
                .map_err(trapped)?;
            globals.push(address);
        }
        for (global, address) in module.globals.iter().zip(&globals) {
            let bytes = if global.linkage == Linkage::Import {
                match host.global(&global.name, global.size) {
                    Some(bytes) if bytes.len() == global.size as usize => bytes,
                    _ => {
                        return Err(fail(
                            "EI007",
                            "The program called a function the host does not provide.",
                        )
                        .with_context(format!("Name: {}", global_symbol(&global.name))));
                    }
                }
            } else {
                match &global.content {
                    GlobalContent::Zero => Vec::new(),
                    GlobalContent::Bytes(bytes) => bytes.clone(),
                }
            };
            machine
                .write_bytes(*address, &bytes, true)
                .map_err(trapped)?;
            for relocation in &global.relocations {
                let target = match relocation.target {
                    Symbol::Global(id) => globals[id.index()],
                    Symbol::Function(id) => machine.function_address(id),
                };
                let value = target.wrapping_add(relocation.addend as u64);
                machine
                    .store(
                        &Datum::Pointer(value),
                        Type::POINTER,
                        address + u64::from(relocation.offset),
                        1,
                        true,
                    )
                    .map_err(trapped)?;
            }
        }
        Ok(Interpreter {
            module,
            host,
            limits,
            machine,
            globals,
            argument_sets: Vec::new(),
            frames: Vec::new(),
            fuel: 0,
            liveness: BTreeMap::new(),
        })
    }

    pub fn machine(&self) -> &Machine {
        &self.machine
    }

    pub fn machine_mut(&mut self) -> &mut Machine {
        &mut self.machine
    }

    pub fn global_address(&self, global: GlobalId) -> Option<u64> {
        self.globals.get(global.index()).copied()
    }

    pub fn fuel_used(&self) -> u64 {
        self.fuel
    }

    pub fn call(
        &mut self,
        function: FunctionId,
        arguments: Vec<Datum>,
    ) -> Result<Finish, Diagnostic> {
        self.unwind_all();
        let outcome = self.run(function, arguments);
        let at = self
            .frames
            .last()
            .map(|frame| frame.function)
            .unwrap_or(function);
        self.unwind_all();
        match outcome {
            Ok(value) => Ok(Finish::Returned(value)),
            Err(stop) => self.report(stop, at),
        }
    }

    fn report(&self, stop: Stop, at: FunctionId) -> Result<Finish, Diagnostic> {
        let name = |id: FunctionId| {
            self.module
                .functions
                .get(id.index())
                .map(|function| global_symbol(&function.name))
                .unwrap_or_default()
        };
        Err(match stop {
            Stop::Exit(code) => return Ok(Finish::Exited(code)),
            Stop::Trap(trap, code) => {
                let mut diagnostic = fail("EI004", "The program stopped at a trap.")
                    .with_context(format!("Reason: {}", trap.name()))
                    .with_context(format!("Name: {}", name(at)));
                if trap == Trap::Explicit {
                    diagnostic = diagnostic.with_context(format!("Value: {code}"));
                }
                diagnostic
            }
            Stop::Fuel => fail("EI005", "The program ran longer than it was allowed to.")
                .with_context(format!("Limit: {}", self.limits.fuel)),
            Stop::Uncaught(handle) => {
                let diagnostic = fail("EI006", "The program threw an exception nothing caught.");
                match self.machine.object(handle) {
                    Some(Object::Instance { class, .. }) => diagnostic.with_context(format!(
                        "Class: {}",
                        local_symbol(&self.module.classes[class.index()].name)
                    )),
                    _ => diagnostic,
                }
            }
            Stop::MissingHost(id) => fail(
                "EI007",
                "The program called a function the host does not provide.",
            )
            .with_context(format!("Name: {}", name(id))),
            Stop::Depth => fail(
                "EI008",
                "The program used more stack than it was allowed to.",
            )
            .with_context(format!("Limit: {}", self.limits.depth)),
        })
    }

    fn unwind_all(&mut self) {
        while let Some(frame) = self.frames.pop() {
            self.free_slots(&frame);
        }
    }

    fn free_slots(&mut self, frame: &Frame<'m>) {
        for address in &frame.slots {
            if let Some(index) = self.machine.starts.get(address).copied() {
                if self.machine.allocations[index].live {
                    self.machine.retire(index);
                }
            }
        }
    }

    fn make_room(&mut self, bytes: u64) {
        if !self.machine.pressed(bytes) {
            return;
        }
        let module = self.module;
        let mut roots = Vec::new();
        for at in 0..self.frames.len() {
            let function_id = self.frames[at].function;
            let function = &module.functions[function_id.index()];
            let after = self
                .liveness
                .entry(function_id.0)
                .or_insert_with(|| live_after_blocks(function));
            let block = self.frames[at].block;
            let data = &function.blocks[block];
            let mut wanted: Vec<Value> = after.get(block).cloned().unwrap_or_default();
            let position = self.frames[at].position.min(data.instructions.len());
            for inst in &data.instructions[position..] {
                wanted.extend(
                    function.instructions[inst.index()]
                        .arguments
                        .iter()
                        .copied(),
                );
            }
            wanted.extend(terminator_operands(&data.terminator));
            for value in wanted {
                if let Some(Some(datum)) = self.frames[at].values.get(value.index()) {
                    Machine::gather(datum, &mut roots);
                }
            }
        }
        self.machine.collect(roots);
    }

    fn run(&mut self, function: FunctionId, arguments: Vec<Datum>) -> Result<Option<Datum>, Stop> {
        let module = self.module;
        let Some(target) = module.functions.get(function.index()) else {
            return Err(Stop::from(Trap::BadFunctionPointer));
        };
        let pointer_bytes = self.machine.pointer_bytes;
        let fits = arguments.len() == target.signature.parameters.len()
            && arguments
                .iter()
                .zip(&target.signature.parameters)
                .all(|(datum, ty)| datum.fits(*ty, pointer_bytes));
        if !fits {
            return Err(Stop::from(Trap::SignatureMismatch));
        }
        if target.is_declaration() {
            return self.host_call(function, &arguments);
        }
        self.enter(function, arguments, Resume::Top)?;
        loop {
            self.fuel += 1;
            if self.fuel > self.limits.fuel {
                return Err(Stop::Fuel);
            }
            let (function_id, block, position) = match self.frames.last() {
                Some(frame) => (frame.function, frame.block, frame.position),
                None => return Ok(None),
            };
            let function = &module.functions[function_id.index()];
            let data = &function.blocks[block];
            if position < data.instructions.len() {
                let instruction = &function.instructions[data.instructions[position].index()];
                self.step(instruction)?;
            } else if let Some(returned) = self.terminate(&data.terminator)? {
                return Ok(returned);
            }
        }
    }

    fn enter(
        &mut self,
        function: FunctionId,
        arguments: Vec<Datum>,
        resume: Resume<'m>,
    ) -> Result<(), Stop> {
        if self.frames.len() >= self.limits.depth {
            return Err(Stop::Depth);
        }
        let target = &self.module.functions[function.index()];
        let mut values = vec![None; target.value_types.len()];
        let mut varargs = Vec::new();
        let parameters = &target.blocks[0].parameters;
        for (index, argument) in arguments.into_iter().enumerate() {
            match parameters.get(index) {
                Some(parameter) => values[parameter.index()] = Some(argument),
                None => varargs.push(argument),
            }
        }
        self.frames.push(Frame {
            function,
            block: 0,
            position: 0,
            values,
            slots: Vec::new(),
            varargs,
            resume,
        });
        Ok(())
    }

    fn frame(&mut self) -> &mut Frame<'m> {
        self.frames
            .last_mut()
            .expect("the interpreter always runs inside a frame")
    }

    fn get(&self, value: Value) -> Datum {
        self.frames
            .last()
            .and_then(|frame| frame.values.get(value.index()))
            .cloned()
            .flatten()
            .unwrap_or(Datum::Poison)
    }

    fn set(&mut self, value: Option<Value>, datum: Datum) {
        if let Some(value) = value {
            if let Some(slot) = self.frame().values.get_mut(value.index()) {
                *slot = Some(datum);
            }
        }
    }

    fn type_of(&self, value: Value) -> Type {
        let function = self
            .frames
            .last()
            .map(|frame| frame.function)
            .unwrap_or(FunctionId(0));
        self.module.functions[function.index()].value_types[value.index()]
    }

    fn take_edge(&mut self, edge: &BlockCall, prefix: Option<Datum>) {
        let mut incoming: Vec<Datum> = prefix.into_iter().collect();
        incoming.extend(edge.arguments.iter().map(|value| self.get(*value)));
        let function = self.frame().function;
        let parameters =
            &self.module.functions[function.index()].blocks[edge.block.index()].parameters;
        let frame = self
            .frames
            .last_mut()
            .expect("the interpreter always runs inside a frame");
        for (parameter, datum) in parameters.iter().zip(incoming) {
            frame.values[parameter.index()] = Some(datum);
        }
        frame.block = edge.block.index();
        frame.position = 0;
    }

    fn leave(&mut self, value: Option<Datum>) -> Option<Option<Datum>> {
        let frame = self
            .frames
            .pop()
            .expect("the interpreter always runs inside a frame");
        self.free_slots(&frame);
        match frame.resume {
            Resume::Top => Some(value),
            Resume::Plain(result) => {
                if let Some(datum) = value {
                    self.set(result, datum);
                }
                self.frame().position += 1;
                None
            }
            Resume::Invoke { result, normal, .. } => {
                if let Some(datum) = value {
                    self.set(result, datum);
                }
                self.take_edge(normal, None);
                None
            }
        }
    }

    fn throw(&mut self, exception: u32) -> Result<(), Stop> {
        loop {
            let Some(frame) = self.frames.pop() else {
                return Err(Stop::Uncaught(exception));
            };
            self.free_slots(&frame);
            match frame.resume {
                Resume::Top => {
                    self.frames.push(frame);
                    return Err(Stop::Uncaught(exception));
                }
                Resume::Plain(_) => continue,
                Resume::Invoke { unwind, .. } => {
                    self.take_edge(unwind, Some(Datum::Reference(exception)));
                    return Ok(());
                }
            }
        }
    }

    fn host_call(
        &mut self,
        function: FunctionId,
        arguments: &[Datum],
    ) -> Result<Option<Datum>, Stop> {
        let target = &self.module.functions[function.index()];
        if arguments.iter().any(|datum| matches!(datum, Datum::Poison)) {
            return Err(Stop::from(Trap::PoisonObserved));
        }
        match self.host.call(&target.name, arguments, &mut self.machine) {
            HostAnswer::Returned(value) => {
                let pointer_bytes = self.machine.pointer_bytes;
                match (value, target.signature.result) {
                    (None, None) => Ok(None),
                    (Some(datum), Some(ty)) => {
                        let datum = match (datum, ty) {
                            (Datum::Integer(raw), Type::Scalar(scalar)) if scalar.is_integer() => {
                                Datum::Integer(raw & mask(scalar.bits()))
                            }
                            (other, _) => other,
                        };
                        if datum.fits(ty, pointer_bytes) {
                            Ok(Some(datum))
                        } else {
                            Err(Stop::from(Trap::SignatureMismatch))
                        }
                    }
                    _ => Err(Stop::from(Trap::SignatureMismatch)),
                }
            }
            HostAnswer::Missing => Err(Stop::MissingHost(function)),
            HostAnswer::Trapped(trap) => Err(Stop::from(trap)),
            HostAnswer::Exited(code) => Err(Stop::Exit(code)),
        }
    }

    fn resolve_call(&self, instruction: &Instruction) -> Result<(FunctionId, Vec<Datum>), Stop> {
        let arguments: Vec<Datum> = instruction
            .arguments
            .iter()
            .map(|value| self.get(*value))
            .collect();
        match &instruction.operation {
            Operation::Call(callee) => Ok((*callee, arguments)),
            Operation::CallIndirect(signature) => {
                let address = pointer_of(&arguments[0])?;
                let base = self.machine.function_base();
                let index = address
                    .checked_sub(base)
                    .filter(|offset| offset.is_multiple_of(FUNCTION_STRIDE));
                let Some(index) = index.map(|offset| offset / FUNCTION_STRIDE) else {
                    return Err(Stop::from(Trap::BadFunctionPointer));
                };
                let Some(target) = self.module.functions.get(index as usize) else {
                    return Err(Stop::from(Trap::BadFunctionPointer));
                };
                if target.signature != *signature {
                    return Err(Stop::from(Trap::SignatureMismatch));
                }
                Ok((FunctionId(index as u32), arguments[1..].to_vec()))
            }
            _ => Err(Stop::from(Trap::BadFunctionPointer)),
        }
    }

    fn step(&mut self, instruction: &'m Instruction) -> Result<(), Stop> {
        if matches!(
            instruction.operation,
            Operation::Call(_) | Operation::CallIndirect(_)
        ) {
            let (callee, arguments) = self.resolve_call(instruction)?;
            if self.module.functions[callee.index()].is_declaration() {
                let value = self.host_call(callee, &arguments)?;
                if let Some(datum) = value {
                    self.set(instruction.result, datum);
                }
                self.frame().position += 1;
            } else {
                self.enter(callee, arguments, Resume::Plain(instruction.result))?;
            }
            return Ok(());
        }
        if let Operation::Trap(code) = instruction.operation {
            return Err(Stop::Trap(Trap::Explicit, code));
        }
        let value = self.evaluate(instruction)?;
        if let Some(datum) = value {
            self.set(instruction.result, datum);
        }
        self.frame().position += 1;
        Ok(())
    }

    fn terminate(&mut self, terminator: &'m Terminator) -> Result<Option<Option<Datum>>, Stop> {
        match terminator {
            Terminator::Jump(edge) => self.take_edge(edge, None),
            Terminator::Branch {
                condition,
                taken,
                otherwise,
            } => {
                let chosen = if integer_of(&self.get(*condition))? != 0 {
                    taken
                } else {
                    otherwise
                };
                self.take_edge(chosen, None);
            }
            Terminator::Switch {
                value,
                cases,
                default,
            } => {
                let ty = self.type_of(*value);
                let bits = ty.element().bits();
                let scrutinee = integer_of(&self.get(*value))?;
                let chosen = cases
                    .iter()
                    .find(|(case, _)| (*case as u128) & mask(bits) == scrutinee)
                    .map(|(_, edge)| edge)
                    .unwrap_or(default);
                self.take_edge(chosen, None);
            }
            Terminator::Return(value) => {
                let datum = value.map(|value| self.get(value));
                return Ok(self.leave(datum));
            }
            Terminator::Unreachable => return Err(Stop::from(Trap::Unreachable)),
            Terminator::Throw(value) => {
                let handle = reference_of(&self.get(*value))?;
                if handle == 0 {
                    return Err(Stop::from(Trap::NullReference));
                }
                self.throw(handle)?;
            }
            Terminator::Invoke {
                call,
                normal,
                unwind,
            } => {
                let (callee, arguments) = self.resolve_call(call)?;
                if self.module.functions[callee.index()].is_declaration() {
                    let value = self.host_call(callee, &arguments)?;
                    if let Some(datum) = value {
                        self.set(call.result, datum);
                    }
                    self.take_edge(normal, None);
                } else {
                    self.enter(
                        callee,
                        arguments,
                        Resume::Invoke {
                            result: call.result,
                            normal,
                            unwind,
                        },
                    )?;
                }
            }
        }
        Ok(None)
    }

    fn evaluate(&mut self, instruction: &Instruction) -> Result<Option<Datum>, Stop> {
        let module = self.module;
        let layout = self.machine.layout;
        let arguments: Vec<Datum> = instruction
            .arguments
            .iter()
            .map(|value| self.get(*value))
            .collect();
        let argument_types: Vec<Type> = instruction
            .arguments
            .iter()
            .map(|value| self.type_of(*value))
            .collect();
        let result_type = instruction.result.map(|value| self.type_of(value));
        let pointer_mask = mask(self.machine.pointer_bytes * 8) as u64;
        let datum = match &instruction.operation {
            Operation::IntegerConstant(value) => Datum::Integer(
                (*value as u128) & mask(result_type.unwrap_or(Type::I128).element().bits()),
            ),
            Operation::FloatConstant(bits) => Datum::Float(*bits),
            Operation::Null => match result_type {
                Some(Type::Scalar(Scalar::Reference)) => Datum::Reference(0),
                _ => Datum::Pointer(0),
            },
            Operation::Poison => Datum::Poison,
            Operation::GlobalAddress(id) => Datum::Pointer(self.globals[id.index()]),
            Operation::FunctionAddress(id) => Datum::Pointer(self.machine.function_address(*id)),
            Operation::StringConstant(index) => {
                if let Some(handle) = self.machine.texts.get(index) {
                    Datum::Reference(*handle)
                } else {
                    let text = module.strings[*index as usize].clone();
                    let datum = self.machine.make_text(&text)?;
                    if let Datum::Reference(handle) = datum {
                        self.machine.texts.insert(*index, handle);
                    }
                    datum
                }
            }
            Operation::Binary(op) => {
                let element = argument_types[0].element();
                let op = *op;
                lane_wise(&arguments[0], &arguments[1], &mut |left, right| {
                    if op.is_float() {
                        let (Datum::Float(left), Datum::Float(right)) = (left, right) else {
                            return Err(Stop::from(Trap::TypeConfusion));
                        };
                        Ok(Datum::Float(if element == Scalar::F32 {
                            u64::from(single_binary(
                                op,
                                f32::from_bits(*left as u32),
                                f32::from_bits(*right as u32),
                            ))
                        } else {
                            double_binary(op, f64::from_bits(*left), f64::from_bits(*right))
                        }))
                    } else {
                        integer_binary(op, integer_of(left)?, integer_of(right)?, element.bits())
                            .map(Datum::Integer)
                            .map_err(Stop::from)
                    }
                })?
            }
            Operation::Unary(op) => {
                let element = argument_types[0].element();
                let op = *op;
                lane_map(&arguments[0], &mut |value| {
                    if op.is_float() {
                        let Datum::Float(bits) = value else {
                            return Err(Stop::from(Trap::TypeConfusion));
                        };
                        Ok(Datum::Float(if element == Scalar::F32 {
                            u64::from(single_unary(op, f32::from_bits(*bits as u32)))
                        } else {
                            double_unary(op, f64::from_bits(*bits))
                        }))
                    } else {
                        Ok(Datum::Integer(integer_unary(
                            op,
                            integer_of(value)?,
                            element.bits(),
                        )))
                    }
                })?
            }
            Operation::Compare(comparison) => {
                if arguments.iter().any(|datum| matches!(datum, Datum::Poison)) {
                    Datum::Poison
                } else {
                    let bits = argument_types[0].element().bits();
                    let raw = |datum: &Datum| -> u128 {
                        match datum {
                            Datum::Integer(value) => *value,
                            Datum::Pointer(address) => u128::from(*address),
                            Datum::Reference(handle) => u128::from(*handle),
                            _ => 0,
                        }
                    };
                    let (left, right) = (raw(&arguments[0]), raw(&arguments[1]));
                    let (left_signed, right_signed) = (signed(left, bits), signed(right, bits));
                    let holds = match comparison {
                        Comparison::Equal => left == right,
                        Comparison::NotEqual => left != right,
                        Comparison::LessSigned => left_signed < right_signed,
                        Comparison::LessEqualSigned => left_signed <= right_signed,
                        Comparison::GreaterSigned => left_signed > right_signed,
                        Comparison::GreaterEqualSigned => left_signed >= right_signed,
                        Comparison::LessUnsigned => left < right,
                        Comparison::LessEqualUnsigned => left <= right,
                        Comparison::GreaterUnsigned => left > right,
                        Comparison::GreaterEqualUnsigned => left >= right,
                    };
                    Datum::Integer(u128::from(holds))
                }
            }
            Operation::FloatCompare(comparison) => match (&arguments[0], &arguments[1]) {
                (Datum::Float(left), Datum::Float(right)) => {
                    let holds = if argument_types[0] == Type::F32 {
                        single_compare(
                            *comparison,
                            f32::from_bits(*left as u32),
                            f32::from_bits(*right as u32),
                        )
                    } else {
                        double_compare(*comparison, f64::from_bits(*left), f64::from_bits(*right))
                    };
                    Datum::Integer(u128::from(holds))
                }
                _ => Datum::Poison,
            },
            Operation::Convert(conversion) => {
                let to = result_type.unwrap_or(Type::BOOL);
                self.convert(*conversion, &arguments[0], argument_types[0], to)?
            }
            Operation::Select => match &arguments[0] {
                Datum::Poison => return Err(Stop::from(Trap::PoisonObserved)),
                condition => {
                    if integer_of(condition)? != 0 {
                        arguments[1].clone()
                    } else {
                        arguments[2].clone()
                    }
                }
            },
            Operation::Splat => match &arguments[0] {
                Datum::Poison => Datum::Poison,
                lane => Datum::Vector(vec![
                    lane.clone();
                    result_type.map(Type::lanes).unwrap_or(1) as usize
                ]),
            },
            Operation::ExtractLane(lane) => match &arguments[0] {
                Datum::Vector(lanes) => lanes
                    .get(usize::from(*lane))
                    .cloned()
                    .unwrap_or(Datum::Poison),
                _ => Datum::Poison,
            },
            Operation::InsertLane(lane) => match (&arguments[0], &arguments[1]) {
                (Datum::Vector(lanes), inserted) if !matches!(inserted, Datum::Poison) => {
                    let mut lanes = lanes.clone();
                    if let Some(slot) = lanes.get_mut(usize::from(*lane)) {
                        *slot = inserted.clone();
                    }
                    Datum::Vector(lanes)
                }
                _ => Datum::Poison,
            },
            Operation::StackSlot { size, align } => {
                let address =
                    self.machine
                        .allocate_in(u64::from(*size), u64::from(*align), Region::Stack)?;
                self.frame().slots.push(address);
                Datum::Pointer(address)
            }
            Operation::DynamicStackSlot { align } => {
                let size = integer_of(&arguments[0])?;
                if size > u128::from(u32::MAX) {
                    return Err(Trap::MemoryExhausted.into());
                }
                let address =
                    self.machine
                        .allocate_in(size as u64, u64::from(*align), Region::Stack)?;
                self.frame().slots.push(address);
                Datum::Pointer(address)
            }
            Operation::VaStart => {
                let list = pointer_of(&arguments[0])?;
                let set = self.frame().varargs.clone();
                self.argument_sets.push(set);
                let mut header = [0u8; 16];
                header[..8].copy_from_slice(&(self.argument_sets.len() as u64).to_le_bytes());
                self.machine.write(list, &header)?;
                return Ok(None);
            }
            Operation::VaCopy => {
                let destination = pointer_of(&arguments[0])?;
                let source = pointer_of(&arguments[1])?;
                let bytes = self.machine.read(source, 32)?.to_vec();
                self.machine.write(destination, &bytes)?;
                return Ok(None);
            }
            Operation::VaArg => {
                let list = pointer_of(&arguments[0])?;
                let header = self.machine.read(list, 16)?.to_vec();
                let mut word = [0u8; 8];
                word.copy_from_slice(&header[..8]);
                let identifier = u64::from_le_bytes(word);
                word.copy_from_slice(&header[8..16]);
                let cursor = u64::from_le_bytes(word);
                let given = identifier
                    .checked_sub(1)
                    .and_then(|set| self.argument_sets.get(set as usize))
                    .and_then(|set| set.get(cursor as usize))
                    .cloned()
                    .ok_or(Trap::OutOfBounds)?;
                let wanted = result_type.unwrap_or(Type::I64);
                let datum = match (&given, wanted.element()) {
                    (Datum::Integer(value), scalar) if scalar.is_integer() => {
                        Datum::Integer(*value & mask(scalar.bits()))
                    }
                    (Datum::Integer(value), Scalar::Pointer) => Datum::Pointer(*value as u64),
                    (Datum::Pointer(address), Scalar::Pointer) => Datum::Pointer(*address),
                    (Datum::Pointer(address), scalar) if scalar.is_integer() => {
                        Datum::Integer(u128::from(*address) & mask(scalar.bits()))
                    }
                    (Datum::Float(bits), Scalar::F64) => Datum::Float(*bits),
                    (Datum::Float(bits), Scalar::F32) => Datum::f32(f64::from_bits(*bits) as f32),
                    _ => return Err(Trap::SignatureMismatch.into()),
                };
                self.machine.write(list + 8, &(cursor + 1).to_le_bytes())?;
                datum
            }
            Operation::StackSave => Datum::Pointer(self.frame().slots.len() as u64),
            Operation::StackRestore => {
                let keep = pointer_of(&arguments[0])? as usize;
                while self.frame().slots.len() > keep {
                    if let Some(address) = self.frame().slots.pop() {
                        if let Some(index) = self.machine.starts.get(&address).copied() {
                            if self.machine.allocations[index].live {
                                self.machine.retire(index);
                            }
                        }
                    }
                }
                return Ok(None);
            }
            Operation::Load {
                align, ordering, ..
            } => {
                let ty = result_type.unwrap_or(Type::BOOL);
                let address = pointer_of(&arguments[0])?;
                let natural = if ordering.is_some() {
                    u64::from(layout.size_of(ty))
                } else {
                    1
                };
                if natural > 1 && !address.is_multiple_of(natural) {
                    return Err(Stop::from(Trap::Misaligned));
                }
                self.machine.load(ty, address, u64::from(*align))?
            }
            Operation::Store {
                align, ordering, ..
            } => {
                let ty = argument_types[0];
                let address = pointer_of(&arguments[1])?;
                let natural = if ordering.is_some() {
                    u64::from(layout.size_of(ty))
                } else {
                    1
                };
                if natural > 1 && !address.is_multiple_of(natural) {
                    return Err(Stop::from(Trap::Misaligned));
                }
                self.machine
                    .store(&arguments[0], ty, address, u64::from(*align), false)?;
                return Ok(None);
            }
            Operation::Offset => {
                let address = pointer_of(&arguments[0])?;
                let bits = argument_types[1].element().bits();
                let delta = signed(integer_of(&arguments[1])?, bits);
                Datum::Pointer(address.wrapping_add(delta as u64) & pointer_mask)
            }
            Operation::ElementAddress(ty) => {
                let address = pointer_of(&arguments[0])?;
                let bits = argument_types[1].element().bits();
                let index = signed(integer_of(&arguments[1])?, bits);
                let stride = i128::from(layout.size_of(*ty));
                Datum::Pointer(
                    address.wrapping_add(index.wrapping_mul(stride) as u64) & pointer_mask,
                )
            }
            Operation::FieldAddress(structure, index) => {
                let address = pointer_of(&arguments[0])?;
                let (offsets, _, _) = layout.struct_layout(&module.structs[structure.index()]);
                Datum::Pointer(
                    address.wrapping_add(u64::from(offsets[*index as usize])) & pointer_mask,
                )
            }
            Operation::AtomicUpdate(op, _) => {
                let ty = argument_types[1];
                let address = pointer_of(&arguments[0])?;
                let size = u64::from(layout.size_of(ty));
                let old = self.machine.load(ty, address, size)?;
                let operand = &arguments[1];
                let bits = ty.element().bits();
                let fresh = match (op, &old, operand) {
                    (AtomicOp::Exchange, _, _) => operand.clone(),
                    (_, Datum::Integer(left), Datum::Integer(right)) => {
                        let (left, right) = (*left, *right);
                        let (left_signed, right_signed) = (signed(left, bits), signed(right, bits));
                        let value = match op {
                            AtomicOp::Add => left.wrapping_add(right),
                            AtomicOp::Subtract => left.wrapping_sub(right),
                            AtomicOp::And => left & right,
                            AtomicOp::Nand => !(left & right),
                            AtomicOp::Or => left | right,
                            AtomicOp::Xor => left ^ right,
                            AtomicOp::MinimumSigned => {
                                if left_signed <= right_signed {
                                    left
                                } else {
                                    right
                                }
                            }
                            AtomicOp::MaximumSigned => {
                                if left_signed >= right_signed {
                                    left
                                } else {
                                    right
                                }
                            }
                            AtomicOp::MinimumUnsigned => left.min(right),
                            AtomicOp::MaximumUnsigned => left.max(right),
                            AtomicOp::Exchange => right,
                        };
                        Datum::Integer(value & mask(bits))
                    }
                    _ => return Err(Stop::from(Trap::PoisonObserved)),
                };
                self.machine.store(&fresh, ty, address, size, false)?;
                old
            }
            Operation::CompareExchange(..) => {
                let ty = argument_types[1];
                let address = pointer_of(&arguments[0])?;
                let size = u64::from(layout.size_of(ty));
                let old = self.machine.load(ty, address, size)?;
                if matches!(arguments[1], Datum::Poison) {
                    return Err(Stop::from(Trap::PoisonObserved));
                }
                if old == arguments[1] {
                    self.machine
                        .store(&arguments[2], ty, address, size, false)?;
                }
                old
            }
            Operation::Fence(_) => return Ok(None),
            Operation::MemoryCopy | Operation::MemoryMove => {
                let target = pointer_of(&arguments[0])?;
                let source = pointer_of(&arguments[1])?;
                let length = integer_of(&arguments[2])? as u64;
                if matches!(instruction.operation, Operation::MemoryCopy)
                    && length > 0
                    && target < source.saturating_add(length)
                    && source < target.saturating_add(length)
                {
                    return Err(Stop::from(Trap::OverlappingCopy));
                }
                self.machine.copy(target, source, length)?;
                return Ok(None);
            }
            Operation::MemoryFill => {
                let target = pointer_of(&arguments[0])?;
                let byte = integer_of(&arguments[1])? as u8;
                let length = integer_of(&arguments[2])? as u64;
                if length > 0 {
                    self.machine.find(target, length)?;
                    let filled = vec![byte; length as usize];
                    self.machine.write(target, &filled)?;
                }
                return Ok(None);
            }
            Operation::Allocate(class) => {
                let fields: Vec<Datum> = module
                    .field_types(*class)
                    .into_iter()
                    .map(Datum::zero)
                    .collect();
                let bytes = 16 + (std::mem::size_of::<Datum>() * fields.len()) as u64;
                self.make_room(bytes);
                Datum::Reference(self.machine.new_object(
                    Object::Instance {
                        class: *class,
                        fields,
                    },
                    bytes,
                )?)
            }
            Operation::AllocateArray(class) => {
                let ty = &module.classes[class.index()].element.unwrap_or(Type::I8);
                let bits = argument_types[0].element().bits();
                let length = signed(integer_of(&arguments[0])?, bits);
                if length < 0 {
                    return Err(Stop::from(Trap::NegativeLength));
                }
                let item = std::mem::size_of::<Datum>() as u128 + u128::from(layout.size_of(*ty));
                let bytes = (length as u128)
                    .saturating_mul(item)
                    .saturating_add(16)
                    .min(u128::from(u64::MAX)) as u64;
                self.make_room(bytes);
                if self.machine.pressed(bytes) {
                    return Err(Stop::from(Trap::MemoryExhausted));
                }
                let items = vec![Datum::zero(*ty); length as usize];
                Datum::Reference(self.machine.new_object(
                    Object::Array {
                        class: *class,
                        element: *ty,
                        items,
                    },
                    bytes,
                )?)
            }
            Operation::LoadField(class, index) | Operation::StoreField(class, index) => {
                let handle = reference_of(&arguments[0])?;
                if handle == 0 {
                    return Err(Stop::from(Trap::NullReference));
                }
                let owner = match self.machine.object(handle) {
                    Some(Object::Instance { class: owner, .. }) => *owner,
                    _ => return Err(Stop::from(Trap::TypeConfusion)),
                };
                if !module.is_subclass(owner, *class) {
                    return Err(Stop::from(Trap::TypeConfusion));
                }
                let storing = matches!(instruction.operation, Operation::StoreField(..));
                let Some(Object::Instance { fields, .. }) = self.machine.object_mut(handle) else {
                    return Err(Stop::from(Trap::TypeConfusion));
                };
                let slot = fields
                    .get_mut(*index as usize)
                    .ok_or(Stop::from(Trap::TypeConfusion))?;
                if storing {
                    *slot = arguments[1].clone();
                    return Ok(None);
                }
                slot.clone()
            }
            Operation::LoadElement(ty) | Operation::StoreElement(ty) => {
                let handle = reference_of(&arguments[0])?;
                if handle == 0 {
                    return Err(Stop::from(Trap::NullReference));
                }
                let bits = argument_types[1].element().bits();
                let index = signed(integer_of(&arguments[1])?, bits);
                let storing = matches!(instruction.operation, Operation::StoreElement(_));
                let Some(Object::Array { element, items, .. }) = self.machine.object_mut(handle)
                else {
                    return Err(Stop::from(Trap::TypeConfusion));
                };
                if *element != *ty {
                    return Err(Stop::from(Trap::TypeConfusion));
                }
                if index < 0 || index as u128 >= items.len() as u128 {
                    return Err(Stop::from(Trap::IndexOutOfBounds));
                }
                if storing {
                    items[index as usize] = arguments[2].clone();
                    return Ok(None);
                }
                items[index as usize].clone()
            }
            Operation::ArrayLength => {
                let handle = reference_of(&arguments[0])?;
                if handle == 0 {
                    return Err(Stop::from(Trap::NullReference));
                }
                let Some(Object::Array { items, .. }) = self.machine.object(handle) else {
                    return Err(Stop::from(Trap::TypeConfusion));
                };
                let bits = result_type.unwrap_or(Type::I32).element().bits();
                Datum::Integer((items.len() as u128) & mask(bits))
            }
            Operation::InstanceOf(class) => {
                let handle = reference_of(&arguments[0])?;
                let holds = self
                    .machine
                    .class_of(handle)
                    .is_some_and(|owner| module.is_subclass(owner, *class));
                Datum::Integer(u128::from(holds))
            }
            Operation::VirtualMethod(class, slot) | Operation::InterfaceMethod(class, slot) => {
                let handle = reference_of(&arguments[0])?;
                if handle == 0 {
                    return Err(Stop::from(Trap::NullReference));
                }
                let Some(owner) = self.machine.class_of(handle) else {
                    return Err(Stop::from(Trap::TypeConfusion));
                };
                let chosen = if matches!(instruction.operation, Operation::VirtualMethod(..)) {
                    if !module.is_subclass(owner, *class) {
                        return Err(Stop::from(Trap::TypeConfusion));
                    }
                    module.classes[owner.index()]
                        .methods
                        .get(*slot as usize)
                        .copied()
                        .flatten()
                } else {
                    let mut found = None;
                    let mut current = Some(owner);
                    let mut steps = 0;
                    while let Some(at) = current {
                        let data = &module.classes[at.index()];
                        if let Some((_, slots)) = data
                            .implementations
                            .iter()
                            .find(|(interface, _)| interface == class)
                        {
                            found = Some(slots.get(*slot as usize).copied().flatten());
                            break;
                        }
                        current = data.parent;
                        steps += 1;
                        if steps > module.classes.len() {
                            break;
                        }
                    }
                    match found {
                        None => return Err(Stop::from(Trap::MissingImplementation)),
                        Some(Some(function)) => Some(function),
                        Some(None) => module.classes[class.index()]
                            .methods
                            .get(*slot as usize)
                            .copied()
                            .flatten(),
                    }
                };
                match chosen {
                    Some(function) => Datum::Pointer(self.machine.function_address(function)),
                    None => return Err(Stop::from(Trap::AbstractMethod)),
                }
            }
            Operation::SameObject => {
                let left = reference_of(&arguments[0])?;
                let right = reference_of(&arguments[1])?;
                Datum::Integer(u128::from(left == right))
            }
            Operation::Call(_) | Operation::CallIndirect(_) | Operation::Trap(_) => {
                return Ok(None)
            }
        };
        Ok(Some(datum))
    }

    fn convert(
        &mut self,
        conversion: Conversion,
        value: &Datum,
        from: Type,
        to: Type,
    ) -> Result<Datum, Stop> {
        let source = from.element();
        let target = to.element();
        let pointer_mask = mask(self.machine.pointer_bytes * 8);
        if conversion == Conversion::Bitcast {
            if matches!(value, Datum::Poison) {
                return Ok(Datum::Poison);
            }
            let mut bytes = Vec::new();
            self.machine.encode(value, from, &mut bytes)?;
            return Ok(self.machine.decode(&bytes, to));
        }
        lane_map(value, &mut |lane| {
            Ok(match conversion {
                Conversion::ZeroExtend => Datum::Integer(integer_of(lane)?),
                Conversion::SignExtend => Datum::Integer(
                    (signed(integer_of(lane)?, source.bits()) as u128) & mask(target.bits()),
                ),
                Conversion::Truncate => Datum::Integer(integer_of(lane)? & mask(target.bits())),
                Conversion::FloatExtend | Conversion::FloatTruncate => {
                    let Datum::Float(bits) = lane else {
                        return Err(Stop::from(Trap::TypeConfusion));
                    };
                    Datum::Float(float_bits(float_value(*bits, source), target))
                }
                Conversion::SignedToFloat => {
                    let number = signed(integer_of(lane)?, source.bits());
                    Datum::Float(if target == Scalar::F32 {
                        u64::from((number as f32).to_bits())
                    } else {
                        (number as f64).to_bits()
                    })
                }
                Conversion::UnsignedToFloat => {
                    let number = integer_of(lane)?;
                    Datum::Float(if target == Scalar::F32 {
                        u64::from((number as f32).to_bits())
                    } else {
                        (number as f64).to_bits()
                    })
                }
                Conversion::FloatToSigned
                | Conversion::FloatToUnsigned
                | Conversion::FloatToSignedSaturating
                | Conversion::FloatToUnsignedSaturating => {
                    let Datum::Float(bits) = lane else {
                        return Err(Stop::from(Trap::TypeConfusion));
                    };
                    let signed_target = matches!(
                        conversion,
                        Conversion::FloatToSigned | Conversion::FloatToSignedSaturating
                    );
                    let saturating = matches!(
                        conversion,
                        Conversion::FloatToSignedSaturating | Conversion::FloatToUnsignedSaturating
                    );
                    Datum::Integer(integer_from_float(
                        float_value(*bits, source),
                        target.bits(),
                        signed_target,
                        saturating,
                    )?)
                }
                Conversion::PointerToInteger => {
                    Datum::Integer(u128::from(pointer_of(lane)?) & mask(target.bits()))
                }
                Conversion::IntegerToPointer => {
                    Datum::Pointer((integer_of(lane)? & pointer_mask) as u64)
                }
                Conversion::Bitcast => lane.clone(),
            })
        })
    }
}

pub mod optimizer {
    use super::*;

    #[derive(Clone, PartialEq, Eq, Debug)]
    pub struct Rewrite {
        pub function: String,
        pub rule: &'static str,
        pub block: u32,
    }

    #[derive(Clone, Debug, Default)]
    pub struct Report {
        pub keep_log: bool,
        pub log: Vec<Rewrite>,
        counts: BTreeMap<&'static str, usize>,
    }

    impl Report {
        pub fn logging() -> Report {
            Report {
                keep_log: true,
                ..Report::default()
            }
        }

        fn note(&mut self, function: &str, rule: &'static str, block: u32) {
            *self.counts.entry(rule).or_insert(0) += 1;
            if self.keep_log {
                self.log.push(Rewrite {
                    function: function.to_string(),
                    rule,
                    block,
                });
            }
        }

        fn absorb(&mut self, other: Report) {
            for (rule, count) in other.counts {
                *self.counts.entry(rule).or_insert(0) += count;
            }
            self.log.extend(other.log);
        }

        pub fn count(&self, rule: &str) -> usize {
            self.counts
                .iter()
                .find(|(name, _)| **name == rule)
                .map_or(0, |(_, count)| *count)
        }

        pub fn total(&self) -> usize {
            self.counts.values().sum()
        }

        pub fn rules(&self) -> Vec<(&'static str, usize)> {
            self.counts
                .iter()
                .map(|(rule, count)| (*rule, *count))
                .collect()
        }
    }

    fn has_invoke(function: &Function) -> bool {
        function
            .blocks
            .iter()
            .any(|data| matches!(data.terminator, Terminator::Invoke { .. }))
    }

    fn terminator_values_mut(terminator: &mut Terminator, visit: &mut impl FnMut(&mut Value)) {
        match terminator {
            Terminator::Jump(call) => call.arguments.iter_mut().for_each(&mut *visit),
            Terminator::Branch {
                condition,
                taken,
                otherwise,
            } => {
                visit(condition);
                taken.arguments.iter_mut().for_each(&mut *visit);
                otherwise.arguments.iter_mut().for_each(&mut *visit);
            }
            Terminator::Switch {
                value,
                cases,
                default,
            } => {
                visit(value);
                for (_, call) in cases.iter_mut() {
                    call.arguments.iter_mut().for_each(&mut *visit);
                }
                default.arguments.iter_mut().for_each(&mut *visit);
            }
            Terminator::Return(value) => {
                if let Some(value) = value {
                    visit(value);
                }
            }
            Terminator::Unreachable => {}
            Terminator::Throw(value) => visit(value),
            Terminator::Invoke {
                call,
                normal,
                unwind,
            } => {
                call.arguments.iter_mut().for_each(&mut *visit);
                normal.arguments.iter_mut().for_each(&mut *visit);
                unwind.arguments.iter_mut().for_each(&mut *visit);
            }
        }
    }

    fn for_each_operand(function: &mut Function, mut visit: impl FnMut(&mut Value)) {
        let Function {
            instructions,
            blocks,
            ..
        } = function;
        for block in blocks.iter_mut() {
            for inst in &block.instructions {
                instructions[inst.index()]
                    .arguments
                    .iter_mut()
                    .for_each(&mut visit);
            }
            terminator_values_mut(&mut block.terminator, &mut visit);
        }
    }

    fn resolve(map: &[Option<Value>], mut value: Value) -> Value {
        let mut steps = 0;
        while let Some(Some(next)) = map.get(value.index()) {
            if *next == value || steps > map.len() {
                break;
            }
            value = *next;
            steps += 1;
        }
        value
    }

    fn apply_replacements(function: &mut Function, map: &[Option<Value>]) {
        if map.iter().all(Option::is_none) {
            return;
        }
        for_each_operand(function, |value| *value = resolve(map, *value));
    }

    fn reachable_blocks(function: &Function) -> Vec<bool> {
        let count = function.blocks.len();
        let mut seen = vec![false; count];
        if count == 0 {
            return seen;
        }
        let mut stack = vec![0usize];
        seen[0] = true;
        while let Some(block) = stack.pop() {
            for edge in function.blocks[block].terminator.successors() {
                let target = edge.block.index();
                if target < count && !seen[target] {
                    seen[target] = true;
                    stack.push(target);
                }
            }
        }
        seen
    }

    fn reverse_postorder(function: &Function) -> Vec<u32> {
        let count = function.blocks.len();
        let mut order = Vec::new();
        if count == 0 {
            return order;
        }
        let mut seen = vec![false; count];
        let mut stack: Vec<(usize, usize)> = vec![(0, 0)];
        seen[0] = true;
        while let Some((block, next)) = stack.last().copied() {
            let edges = function.blocks[block].terminator.successors();
            if next < edges.len() {
                if let Some(top) = stack.last_mut() {
                    top.1 += 1;
                }
                let target = edges[next].block.index();
                if target < count && !seen[target] {
                    seen[target] = true;
                    stack.push((target, 0));
                }
            } else {
                order.push(block as u32);
                stack.pop();
            }
        }
        order.reverse();
        order
    }

    fn predecessor_edges(function: &Function) -> Vec<Vec<u32>> {
        let mut found = vec![Vec::new(); function.blocks.len()];
        for (index, data) in function.blocks.iter().enumerate() {
            for edge in data.terminator.successors() {
                if edge.block.index() < found.len() {
                    found[edge.block.index()].push(index as u32);
                }
            }
        }
        found
    }

    fn safe_to_speculate(operation: &Operation) -> bool {
        match operation {
            Operation::Binary(op) => {
                !op.is_float()
                    && !matches!(
                        op,
                        BinaryOp::DivideSigned
                            | BinaryOp::DivideUnsigned
                            | BinaryOp::RemainderSigned
                            | BinaryOp::RemainderUnsigned
                    )
            }
            Operation::Unary(op) => !op.is_float(),
            Operation::Convert(conversion) => matches!(
                conversion,
                Conversion::ZeroExtend
                    | Conversion::SignExtend
                    | Conversion::Truncate
                    | Conversion::PointerToInteger
                    | Conversion::IntegerToPointer
            ),
            Operation::Compare(_)
            | Operation::Select
            | Operation::Offset
            | Operation::FieldAddress(..)
            | Operation::ElementAddress(_) => true,
            _ => false,
        }
    }

    fn hoist_invariants(function: &mut Function, report: &mut Report) -> bool {
        let count = function.blocks.len();
        if has_invoke(function) || count < 3 {
            return false;
        }
        let rpo: Vec<usize> = reverse_postorder(function)
            .into_iter()
            .map(|block| block as usize)
            .collect();
        let mut order = vec![usize::MAX; count];
        for (at, block) in rpo.iter().enumerate() {
            order[*block] = at;
        }
        let preds = predecessor_edges(function);
        let mut idom = vec![usize::MAX; count];
        idom[0] = 0;
        let mut moved = true;
        while moved {
            moved = false;
            for &block in rpo.iter().skip(1) {
                let mut chosen = usize::MAX;
                for &pred in &preds[block] {
                    let pred = pred as usize;
                    if idom[pred] == usize::MAX {
                        continue;
                    }
                    if chosen == usize::MAX {
                        chosen = pred;
                        continue;
                    }
                    let (mut a, mut b) = (pred, chosen);
                    while a != b {
                        while order[a] > order[b] {
                            a = idom[a];
                        }
                        while order[b] > order[a] {
                            b = idom[b];
                        }
                    }
                    chosen = a;
                }
                if chosen != usize::MAX && idom[block] != chosen {
                    idom[block] = chosen;
                    moved = true;
                }
            }
        }
        let dominates = |above: usize, below: usize| {
            let mut at = below;
            loop {
                if at == above {
                    return true;
                }
                if at == 0 || idom[at] == usize::MAX {
                    return false;
                }
                at = idom[at];
            }
        };
        let mut tails: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for (block, position) in order.iter().enumerate() {
            if *position == usize::MAX {
                continue;
            }
            for edge in function.blocks[block].terminator.successors() {
                let target = edge.block.index();
                if target < count && dominates(target, block) {
                    tails.entry(target).or_default().push(block);
                }
            }
        }
        let mut loops: Vec<(usize, Vec<bool>, usize)> = Vec::new();
        for (header, tail_list) in &tails {
            if *header == 0 {
                continue;
            }
            let mut body = vec![false; count];
            body[*header] = true;
            let mut stack: Vec<usize> = Vec::new();
            for tail in tail_list {
                if !body[*tail] {
                    body[*tail] = true;
                    stack.push(*tail);
                }
            }
            while let Some(block) = stack.pop() {
                for &pred in &preds[block] {
                    let pred = pred as usize;
                    if !body[pred] {
                        body[pred] = true;
                        stack.push(pred);
                    }
                }
            }
            let size = body.iter().filter(|held| **held).count();
            loops.push((*header, body, size));
        }
        loops.sort_by_key(|(header, _, size)| (*size, *header));
        let mut block_of = vec![usize::MAX; function.value_types.len()];
        for (block, data) in function.blocks.iter().enumerate() {
            for parameter in &data.parameters {
                block_of[parameter.index()] = block;
            }
            for inst in &data.instructions {
                if let Some(result) = function.instructions[inst.index()].result {
                    block_of[result.index()] = block;
                }
            }
        }
        let mut changed = false;
        for (header, body, _) in loops {
            let target = idom[header];
            if target == usize::MAX || body[target] {
                continue;
            }
            for &block in &rpo {
                if !body[block] {
                    continue;
                }
                let mut position = 0;
                while position < function.blocks[block].instructions.len() {
                    let inst = function.blocks[block].instructions[position];
                    let instruction = &function.instructions[inst.index()];
                    let movable = safe_to_speculate(&instruction.operation)
                        && instruction.result.is_some()
                        && instruction.arguments.iter().all(|argument| {
                            let home = block_of[argument.index()];
                            home != usize::MAX && !body[home]
                        });
                    if !movable {
                        position += 1;
                        continue;
                    }
                    let result = instruction.result;
                    function.blocks[block].instructions.remove(position);
                    function.blocks[target].instructions.push(inst);
                    if let Some(result) = result {
                        block_of[result.index()] = target;
                    }
                    report.note(&function.name, "hoisted", block as u32);
                    changed = true;
                }
            }
        }
        changed
    }

    fn remove_unreachable(function: &mut Function, report: &mut Report) -> bool {
        let seen = reachable_blocks(function);
        if seen.iter().all(|reached| *reached) {
            return false;
        }
        let mut renumber = vec![u32::MAX; seen.len()];
        let mut next = 0u32;
        for (index, reached) in seen.iter().enumerate() {
            if *reached {
                renumber[index] = next;
                next += 1;
            } else {
                report.note(&function.name, "unreachable-block", index as u32);
            }
        }
        let old = std::mem::take(&mut function.blocks);
        for (index, mut data) in old.into_iter().enumerate() {
            if !seen[index] {
                continue;
            }
            for edge in data.terminator.successors_mut() {
                edge.block = Block(renumber[edge.block.index()]);
            }
            function.blocks.push(data);
        }
        true
    }

    fn compact_instructions(function: &mut Function) {
        let mut fresh: Vec<Instruction> = Vec::with_capacity(function.instructions.len());
        for block in &mut function.blocks {
            for inst in block.instructions.iter_mut() {
                fresh.push(function.instructions[inst.index()].clone());
                *inst = Inst(fresh.len() as u32 - 1);
            }
        }
        function.instructions = fresh;
    }

    fn compact_values(function: &mut Function) {
        let mut live = vec![false; function.value_types.len()];
        for data in &function.blocks {
            for parameter in &data.parameters {
                live[parameter.index()] = true;
            }
            for inst in &data.instructions {
                if let Some(result) = function.instructions[inst.index()].result {
                    live[result.index()] = true;
                }
            }
            if let Terminator::Invoke { call, .. } = &data.terminator {
                if let Some(result) = call.result {
                    live[result.index()] = true;
                }
            }
        }
        let mut renumber = vec![u32::MAX; live.len()];
        let mut types: Vec<Type> = Vec::new();
        for (index, kept) in live.iter().enumerate() {
            if *kept {
                renumber[index] = types.len() as u32;
                types.push(function.value_types[index]);
            }
        }
        if types.len() == live.len() {
            return;
        }
        let map = |value: &mut Value| {
            if let Some(next) = renumber.get(value.index()) {
                if *next != u32::MAX {
                    *value = Value(*next);
                }
            }
        };
        let mut map = map;
        for data in function.blocks.iter_mut() {
            data.parameters.iter_mut().for_each(&mut map);
            for inst in &data.instructions {
                let instruction = &mut function.instructions[inst.index()];
                instruction.arguments.iter_mut().for_each(&mut map);
                if let Some(result) = instruction.result.as_mut() {
                    map(result);
                }
            }
            terminator_values_mut(&mut data.terminator, &mut map);
            if let Terminator::Invoke { call, .. } = &mut data.terminator {
                if let Some(result) = call.result.as_mut() {
                    map(result);
                }
            }
        }
        function.value_types = types;
    }

    fn integer_type(ty: Type) -> Option<u32> {
        match ty {
            Type::Scalar(scalar) if scalar.is_integer() => Some(scalar.bits()),
            _ => None,
        }
    }

    fn canonical_constant(value: u128, bits: u32) -> i128 {
        if bits == 1 {
            (value & 1) as i128
        } else {
            signed(value & mask(bits), bits)
        }
    }

    fn insert_constant(
        function: &mut Function,
        out: &mut Vec<Inst>,
        ty: Type,
        value: u128,
    ) -> Value {
        let bits = integer_type(ty).unwrap_or(64);
        let result = function.new_value(ty);
        function.instructions.push(Instruction {
            operation: Operation::IntegerConstant(canonical_constant(value, bits)),
            arguments: Vec::new(),
            result: Some(result),
            spot: None,
        });
        out.push(Inst(function.instructions.len() as u32 - 1));
        result
    }

    fn insert_binary(
        function: &mut Function,
        out: &mut Vec<Inst>,
        ty: Type,
        op: BinaryOp,
        left: Value,
        right: Value,
    ) -> Value {
        let result = function.new_value(ty);
        function.instructions.push(Instruction {
            operation: Operation::Binary(op),
            arguments: vec![left, right],
            result: Some(result),
            spot: None,
        });
        out.push(Inst(function.instructions.len() as u32 - 1));
        result
    }

    fn insert_convert(
        function: &mut Function,
        out: &mut Vec<Inst>,
        ty: Type,
        conversion: Conversion,
        value: Value,
    ) -> Value {
        let result = function.new_value(ty);
        function.instructions.push(Instruction {
            operation: Operation::Convert(conversion),
            arguments: vec![value],
            result: Some(result),
            spot: None,
        });
        out.push(Inst(function.instructions.len() as u32 - 1));
        result
    }

    fn unsigned_magic(divisor: u128, bits: u32) -> (u128, u32, bool) {
        let all = mask(bits);
        let high = 1u128 << (bits - 1);
        let mut add = false;
        let negated = divisor.wrapping_neg() & all;
        let nc = all.wrapping_sub(negated % divisor) & all;
        let mut p = bits - 1;
        let mut q1 = high / nc;
        let mut r1 = high - q1 * nc;
        let mut q2 = (high - 1) / divisor;
        let mut r2 = (high - 1) - q2 * divisor;
        loop {
            p += 1;
            if r1 >= nc - r1 {
                q1 = (2 * q1 + 1) & all;
                r1 = (2 * r1).wrapping_sub(nc) & all;
            } else {
                q1 = (2 * q1) & all;
                r1 = (2 * r1) & all;
            }
            if r2 + 1 >= divisor - r2 {
                if q2 >= high - 1 {
                    add = true;
                }
                q2 = (2 * q2 + 1) & all;
                r2 = (2 * r2 + 1).wrapping_sub(divisor) & all;
            } else {
                if q2 >= high {
                    add = true;
                }
                q2 = (2 * q2) & all;
                r2 = (2 * r2 + 1) & all;
            }
            let delta = divisor - 1 - r2;
            if !(p < 2 * bits && (q1 < delta || (q1 == delta && r1 == 0))) {
                break;
            }
        }
        ((q2 + 1) & all, p - bits, add)
    }

    fn signed_magic(divisor: u128, bits: u32) -> (u128, u32) {
        let all = mask(bits);
        let high = 1u128 << (bits - 1);
        let t = high;
        let anc = (t - 1 - t % divisor) & all;
        let mut p = bits - 1;
        let mut q1 = high / anc;
        let mut r1 = high - q1 * anc;
        let mut q2 = high / divisor;
        let mut r2 = high - q2 * divisor;
        loop {
            p += 1;
            q1 = (2 * q1) & all;
            r1 = (2 * r1) & all;
            if r1 >= anc {
                q1 = (q1 + 1) & all;
                r1 = r1.wrapping_sub(anc) & all;
            }
            q2 = (2 * q2) & all;
            r2 = (2 * r2) & all;
            if r2 >= divisor {
                q2 = (q2 + 1) & all;
                r2 = r2.wrapping_sub(divisor) & all;
            }
            let delta = divisor - r2;
            if !(q1 < delta || (q1 == delta && r1 == 0)) {
                break;
            }
        }
        ((q2 + 1) & all, p - bits)
    }

    fn divide_by_constant(
        function: &mut Function,
        out: &mut Vec<Inst>,
        ty: Type,
        bits: u32,
        signed: bool,
        value: Value,
        divisor: u128,
    ) -> Value {
        let wide = Type::I64;
        let high_part = |function: &mut Function, out: &mut Vec<Inst>, magic: u128| -> Value {
            if bits == 64 {
                let constant = insert_constant(function, out, ty, magic);
                let op = if signed {
                    BinaryOp::MultiplyHighSigned
                } else {
                    BinaryOp::MultiplyHighUnsigned
                };
                return insert_binary(function, out, ty, op, value, constant);
            }
            let extension = if signed {
                Conversion::SignExtend
            } else {
                Conversion::ZeroExtend
            };
            let widened = insert_convert(function, out, wide, extension, value);
            let extended = if signed {
                u128::from(magic as u32 as i32 as i64 as u64)
            } else {
                magic
            };
            let constant = insert_constant(function, out, wide, extended);
            let product = insert_binary(function, out, wide, BinaryOp::Multiply, widened, constant);
            let shift = insert_constant(function, out, wide, 32);
            let shifted_op = if signed {
                BinaryOp::ShiftRightArithmetic
            } else {
                BinaryOp::ShiftRightLogical
            };
            let shifted = insert_binary(function, out, wide, shifted_op, product, shift);
            insert_convert(function, out, ty, Conversion::Truncate, shifted)
        };
        if !signed {
            let (magic, shift, add) = unsigned_magic(divisor, bits);
            let high = high_part(function, out, magic);
            if !add {
                let amount = insert_constant(function, out, ty, u128::from(shift));
                return insert_binary(function, out, ty, BinaryOp::ShiftRightLogical, high, amount);
            }
            let difference = insert_binary(function, out, ty, BinaryOp::Subtract, value, high);
            let one = insert_constant(function, out, ty, 1);
            let halved = insert_binary(
                function,
                out,
                ty,
                BinaryOp::ShiftRightLogical,
                difference,
                one,
            );
            let sum = insert_binary(function, out, ty, BinaryOp::Add, halved, high);
            let amount = insert_constant(function, out, ty, u128::from(shift - 1));
            return insert_binary(function, out, ty, BinaryOp::ShiftRightLogical, sum, amount);
        }
        let (magic, shift) = signed_magic(divisor, bits);
        let mut quotient = high_part(function, out, magic);
        if magic >> (bits - 1) != 0 {
            quotient = insert_binary(function, out, ty, BinaryOp::Add, quotient, value);
        }
        if shift > 0 {
            let amount = insert_constant(function, out, ty, u128::from(shift));
            quotient = insert_binary(
                function,
                out,
                ty,
                BinaryOp::ShiftRightArithmetic,
                quotient,
                amount,
            );
        }
        let top = insert_constant(function, out, ty, u128::from(bits - 1));
        let sign = insert_binary(
            function,
            out,
            ty,
            BinaryOp::ShiftRightLogical,
            quotient,
            top,
        );
        insert_binary(function, out, ty, BinaryOp::Add, quotient, sign)
    }

    struct Access {
        block: u32,
        inst: Inst,
        store: bool,
        value: Value,
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Def {
        Nothing,
        Value(Value),
        Phi(u32),
    }

    fn promote_slots(function: &mut Function, report: &mut Report) -> bool {
        if has_invoke(function) || function.blocks.is_empty() {
            return false;
        }
        let preds = predecessor_edges(function);
        if !preds[0].is_empty() {
            return false;
        }
        let mut slots: Vec<Value> = Vec::new();
        let mut slot_index: BTreeMap<Value, usize> = BTreeMap::new();
        let mut slot_inst: Vec<(u32, Inst)> = Vec::new();
        for (block, data) in function.blocks.iter().enumerate() {
            for inst in &data.instructions {
                let instruction = &function.instructions[inst.index()];
                if let (Operation::StackSlot { .. }, Some(result)) =
                    (&instruction.operation, instruction.result)
                {
                    slot_index.insert(result, slots.len());
                    slots.push(result);
                    slot_inst.push((block as u32, *inst));
                }
            }
        }
        if slots.is_empty() {
            return false;
        }
        let mut sound = vec![true; slots.len()];
        let mut types: Vec<Option<Type>> = vec![None; slots.len()];
        let mut accesses: Vec<Vec<Access>> = (0..slots.len()).map(|_| Vec::new()).collect();
        for (block, data) in function.blocks.iter().enumerate() {
            for inst in &data.instructions {
                let instruction = &function.instructions[inst.index()];
                for (position, argument) in instruction.arguments.iter().enumerate() {
                    let Some(&variable) = slot_index.get(argument) else {
                        continue;
                    };
                    let plain_load = matches!(
                        instruction.operation,
                        Operation::Load {
                            volatile: false,
                            ordering: None,
                            ..
                        }
                    );
                    let plain_store = matches!(
                        instruction.operation,
                        Operation::Store {
                            volatile: false,
                            ordering: None,
                            ..
                        }
                    );
                    if plain_load && position == 0 {
                        let ty = instruction.result.map(|result| function.type_of(result));
                        match (types[variable], ty) {
                            (_, None) => sound[variable] = false,
                            (None, Some(ty)) => types[variable] = Some(ty),
                            (Some(known), Some(ty)) if known != ty => sound[variable] = false,
                            _ => {}
                        }
                        accesses[variable].push(Access {
                            block: block as u32,
                            inst: *inst,
                            store: false,
                            value: instruction.result.unwrap_or(Value(0)),
                        });
                    } else if plain_store && position == 1 && instruction.arguments[0] != *argument
                    {
                        let stored = instruction.arguments[0];
                        let ty = function.type_of(stored);
                        match types[variable] {
                            None => types[variable] = Some(ty),
                            Some(known) if known != ty => sound[variable] = false,
                            _ => {}
                        }
                        accesses[variable].push(Access {
                            block: block as u32,
                            inst: *inst,
                            store: true,
                            value: stored,
                        });
                    } else {
                        sound[variable] = false;
                    }
                }
            }
        }
        {
            let mut escaped = |value: &mut Value| {
                if let Some(&variable) = slot_index.get(value) {
                    sound[variable] = false;
                }
            };
            let mut scratch: Vec<Terminator> = function
                .blocks
                .iter()
                .map(|data| data.terminator.clone())
                .collect();
            for terminator in scratch.iter_mut() {
                terminator_values_mut(terminator, &mut escaped);
            }
        }
        let order = reverse_postorder(function);
        let mut removed: BTreeSet<Inst> = BTreeSet::new();
        let mut replacements: Vec<Option<Value>> = vec![None; function.value_types.len()];
        let mut added_parameters: Vec<(u32, Value)> = Vec::new();
        let mut edge_arguments: Vec<(u32, u32, Value)> = Vec::new();
        let mut changed = false;
        for variable in 0..slots.len() {
            if !sound[variable] {
                continue;
            }
            let Some(ty) = types[variable] else {
                continue;
            };
            let count = function.blocks.len();
            let mut last_store: Vec<Option<Value>> = vec![None; count];
            let mut reads_before: Vec<bool> = vec![false; count];
            for access in &accesses[variable] {
                let block = access.block as usize;
                if access.store {
                    last_store[block] = Some(access.value);
                } else if last_store[block].is_none() {
                    reads_before[block] = true;
                }
            }
            let mut incoming = vec![Def::Nothing; count];
            let mut outgoing = vec![Def::Nothing; count];
            for block in &order {
                let index = *block as usize;
                incoming[index] = if index == 0 {
                    Def::Nothing
                } else if preds[index].len() == 1 {
                    outgoing[preds[index][0] as usize]
                } else {
                    Def::Phi(*block)
                };
                outgoing[index] = match last_store[index] {
                    Some(value) => Def::Value(value),
                    None => incoming[index],
                };
            }
            let mut needed: BTreeSet<u32> = BTreeSet::new();
            let mut pending: Vec<u32> = Vec::new();
            let mut failed = false;
            for block in &order {
                if reads_before[*block as usize] {
                    match incoming[*block as usize] {
                        Def::Nothing => failed = true,
                        Def::Phi(owner) => {
                            if needed.insert(owner) {
                                pending.push(owner);
                            }
                        }
                        Def::Value(_) => {}
                    }
                }
            }
            while let Some(owner) = pending.pop() {
                for predecessor in &preds[owner as usize] {
                    match outgoing[*predecessor as usize] {
                        Def::Nothing => failed = true,
                        Def::Phi(next) => {
                            if needed.insert(next) {
                                pending.push(next);
                            }
                        }
                        Def::Value(_) => {}
                    }
                }
            }
            if failed {
                continue;
            }
            let mut phi_values: BTreeMap<u32, Value> = BTreeMap::new();
            for owner in &needed {
                let value = function.new_value(ty);
                replacements.push(None);
                phi_values.insert(*owner, value);
                added_parameters.push((*owner, value));
            }
            let concrete = |def: Def| -> Option<Value> {
                match def {
                    Def::Nothing => None,
                    Def::Value(value) => Some(value),
                    Def::Phi(owner) => phi_values.get(&owner).copied(),
                }
            };
            for owner in &needed {
                let unique: BTreeSet<u32> = preds[*owner as usize].iter().copied().collect();
                for predecessor in unique {
                    if let Some(value) = concrete(outgoing[predecessor as usize]) {
                        edge_arguments.push((predecessor, *owner, value));
                    }
                }
            }
            let mut current: Option<Value> = None;
            let mut at_block = u32::MAX;
            for access in &accesses[variable] {
                if access.block != at_block {
                    at_block = access.block;
                    current = concrete(incoming[at_block as usize]);
                }
                if access.store {
                    current = Some(access.value);
                } else if let Some(value) = current {
                    replacements[access.value.index()] = Some(value);
                }
                removed.insert(access.inst);
            }
            removed.insert(slot_inst[variable].1);
            report.note(&function.name, "slot-promoted", slot_inst[variable].0);
            changed = true;
        }
        if !changed {
            return false;
        }
        for (owner, value) in &added_parameters {
            function.blocks[*owner as usize].parameters.push(*value);
        }
        let mut grouped: BTreeMap<(u32, u32), Vec<Value>> = BTreeMap::new();
        for (from, to, value) in edge_arguments {
            grouped.entry((from, to)).or_default().push(value);
        }
        for ((from, to), values) in grouped {
            for edge in function.blocks[from as usize].terminator.successors_mut() {
                if edge.block.0 == to {
                    edge.arguments.extend(values.iter().copied());
                }
            }
        }
        for block in function.blocks.iter_mut() {
            block.instructions.retain(|inst| !removed.contains(inst));
        }
        apply_replacements(function, &replacements);
        true
    }

    fn prune_parameters(function: &mut Function, report: &mut Report) -> bool {
        if has_invoke(function) || function.blocks.len() < 2 {
            return false;
        }
        let mut changed = false;
        loop {
            let count = function.blocks.len();
            let mut incoming: Vec<Vec<Vec<Value>>> = vec![Vec::new(); count];
            for data in &function.blocks {
                for edge in data.terminator.successors() {
                    incoming[edge.block.index()].push(edge.arguments.clone());
                }
            }
            let mut replacements: Vec<Option<Value>> = vec![None; function.value_types.len()];
            let mut trivial = false;
            for (block, arrivals) in incoming.iter().enumerate().take(count).skip(1) {
                let parameters = function.blocks[block].parameters.clone();
                for (slot, parameter) in parameters.iter().enumerate() {
                    let mut same: Option<Value> = None;
                    let mut ok = !arrivals.is_empty();
                    for arguments in arrivals {
                        let argument = resolve(&replacements, arguments[slot]);
                        if argument == *parameter {
                            continue;
                        }
                        match same {
                            None => same = Some(argument),
                            Some(known) if known == argument => {}
                            Some(_) => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    if ok {
                        if let Some(value) = same {
                            replacements[parameter.index()] = Some(value);
                            trivial = true;
                            report.note(&function.name, "trivial-parameter", block as u32);
                        }
                    }
                }
            }
            if trivial {
                apply_replacements(function, &replacements);
                let dropped: Vec<Vec<bool>> = (0..count)
                    .map(|block| {
                        function.blocks[block]
                            .parameters
                            .iter()
                            .map(|parameter| replacements[parameter.index()].is_some())
                            .collect()
                    })
                    .collect();
                drop_parameters(function, &dropped);
                changed = true;
                continue;
            }
            let mut uses = vec![0usize; function.value_types.len()];
            let mut live_parameter = vec![false; function.value_types.len()];
            let mut owner: BTreeMap<Value, (usize, usize)> = BTreeMap::new();
            for (block, data) in function.blocks.iter().enumerate() {
                for (slot, parameter) in data.parameters.iter().enumerate() {
                    owner.insert(*parameter, (block, slot));
                }
            }
            let mut roots: Vec<Value> = Vec::new();
            {
                let mut probe = |value: &mut Value| {
                    uses[value.index()] += 1;
                    roots.push(*value);
                };
                let Function {
                    instructions,
                    blocks,
                    ..
                } = &mut *function;
                for data in blocks.iter_mut() {
                    for inst in &data.instructions {
                        instructions[inst.index()]
                            .arguments
                            .iter_mut()
                            .for_each(&mut probe);
                    }
                    match &mut data.terminator {
                        Terminator::Branch { condition, .. } => probe(condition),
                        Terminator::Switch { value, .. } => probe(value),
                        Terminator::Return(Some(value)) | Terminator::Throw(value) => probe(value),
                        _ => {}
                    }
                }
            }
            let mut pending: Vec<Value> = roots;
            while let Some(value) = pending.pop() {
                let Some(&(block, slot)) = owner.get(&value) else {
                    continue;
                };
                if live_parameter[value.index()] {
                    continue;
                }
                live_parameter[value.index()] = true;
                for data in &function.blocks {
                    for edge in data.terminator.successors() {
                        if edge.block.index() == block {
                            pending.push(edge.arguments[slot]);
                        }
                    }
                }
            }
            let mut dropped: Vec<Vec<bool>> = Vec::with_capacity(count);
            let mut any = false;
            for (block, data) in function.blocks.iter().enumerate() {
                let mut row = Vec::new();
                for parameter in &data.parameters {
                    let dead = block != 0 && !live_parameter[parameter.index()];
                    if dead {
                        any = true;
                        report.note(&function.name, "dead-parameter", block as u32);
                    }
                    row.push(dead);
                }
                dropped.push(row);
            }
            if !any {
                return changed;
            }
            drop_parameters(function, &dropped);
            changed = true;
        }
    }

    fn drop_parameters(function: &mut Function, dropped: &[Vec<bool>]) {
        for (block, data) in function.blocks.iter_mut().enumerate() {
            let mut slot = 0;
            data.parameters.retain(|_| {
                let keep = !dropped[block][slot];
                slot += 1;
                keep
            });
        }
        for data in function.blocks.iter_mut() {
            for edge in data.terminator.successors_mut() {
                let row = &dropped[edge.block.index()];
                let mut slot = 0;
                edge.arguments.retain(|_| {
                    let keep = !row.get(slot).copied().unwrap_or(false);
                    slot += 1;
                    keep
                });
            }
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq, Debug)]
    enum Lattice {
        Top,
        Constant(u128),
        Bottom,
    }

    fn meet(left: Lattice, right: Lattice) -> Lattice {
        match (left, right) {
            (Lattice::Top, other) | (other, Lattice::Top) => other,
            (Lattice::Constant(a), Lattice::Constant(b)) if a == b => left,
            _ => Lattice::Bottom,
        }
    }

    fn compare_integers(comparison: Comparison, left: u128, right: u128, bits: u32) -> bool {
        let (left_signed, right_signed) = (signed(left, bits), signed(right, bits));
        match comparison {
            Comparison::Equal => left == right,
            Comparison::NotEqual => left != right,
            Comparison::LessSigned => left_signed < right_signed,
            Comparison::LessEqualSigned => left_signed <= right_signed,
            Comparison::GreaterSigned => left_signed > right_signed,
            Comparison::GreaterEqualSigned => left_signed >= right_signed,
            Comparison::LessUnsigned => left < right,
            Comparison::LessEqualUnsigned => left <= right,
            Comparison::GreaterUnsigned => left > right,
            Comparison::GreaterEqualUnsigned => left >= right,
        }
    }

    fn foldable(operation: &Operation) -> bool {
        matches!(
            operation,
            Operation::IntegerConstant(_)
                | Operation::Binary(_)
                | Operation::Unary(_)
                | Operation::Compare(_)
                | Operation::Convert(_)
                | Operation::Select
        )
    }

    fn evaluate(function: &Function, instruction: &Instruction, state: &[Lattice]) -> Lattice {
        let Some(result) = instruction.result else {
            return Lattice::Bottom;
        };
        let ty = function.type_of(result);
        let Some(bits) = integer_type(ty) else {
            return Lattice::Bottom;
        };
        let operand = |index: usize| state[instruction.arguments[index].index()];
        let operand_type = |index: usize| function.type_of(instruction.arguments[index]);
        match &instruction.operation {
            Operation::IntegerConstant(value) => Lattice::Constant((*value as u128) & mask(bits)),
            Operation::Binary(op) if !op.is_float() && instruction.arguments.len() == 2 => {
                let (left, right) = (operand(0), operand(1));
                match (left, right) {
                    (Lattice::Top, _) | (_, Lattice::Top) => Lattice::Top,
                    (Lattice::Constant(a), Lattice::Constant(b)) => {
                        let width = integer_type(operand_type(0)).unwrap_or(bits);
                        match integer_binary(*op, a, b, width) {
                            Ok(value) => Lattice::Constant(value & mask(bits)),
                            Err(_) => Lattice::Bottom,
                        }
                    }
                    _ => Lattice::Bottom,
                }
            }
            Operation::Unary(op) if !op.is_float() && instruction.arguments.len() == 1 => {
                match operand(0) {
                    Lattice::Top => Lattice::Top,
                    Lattice::Constant(a) => {
                        let width = integer_type(operand_type(0)).unwrap_or(bits);
                        Lattice::Constant(integer_unary(*op, a, width) & mask(bits))
                    }
                    Lattice::Bottom => Lattice::Bottom,
                }
            }
            Operation::Compare(comparison) if instruction.arguments.len() == 2 => {
                let Some(width) = integer_type(operand_type(0)) else {
                    return Lattice::Bottom;
                };
                match (operand(0), operand(1)) {
                    (Lattice::Top, _) | (_, Lattice::Top) => Lattice::Top,
                    (Lattice::Constant(a), Lattice::Constant(b)) => {
                        Lattice::Constant(u128::from(compare_integers(*comparison, a, b, width)))
                    }
                    _ => Lattice::Bottom,
                }
            }
            Operation::Convert(conversion) if instruction.arguments.len() == 1 => {
                let Some(width) = integer_type(operand_type(0)) else {
                    return Lattice::Bottom;
                };
                match (operand(0), conversion) {
                    (Lattice::Top, _) => Lattice::Top,
                    (Lattice::Constant(a), Conversion::ZeroExtend | Conversion::Truncate) => {
                        Lattice::Constant(a & mask(bits))
                    }
                    (Lattice::Constant(a), Conversion::SignExtend) => {
                        Lattice::Constant((signed(a, width) as u128) & mask(bits))
                    }
                    _ => Lattice::Bottom,
                }
            }
            Operation::Select if instruction.arguments.len() == 3 => match operand(0) {
                Lattice::Top => Lattice::Top,
                Lattice::Constant(condition) => {
                    if condition != 0 {
                        operand(1)
                    } else {
                        operand(2)
                    }
                }
                Lattice::Bottom => match (operand(1), operand(2)) {
                    (Lattice::Constant(a), Lattice::Constant(b)) if a == b => Lattice::Constant(a),
                    (Lattice::Top, other) | (other, Lattice::Top) => {
                        if other == Lattice::Top {
                            Lattice::Top
                        } else {
                            other
                        }
                    }
                    _ => Lattice::Bottom,
                },
            },
            _ => Lattice::Bottom,
        }
    }

    fn propagate_constants(function: &mut Function, report: &mut Report) -> bool {
        if has_invoke(function) || function.blocks.is_empty() {
            return false;
        }
        let count = function.blocks.len();
        let mut state = vec![Lattice::Top; function.value_types.len()];
        let mut running = vec![false; count];
        let mut flows: Vec<Vec<bool>> = function
            .blocks
            .iter()
            .map(|data| vec![false; data.terminator.successors().len()])
            .collect();
        running[0] = true;
        for parameter in &function.blocks[0].parameters {
            state[parameter.index()] = Lattice::Bottom;
        }
        let order = reverse_postorder(function);
        let mut rounds = 0;
        loop {
            rounds += 1;
            if rounds > 64 {
                return false;
            }
            let mut moved = false;
            for block in &order {
                let index = *block as usize;
                if !running[index] {
                    continue;
                }
                for inst in &function.blocks[index].instructions {
                    let instruction = &function.instructions[inst.index()];
                    let Some(result) = instruction.result else {
                        continue;
                    };
                    let value = if foldable(&instruction.operation) {
                        evaluate(function, instruction, &state)
                    } else {
                        Lattice::Bottom
                    };
                    let merged = if state[result.index()] == Lattice::Top {
                        value
                    } else {
                        meet(state[result.index()], value)
                    };
                    if merged != state[result.index()] {
                        state[result.index()] = merged;
                        moved = true;
                    }
                }
                let terminator = &function.blocks[index].terminator;
                let edges = terminator.successors();
                let chosen: Vec<bool> = match terminator {
                    Terminator::Branch { condition, .. } => match state[condition.index()] {
                        Lattice::Top => vec![false, false],
                        Lattice::Constant(value) => vec![value != 0, value == 0],
                        Lattice::Bottom => vec![true, true],
                    },
                    _ => vec![true; edges.len()],
                };
                for (slot, edge) in edges.iter().enumerate() {
                    if !chosen[slot] {
                        continue;
                    }
                    if !flows[index][slot] {
                        flows[index][slot] = true;
                        moved = true;
                    }
                    let target = edge.block.index();
                    if !running[target] {
                        running[target] = true;
                        moved = true;
                    }
                    for (position, argument) in edge.arguments.iter().enumerate() {
                        let parameter = function.blocks[target].parameters[position];
                        let incoming = state[argument.index()];
                        let merged = meet(state[parameter.index()], incoming);
                        if merged != state[parameter.index()] {
                            state[parameter.index()] = merged;
                            moved = true;
                        }
                    }
                }
            }
            if !moved {
                break;
            }
        }
        let mut changed = false;
        let mut replacements: Vec<Option<Value>> = vec![None; function.value_types.len()];
        for (block, is_running) in running.iter().enumerate().take(count) {
            if !*is_running {
                continue;
            }
            let mut out: Vec<Inst> = Vec::new();
            for parameter in function.blocks[block].parameters.clone() {
                if block == 0 {
                    break;
                }
                if let Lattice::Constant(value) = state[parameter.index()] {
                    let ty = function.type_of(parameter);
                    let fresh = insert_constant(function, &mut out, ty, value);
                    replacements.push(None);
                    replacements[parameter.index()] = Some(fresh);
                    report.note(&function.name, "constant-parameter", block as u32);
                    changed = true;
                }
            }
            let old = function.blocks[block].instructions.clone();
            for inst in old {
                let instruction = &mut function.instructions[inst.index()];
                if let Some(result) = instruction.result {
                    if foldable(&instruction.operation) {
                        if let Lattice::Constant(value) = state[result.index()] {
                            let ty = function.value_types[result.index()];
                            let bits = integer_type(ty).unwrap_or(64);
                            let already =
                                matches!(instruction.operation, Operation::IntegerConstant(_));
                            if !already {
                                instruction.operation =
                                    Operation::IntegerConstant(canonical_constant(value, bits));
                                instruction.arguments.clear();
                                report.note(&function.name, "folded", block as u32);
                                changed = true;
                            }
                        }
                    }
                }
                out.push(inst);
            }
            function.blocks[block].instructions = out;
            let replacement = match &function.blocks[block].terminator {
                Terminator::Branch {
                    condition,
                    taken,
                    otherwise,
                } => match state[condition.index()] {
                    Lattice::Constant(value) => Some(Terminator::Jump(if value != 0 {
                        taken.clone()
                    } else {
                        otherwise.clone()
                    })),
                    _ => None,
                },
                _ => None,
            };
            if let Some(terminator) = replacement {
                function.blocks[block].terminator = terminator;
                report.note(&function.name, "branch-folded", block as u32);
                changed = true;
            }
        }
        if changed {
            apply_replacements(function, &replacements);
        }
        changed
    }

    fn pure_instruction(operation: &Operation) -> bool {
        match operation {
            Operation::IntegerConstant(_)
            | Operation::FloatConstant(_)
            | Operation::Null
            | Operation::GlobalAddress(_)
            | Operation::FunctionAddress(_)
            | Operation::Unary(_)
            | Operation::Compare(_)
            | Operation::FloatCompare(_)
            | Operation::Select
            | Operation::Splat
            | Operation::ExtractLane(_)
            | Operation::InsertLane(_)
            | Operation::Offset
            | Operation::FieldAddress(..)
            | Operation::ElementAddress(_)
            | Operation::SameObject => true,
            Operation::Binary(op) => !matches!(
                op,
                BinaryOp::DivideSigned
                    | BinaryOp::DivideUnsigned
                    | BinaryOp::RemainderSigned
                    | BinaryOp::RemainderUnsigned
            ),
            Operation::Convert(conversion) => !matches!(
                conversion,
                Conversion::FloatToSigned | Conversion::FloatToUnsigned
            ),
            _ => false,
        }
    }

    fn removable(operation: &Operation) -> bool {
        pure_instruction(operation) || matches!(operation, Operation::StackSlot { .. })
    }

    fn eliminate_dead(function: &mut Function, report: &mut Report) -> bool {
        let mut uses = vec![0usize; function.value_types.len()];
        {
            let mut count = |value: &mut Value| uses[value.index()] += 1;
            let Function {
                instructions,
                blocks,
                ..
            } = &mut *function;
            for data in blocks.iter_mut() {
                for inst in &data.instructions {
                    instructions[inst.index()]
                        .arguments
                        .iter_mut()
                        .for_each(&mut count);
                }
                terminator_values_mut(&mut data.terminator, &mut count);
            }
        }
        let mut changed = false;
        for block in (0..function.blocks.len()).rev() {
            let old = function.blocks[block].instructions.clone();
            let mut keep: Vec<Inst> = Vec::with_capacity(old.len());
            for inst in old.into_iter().rev() {
                let instruction = &function.instructions[inst.index()];
                let dead = match instruction.result {
                    Some(result) => uses[result.index()] == 0 && removable(&instruction.operation),
                    None => false,
                };
                if dead {
                    for argument in instruction.arguments.clone() {
                        uses[argument.index()] -= 1;
                    }
                    report.note(&function.name, "dead-instruction", block as u32);
                    changed = true;
                } else {
                    keep.push(inst);
                }
            }
            keep.reverse();
            function.blocks[block].instructions = keep;
        }
        changed
    }

    fn simplify_patterns(function: &mut Function, report: &mut Report) -> bool {
        let mut definition: Vec<Option<usize>> = vec![None; function.value_types.len()];
        for data in &function.blocks {
            for inst in &data.instructions {
                if let Some(result) = function.instructions[inst.index()].result {
                    definition[result.index()] = Some(inst.index());
                }
            }
        }
        let operation_of = |function: &Function, value: Value| -> Option<Operation> {
            definition[value.index()].map(|at| function.instructions[at].operation.clone())
        };
        let fresh = |operation: &Operation| {
            matches!(
                operation,
                Operation::Allocate(_) | Operation::AllocateArray(_)
            )
        };
        let mut changed = false;
        for at in 0..function.instructions.len() {
            let instruction = function.instructions[at].clone();
            let Some(result) = instruction.result else {
                continue;
            };
            let replacement: Option<(Operation, Vec<Value>)> = match &instruction.operation {
                Operation::Compare(Comparison::Equal | Comparison::NotEqual)
                    if instruction.arguments.len() == 2
                        && function.type_of(instruction.arguments[0]) == Type::REFERENCE =>
                {
                    let (left, right) = (
                        operation_of(function, instruction.arguments[0]),
                        operation_of(function, instruction.arguments[1]),
                    );
                    let known = match (&left, &right) {
                        (Some(a), Some(Operation::Null)) if fresh(a) => true,
                        (Some(Operation::Null), Some(b)) if fresh(b) => true,
                        _ => false,
                    };
                    known.then(|| {
                        let equal =
                            matches!(instruction.operation, Operation::Compare(Comparison::Equal));
                        (Operation::IntegerConstant(i128::from(!equal)), Vec::new())
                    })
                }
                Operation::InstanceOf(class) if instruction.arguments.len() == 1 => {
                    match operation_of(function, instruction.arguments[0]) {
                        Some(Operation::Allocate(made) | Operation::AllocateArray(made))
                            if made == *class =>
                        {
                            Some((Operation::IntegerConstant(1), Vec::new()))
                        }
                        _ => None,
                    }
                }
                Operation::Compare(Comparison::Equal | Comparison::NotEqual)
                    if instruction.arguments.len() == 2
                        && zero_against_comparison(
                            function,
                            &definition,
                            &instruction.arguments,
                        )
                        .is_some() =>
                {
                    let source =
                        zero_against_comparison(function, &definition, &instruction.arguments)
                            .and_then(|value| definition[value.index()])
                            .map(|found| function.instructions[found].clone());
                    let negate =
                        matches!(instruction.operation, Operation::Compare(Comparison::Equal));
                    source.and_then(|compare| match (&compare.operation, negate) {
                        (_, false) => Some((compare.operation.clone(), compare.arguments.clone())),
                        (Operation::Compare(inner), true) => Some((
                            Operation::Compare(inverse_comparison(*inner)),
                            compare.arguments.clone(),
                        )),
                        _ => None,
                    })
                }
                Operation::Compare(comparison)
                    if instruction.arguments.len() == 2
                        && integer_type(function.type_of(instruction.arguments[0])) == Some(32) =>
                {
                    let zero = definition[instruction.arguments[1].index()]
                        .map(|found| function.instructions[found].operation.clone());
                    let difference = definition[instruction.arguments[0].index()]
                        .map(|found| function.instructions[found].clone());
                    match (zero, difference) {
                        (Some(Operation::IntegerConstant(0)), Some(sub))
                            if sub.operation == Operation::Binary(BinaryOp::Subtract) =>
                        {
                            three_way_compare(function, &definition, &sub, *comparison)
                        }
                        _ => None,
                    }
                }
                _ => None,
            };
            let _ = result;
            if let Some((operation, arguments)) = replacement {
                let slot = &mut function.instructions[at];
                slot.operation = operation;
                slot.arguments = arguments;
                report.note(&function.name, "pattern-simplified", at as u32);
                changed = true;
            }
        }
        changed
    }

    fn zero_against_comparison(
        function: &Function,
        definition: &[Option<usize>],
        arguments: &[Value],
    ) -> Option<Value> {
        let is_zero = |value: Value| {
            definition[value.index()].is_some_and(|found| {
                function.instructions[found].operation == Operation::IntegerConstant(0)
            })
        };
        if is_zero(arguments[1]) {
            widened_comparison(function, definition, arguments[0])
        } else if is_zero(arguments[0]) {
            widened_comparison(function, definition, arguments[1])
        } else {
            None
        }
    }

    fn widened_comparison(
        function: &Function,
        definition: &[Option<usize>],
        value: Value,
    ) -> Option<Value> {
        let mut current = value;
        let mut steps = 0;
        loop {
            let found = definition[current.index()]?;
            let instruction = &function.instructions[found];
            match &instruction.operation {
                Operation::Convert(Conversion::ZeroExtend | Conversion::SignExtend)
                    if steps < 4 =>
                {
                    current = instruction.arguments[0];
                    steps += 1;
                }
                Operation::Compare(_) | Operation::FloatCompare(_)
                    if steps > 0 && function.type_of(current) == Type::BOOL =>
                {
                    return Some(current);
                }
                _ => return None,
            }
        }
    }

    fn inverse_comparison(comparison: Comparison) -> Comparison {
        match comparison {
            Comparison::Equal => Comparison::NotEqual,
            Comparison::NotEqual => Comparison::Equal,
            Comparison::LessSigned => Comparison::GreaterEqualSigned,
            Comparison::LessEqualSigned => Comparison::GreaterSigned,
            Comparison::GreaterSigned => Comparison::LessEqualSigned,
            Comparison::GreaterEqualSigned => Comparison::LessSigned,
            Comparison::LessUnsigned => Comparison::GreaterEqualUnsigned,
            Comparison::LessEqualUnsigned => Comparison::GreaterUnsigned,
            Comparison::GreaterUnsigned => Comparison::LessEqualUnsigned,
            Comparison::GreaterEqualUnsigned => Comparison::LessUnsigned,
        }
    }

    fn three_way_compare(
        function: &Function,
        definition: &[Option<usize>],
        subtract: &Instruction,
        comparison: Comparison,
    ) -> Option<(Operation, Vec<Value>)> {
        if subtract.arguments.len() != 2 {
            return None;
        }
        let widened = |value: Value| -> Option<Instruction> {
            let found = function.instructions[definition[value.index()]?].clone();
            (found.operation == Operation::Convert(Conversion::ZeroExtend)).then_some(found)
        };
        let greater = widened(subtract.arguments[0])?;
        let lesser = widened(subtract.arguments[1])?;
        let compare = |value: Value| -> Option<(Comparison, Value, Value)> {
            let found = function.instructions[definition[value.index()]?].clone();
            match found.operation {
                Operation::Compare(kind) if found.arguments.len() == 2 => {
                    Some((kind, found.arguments[0], found.arguments[1]))
                }
                _ => None,
            }
        };
        let (gk, ga, gb) = compare(greater.arguments[0])?;
        let (lk, la, lb) = compare(lesser.arguments[0])?;
        if (ga, gb) != (la, lb) {
            return None;
        }
        let signed_pair = gk == Comparison::GreaterSigned && lk == Comparison::LessSigned;
        let unsigned_pair = gk == Comparison::GreaterUnsigned && lk == Comparison::LessUnsigned;
        if !signed_pair && !unsigned_pair {
            return None;
        }
        let mapped = match (comparison, unsigned_pair) {
            (Comparison::Equal, _) => Comparison::Equal,
            (Comparison::NotEqual, _) => Comparison::NotEqual,
            (Comparison::LessSigned, false) => Comparison::LessSigned,
            (Comparison::LessEqualSigned, false) => Comparison::LessEqualSigned,
            (Comparison::GreaterSigned, false) => Comparison::GreaterSigned,
            (Comparison::GreaterEqualSigned, false) => Comparison::GreaterEqualSigned,
            (Comparison::LessSigned, true) => Comparison::LessUnsigned,
            (Comparison::LessEqualSigned, true) => Comparison::LessEqualUnsigned,
            (Comparison::GreaterSigned, true) => Comparison::GreaterUnsigned,
            (Comparison::GreaterEqualSigned, true) => Comparison::GreaterEqualUnsigned,
            _ => return None,
        };
        Some((Operation::Compare(mapped), vec![ga, gb]))
    }

    enum Threaded {
        Existing(Value),
        Constant(u128, Type),
    }

    fn thread_jumps(function: &mut Function, report: &mut Report) -> bool {
        if has_invoke(function) || function.blocks.len() < 2 {
            return false;
        }
        let mut changed = false;
        for _ in 0..4 {
            if !thread_jumps_once(function, report) {
                break;
            }
            changed = true;
        }
        changed
    }

    fn thread_edge(
        function: &Function,
        definition: &[u32],
        scratch: &mut [Lattice],
        block: usize,
        edge: &[Value],
    ) -> Option<(Block, Vec<Threaded>)> {
        let data = &function.blocks[block];
        if data.parameters.len() != edge.len() {
            return None;
        }
        for (parameter, argument) in data.parameters.iter().zip(edge) {
            scratch[parameter.index()] = scratch[argument.index()];
        }
        let outcome = (|| {
            for inst in &data.instructions {
                let instruction = &function.instructions[inst.index()];
                let result = instruction.result?;
                let value = evaluate(function, instruction, scratch);
                if !matches!(value, Lattice::Constant(_)) {
                    return None;
                }
                scratch[result.index()] = value;
            }
            let call = match &data.terminator {
                Terminator::Jump(call) => call,
                Terminator::Branch {
                    condition,
                    taken,
                    otherwise,
                } => match scratch[condition.index()] {
                    Lattice::Constant(0) => otherwise,
                    Lattice::Constant(_) => taken,
                    _ => return None,
                },
                _ => return None,
            };
            let mut arguments = Vec::with_capacity(call.arguments.len());
            for argument in &call.arguments {
                if definition[argument.index()] as usize != block {
                    arguments.push(Threaded::Existing(*argument));
                } else if let Some(at) = data.parameters.iter().position(|held| held == argument) {
                    arguments.push(Threaded::Existing(edge[at]));
                } else if let Lattice::Constant(value) = scratch[argument.index()] {
                    arguments.push(Threaded::Constant(value, function.type_of(*argument)));
                } else {
                    return None;
                }
            }
            Some((call.block, arguments))
        })();
        for parameter in &data.parameters {
            scratch[parameter.index()] = Lattice::Bottom;
        }
        for inst in &data.instructions {
            let instruction = &function.instructions[inst.index()];
            if let (Some(result), false) = (
                instruction.result,
                matches!(instruction.operation, Operation::IntegerConstant(_)),
            ) {
                scratch[result.index()] = Lattice::Bottom;
            }
        }
        outcome
    }

    fn thread_jumps_once(function: &mut Function, report: &mut Report) -> bool {
        let value_count = function.value_types.len();
        let mut definition = vec![u32::MAX; value_count];
        let mut scratch = vec![Lattice::Bottom; value_count];
        for (index, data) in function.blocks.iter().enumerate() {
            for parameter in &data.parameters {
                definition[parameter.index()] = index as u32;
            }
            for inst in &data.instructions {
                let instruction = &function.instructions[inst.index()];
                if let Some(result) = instruction.result {
                    definition[result.index()] = index as u32;
                    if let Operation::IntegerConstant(value) = &instruction.operation {
                        if let Some(bits) = integer_type(function.type_of(result)) {
                            scratch[result.index()] =
                                Lattice::Constant((*value as u128) & mask(bits));
                        }
                    }
                }
            }
        }
        let mut leaks = vec![false; function.blocks.len()];
        {
            let mut note = |owner: usize, value: Value| {
                let home = definition[value.index()];
                if home != u32::MAX && home as usize != owner {
                    leaks[home as usize] = true;
                }
            };
            for (index, data) in function.blocks.iter().enumerate() {
                for inst in &data.instructions {
                    for argument in &function.instructions[inst.index()].arguments {
                        note(index, *argument);
                    }
                }
                match &data.terminator {
                    Terminator::Branch { condition, .. } => note(index, *condition),
                    Terminator::Switch { value, .. } => note(index, *value),
                    Terminator::Return(Some(value)) | Terminator::Throw(value) => {
                        note(index, *value)
                    }
                    _ => {}
                }
                for call in data.terminator.successors() {
                    for argument in &call.arguments {
                        note(index, *argument);
                    }
                }
            }
        }
        let mut eligible = vec![false; function.blocks.len()];
        for (index, data) in function.blocks.iter().enumerate() {
            if index == 0 || leaks[index] {
                continue;
            }
            if !matches!(
                data.terminator,
                Terminator::Jump(_) | Terminator::Branch { .. }
            ) {
                continue;
            }
            eligible[index] = data.instructions.iter().all(|inst| {
                let instruction = &function.instructions[inst.index()];
                foldable(&instruction.operation)
                    && instruction
                        .result
                        .is_some_and(|result| integer_type(function.type_of(result)).is_some())
            });
        }
        let mut changed = false;
        for owner in 0..function.blocks.len() {
            let slots = function.blocks[owner].terminator.successors().len();
            for slot in 0..slots {
                for _ in 0..8 {
                    let call = function.blocks[owner].terminator.successors()[slot].clone();
                    let target = call.block.index();
                    if target == owner || !eligible[target] {
                        break;
                    }
                    let Some((next, threaded)) =
                        thread_edge(function, &definition, &mut scratch, target, &call.arguments)
                    else {
                        break;
                    };
                    if next.index() == target {
                        break;
                    }
                    let mut arguments = Vec::with_capacity(threaded.len());
                    for entry in threaded {
                        match entry {
                            Threaded::Existing(value) => arguments.push(value),
                            Threaded::Constant(value, ty) => {
                                let bits = integer_type(ty).unwrap_or(32);
                                let made = function.new_value(ty);
                                function.instructions.push(Instruction {
                                    operation: Operation::IntegerConstant(canonical_constant(
                                        value, bits,
                                    )),
                                    arguments: Vec::new(),
                                    result: Some(made),
                                    spot: None,
                                });
                                let inst = Inst(function.instructions.len() as u32 - 1);
                                function.blocks[owner].instructions.push(inst);
                                definition.push(owner as u32);
                                scratch.push(Lattice::Constant(value));
                                arguments.push(made);
                            }
                        }
                    }
                    let replacement = BlockCall {
                        block: next,
                        arguments,
                    };
                    *function.blocks[owner].terminator.successors_mut()[slot] = replacement;
                    report.note(&function.name, "jump-threaded", owner as u32);
                    changed = true;
                }
            }
        }
        changed
    }

    fn merge_blocks(function: &mut Function, report: &mut Report) -> bool {
        if has_invoke(function) {
            return false;
        }
        let mut changed = false;
        loop {
            let preds = predecessor_edges(function);
            let mut merged_any = false;
            for block in 0..function.blocks.len() {
                let Terminator::Jump(call) = &function.blocks[block].terminator else {
                    continue;
                };
                let target = call.block.index();
                if target == 0 || target == block || preds[target].len() != 1 {
                    continue;
                }
                let arguments = call.arguments.clone();
                let parameters = function.blocks[target].parameters.clone();
                if arguments.len() != parameters.len() {
                    continue;
                }
                let mut replacements: Vec<Option<Value>> = vec![None; function.value_types.len()];
                for (parameter, argument) in parameters.iter().zip(&arguments) {
                    replacements[parameter.index()] = Some(*argument);
                }
                let moved = std::mem::take(&mut function.blocks[target].instructions);
                let terminator = std::mem::replace(
                    &mut function.blocks[target].terminator,
                    Terminator::Unreachable,
                );
                function.blocks[target].parameters.clear();
                function.blocks[block].instructions.extend(moved);
                function.blocks[block].terminator = terminator;
                apply_replacements(function, &replacements);
                report.note(&function.name, "blocks-merged", block as u32);
                merged_any = true;
                changed = true;
                break;
            }
            if !merged_any {
                break;
            }
            remove_unreachable(function, report);
        }
        changed
    }

    fn is_power_of_two(value: u128) -> Option<u32> {
        (value != 0 && value.is_power_of_two()).then(|| value.trailing_zeros())
    }

    fn simplify_value(
        function: &mut Function,
        out: &mut Vec<Inst>,
        constants: &BTreeMap<Value, u128>,
        instruction: &mut Instruction,
    ) -> Option<(Value, &'static str)> {
        let result = instruction.result?;
        let ty = function.type_of(result);
        let bits = integer_type(ty)?;
        let all = mask(bits);
        let constant_of = |value: Value| constants.get(&value).copied();
        match &instruction.operation.clone() {
            Operation::Binary(op) if instruction.arguments.len() == 2 => {
                let (left, right) = (instruction.arguments[0], instruction.arguments[1]);
                let (left_constant, right_constant) = (constant_of(left), constant_of(right));
                match op {
                    BinaryOp::Add | BinaryOp::Or | BinaryOp::Xor if right_constant == Some(0) => {
                        return Some((left, "identity"));
                    }
                    BinaryOp::Add | BinaryOp::Or | BinaryOp::Xor if left_constant == Some(0) => {
                        return Some((right, "identity"));
                    }
                    BinaryOp::Subtract
                    | BinaryOp::ShiftLeft
                    | BinaryOp::ShiftRightLogical
                    | BinaryOp::ShiftRightArithmetic
                    | BinaryOp::RotateLeft
                    | BinaryOp::RotateRight
                        if right_constant == Some(0) =>
                    {
                        return Some((left, "identity"));
                    }
                    BinaryOp::Multiply if right_constant == Some(1) => {
                        return Some((left, "identity"));
                    }
                    BinaryOp::Multiply if left_constant == Some(1) => {
                        return Some((right, "identity"));
                    }
                    BinaryOp::Multiply if right_constant == Some(0) => {
                        return Some((right, "annihilator"));
                    }
                    BinaryOp::Multiply if left_constant == Some(0) => {
                        return Some((left, "annihilator"));
                    }
                    BinaryOp::And if right_constant == Some(0) => {
                        return Some((right, "annihilator"));
                    }
                    BinaryOp::And if left_constant == Some(0) => {
                        return Some((left, "annihilator"));
                    }
                    BinaryOp::And if right_constant == Some(all) => {
                        return Some((left, "identity"));
                    }
                    BinaryOp::And if left_constant == Some(all) => {
                        return Some((right, "identity"));
                    }
                    BinaryOp::Or if right_constant == Some(all) => {
                        return Some((right, "annihilator"));
                    }
                    BinaryOp::And
                    | BinaryOp::Or
                    | BinaryOp::MinimumSigned
                    | BinaryOp::MaximumSigned
                    | BinaryOp::MinimumUnsigned
                    | BinaryOp::MaximumUnsigned
                        if left == right =>
                    {
                        return Some((left, "idempotent"));
                    }
                    BinaryOp::Subtract | BinaryOp::Xor if left == right => {
                        let zero = insert_constant(function, out, ty, 0);
                        return Some((zero, "self-cancel"));
                    }
                    BinaryOp::DivideUnsigned if right_constant == Some(1) => {
                        return Some((left, "identity"));
                    }
                    BinaryOp::DivideSigned if right_constant == Some(1) => {
                        return Some((left, "identity"));
                    }
                    _ => {}
                }
                if let Some(shift) = right_constant.and_then(is_power_of_two) {
                    match op {
                        BinaryOp::Multiply if shift > 0 && shift < bits => {
                            let amount = insert_constant(function, out, ty, u128::from(shift));
                            instruction.operation = Operation::Binary(BinaryOp::ShiftLeft);
                            instruction.arguments = vec![left, amount];
                            return None;
                        }
                        BinaryOp::DivideUnsigned if shift > 0 && shift < bits => {
                            let amount = insert_constant(function, out, ty, u128::from(shift));
                            instruction.operation = Operation::Binary(BinaryOp::ShiftRightLogical);
                            instruction.arguments = vec![left, amount];
                            return None;
                        }
                        BinaryOp::RemainderUnsigned if shift > 0 && shift < bits => {
                            let low = insert_constant(function, out, ty, (1u128 << shift) - 1);
                            instruction.operation = Operation::Binary(BinaryOp::And);
                            instruction.arguments = vec![left, low];
                            return None;
                        }
                        BinaryOp::DivideSigned | BinaryOp::RemainderSigned
                            if shift > 0 && shift + 1 < bits =>
                        {
                            let top = insert_constant(function, out, ty, u128::from(bits - 1));
                            let sign = insert_binary(
                                function,
                                out,
                                ty,
                                BinaryOp::ShiftRightArithmetic,
                                left,
                                top,
                            );
                            let back = insert_constant(function, out, ty, u128::from(bits - shift));
                            let bias = insert_binary(
                                function,
                                out,
                                ty,
                                BinaryOp::ShiftRightLogical,
                                sign,
                                back,
                            );
                            let biased =
                                insert_binary(function, out, ty, BinaryOp::Add, left, bias);
                            if matches!(op, BinaryOp::DivideSigned) {
                                let amount = insert_constant(function, out, ty, u128::from(shift));
                                let quotient = insert_binary(
                                    function,
                                    out,
                                    ty,
                                    BinaryOp::ShiftRightArithmetic,
                                    biased,
                                    amount,
                                );
                                return Some((quotient, "signed-division-by-power-of-two"));
                            }
                            let keep =
                                insert_constant(function, out, ty, all & !((1u128 << shift) - 1));
                            let truncated =
                                insert_binary(function, out, ty, BinaryOp::And, biased, keep);
                            let remainder = insert_binary(
                                function,
                                out,
                                ty,
                                BinaryOp::Subtract,
                                left,
                                truncated,
                            );
                            return Some((remainder, "signed-remainder-by-power-of-two"));
                        }
                        _ => {}
                    }
                }
                if let Some(divisor) = right_constant {
                    let usable = (bits == 32 || bits == 64) && divisor >= 3;
                    let signed_usable = usable && divisor < (1u128 << (bits - 1));
                    match op {
                        BinaryOp::DivideUnsigned | BinaryOp::RemainderUnsigned
                            if usable && is_power_of_two(divisor).is_none() =>
                        {
                            let quotient =
                                divide_by_constant(function, out, ty, bits, false, left, divisor);
                            if matches!(op, BinaryOp::DivideUnsigned) {
                                return Some((quotient, "unsigned-division-by-constant"));
                            }
                            let constant = insert_constant(function, out, ty, divisor);
                            let product = insert_binary(
                                function,
                                out,
                                ty,
                                BinaryOp::Multiply,
                                quotient,
                                constant,
                            );
                            let remainder =
                                insert_binary(function, out, ty, BinaryOp::Subtract, left, product);
                            return Some((remainder, "unsigned-remainder-by-constant"));
                        }
                        BinaryOp::DivideSigned | BinaryOp::RemainderSigned
                            if signed_usable && is_power_of_two(divisor).is_none() =>
                        {
                            let quotient =
                                divide_by_constant(function, out, ty, bits, true, left, divisor);
                            if matches!(op, BinaryOp::DivideSigned) {
                                return Some((quotient, "signed-division-by-constant"));
                            }
                            let constant = insert_constant(function, out, ty, divisor);
                            let product = insert_binary(
                                function,
                                out,
                                ty,
                                BinaryOp::Multiply,
                                quotient,
                                constant,
                            );
                            let remainder =
                                insert_binary(function, out, ty, BinaryOp::Subtract, left, product);
                            return Some((remainder, "signed-remainder-by-constant"));
                        }
                        _ => {}
                    }
                }
                None
            }
            Operation::Compare(comparison) if instruction.arguments.len() == 2 => {
                if instruction.arguments[0] != instruction.arguments[1] {
                    return None;
                }
                let truth = matches!(
                    comparison,
                    Comparison::Equal
                        | Comparison::LessEqualSigned
                        | Comparison::GreaterEqualSigned
                        | Comparison::LessEqualUnsigned
                        | Comparison::GreaterEqualUnsigned
                );
                let value = insert_constant(function, out, ty, u128::from(truth));
                Some((value, "self-compare"))
            }
            Operation::Select if instruction.arguments.len() == 3 => {
                if instruction.arguments[1] == instruction.arguments[2] {
                    return Some((instruction.arguments[1], "select-same"));
                }
                None
            }
            _ => None,
        }
    }

    fn commutative(operation: &Operation) -> bool {
        matches!(
            operation,
            Operation::Binary(
                BinaryOp::Add
                    | BinaryOp::Multiply
                    | BinaryOp::And
                    | BinaryOp::Or
                    | BinaryOp::Xor
                    | BinaryOp::MinimumSigned
                    | BinaryOp::MaximumSigned
                    | BinaryOp::MinimumUnsigned
                    | BinaryOp::MaximumUnsigned
            ) | Operation::Compare(Comparison::Equal | Comparison::NotEqual)
        )
    }

    fn numbering(function: &mut Function, report: &mut Report) -> bool {
        if has_invoke(function) || function.blocks.is_empty() {
            return false;
        }
        let count = function.blocks.len();
        let preds = predecessor_edges(function);
        let order = reverse_postorder(function);
        let mut position = vec![usize::MAX; count];
        for (index, block) in order.iter().enumerate() {
            position[*block as usize] = index;
        }
        let mut dominator: Vec<Option<u32>> = vec![None; count];
        dominator[0] = Some(0);
        let mut moved = true;
        while moved {
            moved = false;
            for block in order.iter().skip(1) {
                let mut chosen: Option<u32> = None;
                for predecessor in &preds[*block as usize] {
                    if dominator[*predecessor as usize].is_none() {
                        continue;
                    }
                    chosen = Some(match chosen {
                        None => *predecessor,
                        Some(mut left) => {
                            let mut right = *predecessor;
                            while left != right {
                                while position[left as usize] > position[right as usize] {
                                    left = dominator[left as usize].unwrap_or(0);
                                }
                                while position[right as usize] > position[left as usize] {
                                    right = dominator[right as usize].unwrap_or(0);
                                }
                            }
                            left
                        }
                    });
                }
                if chosen.is_some() && dominator[*block as usize] != chosen {
                    dominator[*block as usize] = chosen;
                    moved = true;
                }
            }
        }
        let mut children: Vec<Vec<u32>> = vec![Vec::new(); count];
        for block in order.iter().skip(1) {
            if let Some(parent) = dominator[*block as usize] {
                children[parent as usize].push(*block);
            }
        }
        let mut constants: BTreeMap<Value, u128> = BTreeMap::new();
        let mut table: BTreeMap<String, Value> = BTreeMap::new();
        let mut replacements: Vec<Option<Value>> = vec![None; function.value_types.len()];
        let mut changed = false;
        enum Step {
            Enter(u32),
            Leave(Vec<String>, Option<Value>),
        }
        let mut facts: BTreeMap<Value, bool> = BTreeMap::new();
        let mut stack: Vec<Step> = vec![Step::Enter(0)];
        while let Some(step) = stack.pop() {
            let block = match step {
                Step::Leave(keys, fact) => {
                    for key in keys {
                        table.remove(&key);
                    }
                    if let Some(value) = fact {
                        facts.remove(&value);
                    }
                    continue;
                }
                Step::Enter(block) => block as usize,
            };
            let mut learned: Option<Value> = None;
            if let [only] = preds[block].as_slice() {
                if let Terminator::Branch {
                    condition,
                    taken,
                    otherwise,
                } = &function.blocks[*only as usize].terminator
                {
                    if taken.block != otherwise.block {
                        let value = resolve(&replacements, *condition);
                        let on_taken = taken.block.index() == block;
                        let on_otherwise = otherwise.block.index() == block;
                        if (on_taken || on_otherwise) && !facts.contains_key(&value) {
                            facts.insert(value, on_taken);
                            learned = Some(value);
                        }
                    }
                }
            }
            let mut added: Vec<String> = Vec::new();
            let old = function.blocks[block].instructions.clone();
            let mut out: Vec<Inst> = Vec::with_capacity(old.len());
            for inst in old {
                let mut instruction = function.instructions[inst.index()].clone();
                for argument in instruction.arguments.iter_mut() {
                    *argument = resolve(&replacements, *argument);
                }
                if replacements.len() < function.value_types.len() {
                    replacements.resize(function.value_types.len(), None);
                }
                let simplified = simplify_value(function, &mut out, &constants, &mut instruction);
                if replacements.len() < function.value_types.len() {
                    replacements.resize(function.value_types.len(), None);
                }
                if let (Some((value, rule)), Some(result)) = (simplified, instruction.result) {
                    replacements[result.index()] = Some(value);
                    report.note(&function.name, rule, block as u32);
                    changed = true;
                    continue;
                }
                if let (Operation::IntegerConstant(value), Some(result)) =
                    (&instruction.operation, instruction.result)
                {
                    let bits = integer_type(function.type_of(result)).unwrap_or(64);
                    constants.insert(result, (*value as u128) & mask(bits));
                }
                if commutative(&instruction.operation)
                    && instruction.arguments.len() == 2
                    && instruction.arguments[0].0 > instruction.arguments[1].0
                {
                    instruction.arguments.swap(0, 1);
                }
                if let (true, Some(result)) = (
                    pure_instruction(&instruction.operation)
                        || matches!(instruction.operation, Operation::ArrayLength),
                    instruction.result,
                ) {
                    let key = format!(
                        "{:?}|{:?}|{:?}",
                        instruction.operation,
                        instruction.arguments,
                        function.type_of(result)
                    );
                    if let Some(found) = table.get(&key) {
                        replacements[result.index()] = Some(*found);
                        report.note(&function.name, "value-numbered", block as u32);
                        changed = true;
                        continue;
                    }
                    table.insert(key.clone(), result);
                    added.push(key);
                }
                if function.instructions[inst.index()] != instruction {
                    function.instructions[inst.index()] = instruction;
                }
                out.push(inst);
            }
            function.blocks[block].instructions = out;
            if let Terminator::Branch {
                condition,
                taken,
                otherwise,
            } = &function.blocks[block].terminator
            {
                let value = resolve(&replacements, *condition);
                if let Some(known) = facts.get(&value) {
                    let chosen = if *known {
                        taken.clone()
                    } else {
                        otherwise.clone()
                    };
                    function.blocks[block].terminator = Terminator::Jump(chosen);
                    report.note(&function.name, "branch-implied", block as u32);
                    changed = true;
                }
            }
            stack.push(Step::Leave(added, learned));
            for child in children[block].iter().rev() {
                stack.push(Step::Enter(*child));
            }
        }
        if changed {
            replacements.resize(function.value_types.len(), None);
            apply_replacements(function, &replacements);
        }
        changed
    }

    const INLINE_SIZE_LIMIT: usize = 24;
    const INLINE_HINTED_SIZE_LIMIT: usize = 64;
    const INLINE_BLOCK_LIMIT: usize = 8;
    const INLINE_GROWTH_LIMIT: usize = 6000;
    const INLINE_ROUNDS: usize = 4;

    fn function_size(function: &Function) -> usize {
        function
            .blocks
            .iter()
            .map(|data| data.instructions.len() + 1)
            .sum()
    }

    fn call_sites(module: &Module) -> Vec<usize> {
        let mut counts = vec![0usize; module.functions.len()];
        for function in &module.functions {
            for data in &function.blocks {
                for inst in &data.instructions {
                    if let Operation::Call(id) = function.instructions[inst.index()].operation {
                        if let Some(slot) = counts.get_mut(id.index()) {
                            *slot += 1;
                        }
                    }
                }
            }
        }
        counts
    }

    fn inline_limits(function: &Function, sites: usize) -> (usize, usize) {
        if function.attributes.contains(&Attribute::Inline) {
            return (INLINE_HINTED_SIZE_LIMIT.max(400), 64);
        }
        if sites == 1 && function.linkage == Linkage::Internal {
            return (600, 100);
        }
        if sites <= 3 && function.linkage == Linkage::Internal {
            return (160, 40);
        }
        (INLINE_SIZE_LIMIT, INLINE_BLOCK_LIMIT)
    }

    fn inlinable(function: &Function, sites: usize) -> bool {
        if function.is_declaration() || has_invoke(function) || function.signature.variadic {
            return false;
        }
        if matches!(function.linkage, Linkage::Import | Linkage::Weak) {
            return false;
        }
        let attributes = &function.attributes;
        if attributes.contains(&Attribute::NoInline)
            || attributes.contains(&Attribute::NoReturn)
            || attributes.contains(&Attribute::Entry)
        {
            return false;
        }
        let (limit, block_limit) = inline_limits(function, sites);
        if function.blocks.len() > block_limit || function_size(function) > limit {
            return false;
        }
        function.blocks.iter().all(|data| {
            data.instructions.iter().all(|inst| {
                !matches!(
                    function.instructions[inst.index()].operation,
                    Operation::StackSlot { .. }
                        | Operation::DynamicStackSlot { .. }
                        | Operation::StackSave
                        | Operation::StackRestore
                )
            })
        })
    }

    fn throws(function: &Function) -> bool {
        function
            .blocks
            .iter()
            .any(|data| matches!(data.terminator, Terminator::Throw(_)))
    }

    fn calls_only_acyclic_code(module: &Module) -> Vec<bool> {
        let count = module.functions.len();
        let mut callees: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); count];
        for (index, function) in module.functions.iter().enumerate() {
            let mut note = |instruction: &Instruction| {
                if let Operation::Call(id) = instruction.operation {
                    if id.index() < count
                        && !module.functions[id.index()]
                            .name
                            .ends_with(".<ensure>.slow")
                    {
                        callees[index].insert(id.index());
                    }
                }
            };
            for data in &function.blocks {
                for inst in &data.instructions {
                    note(&function.instructions[inst.index()]);
                }
                if let Terminator::Invoke { call, .. } = &data.terminator {
                    note(call);
                }
            }
        }
        let mut callers: Vec<Vec<usize>> = vec![Vec::new(); count];
        let mut pending: Vec<usize> = Vec::with_capacity(count);
        for (index, targets) in callees.iter().enumerate() {
            pending.push(targets.len());
            for target in targets {
                callers[*target].push(index);
            }
        }
        let mut settled = vec![false; count];
        let mut queue: Vec<usize> = (0..count).filter(|index| pending[*index] == 0).collect();
        while let Some(index) = queue.pop() {
            if settled[index] {
                continue;
            }
            settled[index] = true;
            for caller in &callers[index] {
                pending[*caller] -= 1;
                if pending[*caller] == 0 {
                    queue.push(*caller);
                }
            }
        }
        settled
    }

    fn inline_one(function: &mut Function, block: usize, position: usize, callee: &Function) {
        let call_inst = function.blocks[block].instructions[position];
        let call = function.instructions[call_inst.index()].clone();
        let tail: Vec<Inst> = function.blocks[block].instructions.split_off(position + 1);
        function.blocks[block].instructions.pop();
        let terminator = std::mem::replace(
            &mut function.blocks[block].terminator,
            Terminator::Unreachable,
        );
        let continuation = Block(function.blocks.len() as u32);
        let result_type = call.result.map(|value| function.type_of(value));
        let result_parameter = result_type.map(|ty| function.new_value(ty));
        function.blocks.push(BlockData {
            parameters: result_parameter.into_iter().collect(),
            instructions: tail,
            terminator,
        });
        let base = function.blocks.len() as u32;
        let mut map: Vec<Value> = Vec::with_capacity(callee.value_types.len());
        for ty in &callee.value_types {
            map.push(function.new_value(*ty));
        }
        for data in &callee.blocks {
            let mut instructions = Vec::with_capacity(data.instructions.len());
            for inst in &data.instructions {
                let mut instruction = callee.instructions[inst.index()].clone();
                instruction
                    .arguments
                    .iter_mut()
                    .for_each(|value| *value = map[value.index()]);
                if let Some(result) = instruction.result.as_mut() {
                    *result = map[result.index()];
                }
                function.instructions.push(instruction);
                instructions.push(Inst(function.instructions.len() as u32 - 1));
            }
            let mut cloned = data.terminator.clone();
            terminator_values_mut(&mut cloned, &mut |value| *value = map[value.index()]);
            for edge in cloned.successors_mut() {
                edge.block = Block(edge.block.0 + base);
            }
            if let Terminator::Return(value) = &cloned {
                cloned = Terminator::Jump(BlockCall {
                    block: continuation,
                    arguments: value.iter().copied().collect(),
                });
            }
            function.blocks.push(BlockData {
                parameters: data
                    .parameters
                    .iter()
                    .map(|value| map[value.index()])
                    .collect(),
                instructions,
                terminator: cloned,
            });
        }
        function.blocks[block].terminator = Terminator::Jump(BlockCall {
            block: Block(base),
            arguments: call.arguments.clone(),
        });
        if let (Some(old), Some(new)) = (call.result, result_parameter) {
            let mut replacements = vec![None; function.value_types.len()];
            replacements[old.index()] = Some(new);
            apply_replacements(function, &replacements);
        }
    }

    fn inline_calls(
        function: &mut Function,
        candidates: &[Option<Function>],
        report: &mut Report,
    ) -> bool {
        let mut changed = false;
        let mut from = 0usize;
        let unwinds_forbidden = function.attributes.contains(&Attribute::NoUnwind);
        'search: loop {
            if function_size(function) > INLINE_GROWTH_LIMIT {
                break;
            }
            let mut block = from;
            while block < function.blocks.len() {
                for position in 0..function.blocks[block].instructions.len() {
                    let inst = function.blocks[block].instructions[position];
                    let Operation::Call(id) = function.instructions[inst.index()].operation else {
                        continue;
                    };
                    let Some(Some(callee)) = candidates.get(id.index()) else {
                        continue;
                    };
                    if callee.name == function.name || (unwinds_forbidden && throws(callee)) {
                        continue;
                    }
                    let entry_parameters = callee.blocks[0].parameters.len();
                    if function.instructions[inst.index()].arguments.len() != entry_parameters {
                        continue;
                    }
                    inline_one(function, block, position, callee);
                    report.note(&function.name, "inlined", block as u32);
                    changed = true;
                    from = block;
                    continue 'search;
                }
                block += 1;
            }
            break;
        }
        changed
    }

    pub fn split_initialization_guards(module: &mut Module, report: &mut Report) -> bool {
        let mut split: Vec<(usize, Function)> = Vec::new();
        for (index, function) in module.functions.iter().enumerate() {
            if !function.name.ends_with(".<ensure>")
                || !function.signature.parameters.is_empty()
                || function.signature.result.is_some()
                || function.blocks.len() != 3
                || function.linkage != Linkage::Internal
            {
                continue;
            }
            let entry = &function.blocks[0];
            let guarded = matches!(
                &entry.terminator,
                Terminator::Branch { taken, otherwise, .. }
                    if taken.block.index() == 1
                        && taken.arguments.is_empty()
                        && otherwise.block.index() == 2
                        && otherwise.arguments.is_empty()
            );
            let quiet = function.blocks[2].instructions.is_empty()
                && matches!(function.blocks[2].terminator, Terminator::Return(None));
            let slow = matches!(function.blocks[1].terminator, Terminator::Return(None))
                && function.blocks[1].instructions.len() > 1;
            if guarded && quiet && slow && entry.instructions.len() <= 5 {
                split.push((index, function.clone()));
            }
        }
        if split.is_empty() {
            return false;
        }
        for (index, original) in split {
            let mut slow = original.clone();
            slow.name = format!("{}.slow", original.name);
            let slow_id = module.add_function(slow);
            let guard = &mut module.functions[index];
            guard.instructions.push(Instruction {
                operation: Operation::Call(slow_id),
                arguments: Vec::new(),
                result: None,
                spot: None,
            });
            let call = Inst(guard.instructions.len() as u32 - 1);
            guard.blocks[1].instructions = vec![call];
            report.note(&guard.name, "guard-split", index as u32);
        }
        true
    }

    pub fn inline_small_functions(module: &mut Module, report: &mut Report) -> bool {
        inline_small_functions_marking(module, report)
            .iter()
            .any(|changed| *changed)
    }

    fn inline_small_functions_marking(module: &mut Module, report: &mut Report) -> Vec<bool> {
        let mut touched = vec![false; module.functions.len()];
        for _ in 0..INLINE_ROUNDS {
            let acyclic = calls_only_acyclic_code(module);
            let sites = call_sites(module);
            let candidates: Vec<Option<Function>> = module
                .functions
                .iter()
                .enumerate()
                .map(|(index, function)| {
                    (acyclic[index] && inlinable(function, sites[index])).then(|| function.clone())
                })
                .collect();
            if candidates.iter().all(Option::is_none) {
                break;
            }
            let mut changed = false;
            for (index, function) in module.functions.iter_mut().enumerate() {
                if function.is_declaration() || has_invoke(function) {
                    continue;
                }
                if inline_calls(function, &candidates, report) {
                    touched[index] = true;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        touched
    }

    fn for_each_module_instruction(module: &mut Module, mut visit: impl FnMut(&mut Instruction)) {
        for function in module.functions.iter_mut() {
            function.instructions.iter_mut().for_each(&mut visit);
            for data in function.blocks.iter_mut() {
                if let Terminator::Invoke { call, .. } = &mut data.terminator {
                    visit(call);
                }
            }
        }
    }

    fn merge_strings(module: &mut Module, report: &mut Report) -> bool {
        let mut first: BTreeMap<String, u32> = BTreeMap::new();
        let mut kept: Vec<String> = Vec::new();
        let mut renumber: Vec<u32> = Vec::with_capacity(module.strings.len());
        for text in &module.strings {
            let next = kept.len() as u32;
            let at = *first.entry(text.clone()).or_insert(next);
            if at == next {
                kept.push(text.clone());
            }
            renumber.push(at);
        }
        if kept.len() == module.strings.len() {
            return false;
        }
        for_each_module_instruction(module, |instruction| {
            if let Operation::StringConstant(index) = &mut instruction.operation {
                if let Some(next) = renumber.get(*index as usize) {
                    *index = *next;
                }
            }
        });
        for index in 0..module.strings.len() - kept.len() {
            report.note(&module.name, "string-merged", index as u32);
        }
        module.strings = kept;
        true
    }

    type ConstantKey = (u32, u32, bool, Vec<u8>, Vec<(u32, i64, bool, u32)>);

    fn mergeable(global: &Global) -> bool {
        !global.mutable && !global.thread_local && global.linkage == Linkage::Internal
    }

    fn constant_key(global: &Global, target: &[usize]) -> ConstantKey {
        let (filled, bytes) = match &global.content {
            GlobalContent::Zero => (false, Vec::new()),
            GlobalContent::Bytes(bytes) => (true, bytes.clone()),
        };
        let relocations = global
            .relocations
            .iter()
            .map(|relocation| match relocation.target {
                Symbol::Global(id) => (
                    relocation.offset,
                    relocation.addend,
                    true,
                    target.get(id.index()).copied().unwrap_or(id.index()) as u32,
                ),
                Symbol::Function(id) => (
                    relocation.offset,
                    relocation.addend,
                    false,
                    id.index() as u32,
                ),
            })
            .collect();
        (global.size, global.align, filled, bytes, relocations)
    }

    fn merge_constants(module: &mut Module, report: &mut Report) -> bool {
        let count = module.globals.len();
        let mut target: Vec<usize> = (0..count).collect();
        loop {
            let mut seen: BTreeMap<ConstantKey, usize> = BTreeMap::new();
            let mut next: Vec<usize> = (0..count).collect();
            for (index, global) in module.globals.iter().enumerate() {
                if !mergeable(global) {
                    continue;
                }
                let key = constant_key(global, &target);
                next[index] = *seen.entry(key).or_insert(index);
            }
            if next == target {
                break;
            }
            target = next;
        }
        if (0..count).all(|index| target[index] == index) {
            return false;
        }
        let mut renumber: Vec<u32> = vec![0; count];
        let mut kept = 0u32;
        for index in 0..count {
            if target[index] == index {
                renumber[index] = kept;
                kept += 1;
            } else {
                renumber[index] = renumber[target[index]];
                report.note(&module.name, "constant-merged", index as u32);
            }
        }
        for_each_module_instruction(module, |instruction| {
            if let Operation::GlobalAddress(id) = &mut instruction.operation {
                if let Some(next) = renumber.get(id.index()) {
                    *id = GlobalId(*next);
                }
            }
        });
        let old = std::mem::take(&mut module.globals);
        for (index, mut global) in old.into_iter().enumerate() {
            if target[index] != index {
                continue;
            }
            for relocation in global.relocations.iter_mut() {
                if let Symbol::Global(id) = &mut relocation.target {
                    if let Some(next) = renumber.get(id.index()) {
                        *id = GlobalId(*next);
                    }
                }
            }
            module.globals.push(global);
        }
        true
    }

    fn fresh_name(prefix: &str, ordinal: usize, fixed: &BTreeSet<String>) -> String {
        let mut name = format!("{prefix}{ordinal}");
        let mut extra = 0usize;
        while fixed.contains(&name) {
            extra += 1;
            name = format!("{prefix}{ordinal}.{extra}");
        }
        name
    }

    fn strip_names(module: &mut Module, report: &mut Report) -> bool {
        let mut changed = false;
        let mut fixed: BTreeSet<String> = BTreeSet::new();
        for function in &module.functions {
            if function.linkage != Linkage::Internal {
                fixed.insert(function.name.clone());
            }
        }
        for global in &module.globals {
            if global.linkage != Linkage::Internal {
                fixed.insert(global.name.clone());
            }
        }
        let mut ordinal = 0usize;
        for (index, function) in module.functions.iter_mut().enumerate() {
            if function.linkage != Linkage::Internal || function.is_declaration() {
                continue;
            }
            let name = fresh_name(".f", ordinal, &fixed);
            ordinal += 1;
            if function.name != name {
                function.name = name;
                report.note(&module.name, "symbol-stripped", index as u32);
                changed = true;
            }
        }
        let mut ordinal = 0usize;
        for (index, global) in module.globals.iter_mut().enumerate() {
            if global.linkage != Linkage::Internal {
                continue;
            }
            let name = fresh_name(".g", ordinal, &fixed);
            ordinal += 1;
            if global.name != name {
                global.name = name;
                report.note(&module.name, "symbol-stripped", index as u32);
                changed = true;
            }
        }
        let fixed: BTreeSet<String> = module
            .classes
            .iter()
            .map(|class| class.name.clone())
            .collect();
        for (index, structure) in module.structs.iter_mut().enumerate() {
            let name = fresh_name(".s", index, &fixed);
            if structure.name != name {
                structure.name = name;
                report.note(&module.name, "symbol-stripped", index as u32);
                changed = true;
            }
        }
        changed
    }

    fn drop_debug_info(module: &mut Module, report: &mut Report) -> bool {
        let mut dropped = 0u32;
        for_each_module_instruction(module, |instruction| {
            if instruction.spot.take().is_some() {
                dropped += 1;
            }
        });
        let had_files = !module.files.is_empty();
        module.files.clear();
        if dropped > 0 {
            report.note(&module.name, "debug-dropped", dropped);
        }
        dropped > 0 || had_files
    }

    pub fn strip_and_pack_symbols(module: &mut Module, report: &mut Report) -> bool {
        let mut changed = false;
        changed |= merge_strings(module, report);
        changed |= merge_constants(module, report);
        changed |= strip_names(module, report);
        changed |= drop_debug_info(module, report);
        changed
    }

    pub fn build_fingerprint(module: &Module) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in module.canonical().to_binary() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash
    }

    pub fn optimize_function(function: &mut Function, report: &mut Report) {
        if function.is_declaration() || has_invoke(function) {
            return;
        }
        for _ in 0..8 {
            let mut changed = false;
            changed |= remove_unreachable(function, report);
            changed |= promote_slots(function, report);
            changed |= propagate_constants(function, report);
            changed |= remove_unreachable(function, report);
            changed |= simplify_patterns(function, report);
            changed |= thread_jumps(function, report);
            changed |= remove_unreachable(function, report);
            changed |= numbering(function, report);
            changed |= hoist_invariants(function, report);
            changed |= prune_parameters(function, report);
            changed |= eliminate_dead(function, report);
            changed |= merge_blocks(function, report);
            if !changed {
                break;
            }
        }
        compact_instructions(function);
        compact_values(function);
    }

    fn optimize_all(functions: &mut [Function], report: &mut Report) {
        optimize_selected(functions, None, report);
    }

    fn optimize_selected(functions: &mut [Function], only: Option<&[bool]>, report: &mut Report) {
        let wanted = |index: usize| only.is_none_or(|marks| marks[index]);
        let workers = std::thread::available_parallelism()
            .map_or(1, |count| count.get())
            .min(8);
        if workers < 2 || functions.len() < 64 || report.keep_log {
            for (index, function) in functions.iter_mut().enumerate() {
                if wanted(index) {
                    optimize_function(function, report);
                }
            }
            return;
        }
        let keep_log = report.keep_log;
        let mut queue: Vec<&mut Function> = functions
            .iter_mut()
            .enumerate()
            .filter(|(index, _)| wanted(*index))
            .map(|(_, function)| function)
            .collect();
        queue.sort_by_key(|function| std::cmp::Reverse(function_size(function)));
        queue.reverse();
        let queue = std::sync::Mutex::new(queue);
        let parts: Vec<Report> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..workers)
                .map(|_| {
                    scope.spawn(|| {
                        let mut local = Report {
                            keep_log,
                            ..Report::default()
                        };
                        loop {
                            let next = queue.lock().ok().and_then(|mut held| held.pop());
                            let Some(function) = next else {
                                break;
                            };
                            optimize_function(function, &mut local);
                        }
                        local
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap_or_default())
                .collect()
        });
        for part in parts {
            report.absorb(part);
        }
    }

    pub fn optimize_module(module: &mut Module, report: &mut Report) {
        optimize_all(&mut module.functions, report);
        let split = split_initialization_guards(module, report);
        let touched = inline_small_functions_marking(module, report);
        if split {
            optimize_all(&mut module.functions, report);
        } else if touched.iter().any(|changed| *changed) {
            optimize_selected(&mut module.functions, Some(&touched), report);
        }
    }

    pub fn checked(module: &Module) -> (Module, Report, bool) {
        let mut copy = module.clone();
        let mut report = Report::default();
        optimize_module(&mut copy, &mut report);
        if copy.verify().is_ok() {
            (copy, report, true)
        } else {
            (module.clone(), Report::default(), false)
        }
    }

    pub fn release(module: &Module) -> (Module, Report, bool) {
        let (checked_module, mut report, fine) = checked(module);
        if !fine {
            return (checked_module, report, false);
        }
        let mut stripped = checked_module.clone();
        strip_and_pack_symbols(&mut stripped, &mut report);
        if stripped.verify().is_ok() {
            (stripped, report, true)
        } else {
            (checked_module, report, false)
        }
    }
}

pub mod wide {
    use super::*;

    const HELPERS: &str = r#"ir 1
module "ir.wide"
layout lp64
function @ir.wide.udivmod(i64, i64, i64, i64, ptr) internal {
block0(%al: i64, %ah: i64, %bl: i64, %bh: i64, %out: ptr):
    %zero: i64 = const 0
    %one: i64 = const 1
    %top: i64 = const 63
    %last: i64 = const 128
    %orb: i64 = or %bl, %bh
    %isz: bool = icmp eq %orb, %zero
    branch %isz, block1, block2(%al, %ah, %zero, %zero, %zero)
block1:
    %trapped: i64 = udiv %one, %orb
    unreachable
block2(%ql: i64, %qh: i64, %rl: i64, %rh: i64, %i: i64):
    %carry: i64 = lshr %rh, %top
    %carried: bool = icmp ne %carry, %zero
    %msb: i64 = lshr %qh, %top
    %t1: i64 = shl %rh, %one
    %t2: i64 = lshr %rl, %top
    %rh1: i64 = or %t1, %t2
    %t3: i64 = shl %rl, %one
    %rl1: i64 = or %t3, %msb
    %t4: i64 = shl %qh, %one
    %t5: i64 = lshr %ql, %top
    %qh1: i64 = or %t4, %t5
    %ql1: i64 = shl %ql, %one
    %gt: bool = icmp ugt %rh1, %bh
    %eq: bool = icmp eq %rh1, %bh
    %lge: bool = icmp uge %rl1, %bl
    %ge0: bool = select %eq, %lge, %gt
    %ge: bool = or %carried, %ge0
    branch %ge, block3, block4(%ql1, %qh1, %rl1, %rh1)
block3:
    %sl: i64 = sub %rl1, %bl
    %bw: bool = icmp ult %rl1, %bl
    %bwi: i64 = zext %bw
    %sh0: i64 = sub %rh1, %bh
    %sh: i64 = sub %sh0, %bwi
    %qs: i64 = or %ql1, %one
    jump block4(%qs, %qh1, %sl, %sh)
block4(%fql: i64, %fqh: i64, %frl: i64, %frh: i64):
    %i1: i64 = add %i, %one
    %fin: bool = icmp eq %i1, %last
    branch %fin, block5, block2(%fql, %fqh, %frl, %frh, %i1)
block5:
    %c8: i64 = const 8
    %c16: i64 = const 16
    %c24: i64 = const 24
    %p1: ptr = offset %out, %c8
    %p2: ptr = offset %out, %c16
    %p3: ptr = offset %out, %c24
    store align 8 %fql, %out
    store align 8 %fqh, %p1
    store align 8 %frl, %p2
    store align 8 %frh, %p3
    return
}
function @ir.wide.sdivmod(i64, i64, i64, i64, ptr) internal {
block0(%al: i64, %ah: i64, %bl: i64, %bh: i64, %out: ptr):
    %top: i64 = const 63
    %c8: i64 = const 8
    %c16: i64 = const 16
    %c24: i64 = const 24
    %sa: i64 = ashr %ah, %top
    %sb: i64 = ashr %bh, %top
    %xl: i64 = xor %al, %sa
    %xh: i64 = xor %ah, %sa
    %wa: bool = icmp ult %xl, %sa
    %wai: i64 = zext %wa
    %nal: i64 = sub %xl, %sa
    %nah0: i64 = sub %xh, %sa
    %nah: i64 = sub %nah0, %wai
    %yl: i64 = xor %bl, %sb
    %yh: i64 = xor %bh, %sb
    %wb: bool = icmp ult %yl, %sb
    %wbi: i64 = zext %wb
    %nbl: i64 = sub %yl, %sb
    %nbh0: i64 = sub %yh, %sb
    %nbh: i64 = sub %nbh0, %wbi
    call @ir.wide.udivmod(%nal, %nah, %nbl, %nbh, %out)
    %p1: ptr = offset %out, %c8
    %p2: ptr = offset %out, %c16
    %p3: ptr = offset %out, %c24
    %ql: i64 = load align 8 %out
    %qh: i64 = load align 8 %p1
    %rl: i64 = load align 8 %p2
    %rh: i64 = load align 8 %p3
    %sq: i64 = xor %sa, %sb
    %zl: i64 = xor %ql, %sq
    %zh: i64 = xor %qh, %sq
    %wq: bool = icmp ult %zl, %sq
    %wqi: i64 = zext %wq
    %oql: i64 = sub %zl, %sq
    %oqh0: i64 = sub %zh, %sq
    %oqh: i64 = sub %oqh0, %wqi
    %vl: i64 = xor %rl, %sa
    %vh: i64 = xor %rh, %sa
    %wr: bool = icmp ult %vl, %sa
    %wri: i64 = zext %wr
    %orl: i64 = sub %vl, %sa
    %orh0: i64 = sub %vh, %sa
    %orh: i64 = sub %orh0, %wri
    store align 8 %oql, %out
    store align 8 %oqh, %p1
    store align 8 %orl, %p2
    store align 8 %orh, %p3
    return
}
"#;

    const FLOAT_HELPERS: &str = r#"ir 1
module "ir.wide.float"
layout lp64
function @ir.wide.u128_to_f64(i128) -> f64 internal {
block0(%x: i128):
    %zero: i128 = const 0
    %isz: bool = icmp eq %x, %zero
    branch %isz, block1, block2
block1:
    %zf: f64 = fconst 0x0
    return %zf
block2:
    %n: i128 = clz %x
    %sh: i128 = shl %x, %n
    %c64: i128 = const 64
    %upper: i128 = lshr %sh, %c64
    %top: i64 = trunc %upper
    %rest: i64 = trunc %sh
    %z64: i64 = const 0
    %st: bool = icmp ne %rest, %z64
    %sti: i64 = zext %st
    %top2: i64 = or %top, %sti
    %f: f64 = uitofp %top2
    %n64: i64 = trunc %n
    %base: i64 = const 1087
    %e: i64 = sub %base, %n64
    %c52: i64 = const 52
    %eb: i64 = shl %e, %c52
    %scale: f64 = bitcast %eb
    %r: f64 = fmul %f, %scale
    return %r
}
function @ir.wide.u128_to_f32(i128) -> f32 internal {
block0(%x: i128):
    %zero: i128 = const 0
    %isz: bool = icmp eq %x, %zero
    branch %isz, block1, block2
block1:
    %zf: f32 = fconst 0x0
    return %zf
block2:
    %n: i128 = clz %x
    %sh: i128 = shl %x, %n
    %c64: i128 = const 64
    %upper: i128 = lshr %sh, %c64
    %top: i64 = trunc %upper
    %rest: i64 = trunc %sh
    %z64: i64 = const 0
    %st: bool = icmp ne %rest, %z64
    %sti: i64 = zext %st
    %top2: i64 = or %top, %sti
    %f: f32 = uitofp %top2
    %n32: i32 = trunc %n
    %base: i32 = const 191
    %e: i32 = sub %base, %n32
    %c23: i32 = const 23
    %eb: i32 = shl %e, %c23
    %scale: f32 = bitcast %eb
    %r: f32 = fmul %f, %scale
    return %r
}
function @ir.wide.umul_overflows(i128, i128) -> bool internal {
block0(%a: i128, %b: i128):
    %zero: i128 = const 0
    %az: bool = icmp eq %a, %zero
    branch %az, block1, block2
block1:
    %no: bool = icmp ne %zero, %zero
    return %no
block2:
    %r: i128 = mul %a, %b
    %q: i128 = udiv %r, %a
    %bad: bool = icmp ne %q, %b
    return %bad
}
function @ir.wide.smul_overflows(i128, i128) -> bool internal {
block0(%a: i128, %b: i128):
    %zero: i128 = const 0
    %az: bool = icmp eq %a, %zero
    branch %az, block1, block2
block1:
    %no: bool = icmp ne %zero, %zero
    return %no
block2:
    %r: i128 = mul %a, %b
    %q: i128 = sdiv %r, %a
    %bad: bool = icmp ne %q, %b
    %m1: i128 = const -1
    %lowest: i128 = const -170141183460469231731687303715884105728
    %am1: bool = icmp eq %a, %m1
    %bmin: bool = icmp eq %b, %lowest
    %special: bool = and %am1, %bmin
    %res: bool = or %bad, %special
    return %res
}
function @ir.wide.s128_to_f64(i128) -> f64 internal {
block0(%x: i128):
    %zero: i128 = const 0
    %neg: bool = icmp slt %x, %zero
    %m: i128 = abs %x
    %r: f64 = call @ir.wide.u128_to_f64(%m)
    %nr: f64 = fneg %r
    %res: f64 = select %neg, %nr, %r
    return %res
}
function @ir.wide.s128_to_f32(i128) -> f32 internal {
block0(%x: i128):
    %zero: i128 = const 0
    %neg: bool = icmp slt %x, %zero
    %m: i128 = abs %x
    %r: f32 = call @ir.wide.u128_to_f32(%m)
    %nr: f32 = fneg %r
    %res: f32 = select %neg, %nr, %r
    return %res
}
function @ir.wide.magnitude(f64) -> i128 internal {
block0(%v: f64):
    %b: i64 = bitcast %v
    %c52: i64 = const 52
    %m11: i64 = const 2047
    %bias: i64 = const 1023
    %frac_mask: i64 = const 4503599627370495
    %implicit: i64 = const 4503599627370496
    %ex: i64 = lshr %b, %c52
    %ex2: i64 = and %ex, %m11
    %frac: i64 = and %b, %frac_mask
    %mant: i64 = or %frac, %implicit
    %e: i64 = sub %ex2, %bias
    %m: i128 = zext %mant
    %zero64: i64 = const 0
    %small: bool = icmp slt %e, %zero64
    branch %small, block1, block2
block1:
    %z: i128 = const 0
    return %z
block2:
    %ge52: bool = icmp sge %e, %c52
    %upn: i64 = sub %e, %c52
    %downn: i64 = sub %c52, %e
    %upa: i128 = zext %upn
    %downa: i128 = zext %downn
    %l: i128 = shl %m, %upa
    %r: i128 = lshr %m, %downa
    %res: i128 = select %ge52, %l, %r
    return %res
}
function @ir.wide.f64_to_u128(f64) -> i128 internal {
block0(%v: f64):
    %b: i64 = bitcast %v
    %c52: i64 = const 52
    %c63: i64 = const 63
    %m11: i64 = const 2047
    %bias: i64 = const 1023
    %zero64: i64 = const 0
    %c128: i64 = const 128
    %sign: i64 = lshr %b, %c63
    %ex: i64 = lshr %b, %c52
    %ex2: i64 = and %ex, %m11
    %e: i64 = sub %ex2, %bias
    %nan: bool = icmp eq %ex2, %m11
    %neg: bool = icmp ne %sign, %zero64
    %big: bool = icmp sge %e, %c128
    %small: bool = icmp slt %e, %zero64
    %notsmall: bool = icmp sge %e, %zero64
    %outside: bool = or %big, %nan
    %negbig: bool = and %neg, %notsmall
    %bad: bool = or %outside, %negbig
    branch %bad, block1, block2
block1:
    trap 4
    unreachable
block2:
    %r: i128 = call @ir.wide.magnitude(%v)
    return %r
}
function @ir.wide.f64_to_s128(f64) -> i128 internal {
block0(%v: f64):
    %b: i64 = bitcast %v
    %c52: i64 = const 52
    %c63: i64 = const 63
    %m11: i64 = const 2047
    %bias: i64 = const 1023
    %zero64: i64 = const 0
    %c127: i64 = const 127
    %frac_mask: i64 = const 4503599627370495
    %sign: i64 = lshr %b, %c63
    %ex: i64 = lshr %b, %c52
    %ex2: i64 = and %ex, %m11
    %frac: i64 = and %b, %frac_mask
    %e: i64 = sub %ex2, %bias
    %nan: bool = icmp eq %ex2, %m11
    %neg: bool = icmp ne %sign, %zero64
    %big: bool = icmp sge %e, %c127
    %exact: bool = icmp eq %e, %c127
    %nofrac: bool = icmp eq %frac, %zero64
    %is_min: bool = and %exact, %nofrac
    %is_min2: bool = and %is_min, %neg
    %bad: bool = or %big, %nan
    branch %is_min2, block1, block2
block1:
    %lowest: i128 = const -170141183460469231731687303715884105728
    return %lowest
block2:
    branch %bad, block3, block4
block3:
    trap 4
    unreachable
block4:
    %r: i128 = call @ir.wide.magnitude(%v)
    %zero: i128 = const 0
    %nr: i128 = sub %zero, %r
    %res: i128 = select %neg, %nr, %r
    return %res
}
function @ir.wide.f64_to_u128_sat(f64) -> i128 internal {
block0(%v: f64):
    %b: i64 = bitcast %v
    %c52: i64 = const 52
    %c63: i64 = const 63
    %m11: i64 = const 2047
    %bias: i64 = const 1023
    %zero64: i64 = const 0
    %c128: i64 = const 128
    %frac_mask: i64 = const 4503599627370495
    %sign: i64 = lshr %b, %c63
    %ex: i64 = lshr %b, %c52
    %ex2: i64 = and %ex, %m11
    %frac: i64 = and %b, %frac_mask
    %e: i64 = sub %ex2, %bias
    %special: bool = icmp eq %ex2, %m11
    %nofrac: bool = icmp eq %frac, %zero64
    %nan: bool = xor %special, %nofrac
    %nan2: bool = and %special, %nan
    %neg: bool = icmp ne %sign, %zero64
    %big: bool = icmp sge %e, %c128
    %over: bool = or %big, %special
    branch %nan2, block1, block2
block1:
    %z1: i128 = const 0
    return %z1
block2:
    branch %neg, block1, block3
block3:
    branch %over, block4, block5
block4:
    %all: i128 = const -1
    return %all
block5:
    %r: i128 = call @ir.wide.magnitude(%v)
    return %r
}
function @ir.wide.f64_to_s128_sat(f64) -> i128 internal {
block0(%v: f64):
    %b: i64 = bitcast %v
    %c52: i64 = const 52
    %c63: i64 = const 63
    %m11: i64 = const 2047
    %bias: i64 = const 1023
    %zero64: i64 = const 0
    %c127: i64 = const 127
    %frac_mask: i64 = const 4503599627370495
    %sign: i64 = lshr %b, %c63
    %ex: i64 = lshr %b, %c52
    %ex2: i64 = and %ex, %m11
    %frac: i64 = and %b, %frac_mask
    %e: i64 = sub %ex2, %bias
    %special: bool = icmp eq %ex2, %m11
    %nofrac: bool = icmp eq %frac, %zero64
    %nan: bool = xor %special, %nofrac
    %nan2: bool = and %special, %nan
    %neg: bool = icmp ne %sign, %zero64
    %big: bool = icmp sge %e, %c127
    %over: bool = or %big, %special
    branch %nan2, block1, block2
block1:
    %z1: i128 = const 0
    return %z1
block2:
    branch %over, block3, block6
block3:
    branch %neg, block4, block5
block4:
    %lowest: i128 = const -170141183460469231731687303715884105728
    return %lowest
block5:
    %highest: i128 = const 170141183460469231731687303715884105727
    return %highest
block6:
    %r: i128 = call @ir.wide.magnitude(%v)
    %zero: i128 = const 0
    %nr: i128 = sub %zero, %r
    %res: i128 = select %neg, %nr, %r
    return %res
}
"#;

    fn is_wide(ty: Type) -> bool {
        ty == Type::I128
    }

    fn mentions(function: &Function) -> bool {
        function.signature.parameters.iter().any(|ty| is_wide(*ty))
            || function.signature.result.is_some_and(is_wide)
            || function.value_types.iter().any(|ty| is_wide(*ty))
    }

    fn unsupported(what: &str) -> Diagnostic {
        fail(
            "EI500",
            "The native code generator does not support this yet.",
        )
        .with_context(format!("Feature: 128-bit {what}"))
    }

    fn narrowed(signature: &Signature) -> Signature {
        let mut parameters = Vec::with_capacity(signature.parameters.len() + 1);
        for ty in &signature.parameters {
            if is_wide(*ty) {
                parameters.push(Type::I64);
                parameters.push(Type::I64);
            } else {
                parameters.push(*ty);
            }
        }
        let mut result = signature.result;
        if result.is_some_and(is_wide) {
            result = Some(Type::I64);
            parameters.push(Type::POINTER);
        }
        Signature {
            parameters,
            result,
            convention: signature.convention,
            variadic: signature.variadic,
        }
    }

    fn needs_float_helpers(module: &Module) -> bool {
        module.functions.iter().any(|function| {
            function.instructions.iter().any(|instruction| {
                if let Operation::Binary(
                    BinaryOp::MultiplyOverflowsSigned | BinaryOp::MultiplyOverflowsUnsigned,
                ) = &instruction.operation
                {
                    return instruction
                        .arguments
                        .iter()
                        .any(|value| is_wide(function.value_types[value.index()]));
                }
                let Operation::Convert(conversion) = &instruction.operation else {
                    return false;
                };
                let touches_wide = instruction
                    .result
                    .is_some_and(|value| is_wide(function.value_types[value.index()]))
                    || instruction
                        .arguments
                        .iter()
                        .any(|value| is_wide(function.value_types[value.index()]));
                touches_wide
                    && !matches!(
                        conversion,
                        Conversion::ZeroExtend | Conversion::SignExtend | Conversion::Truncate
                    )
            })
        })
    }

    pub fn legalize(module: &Module) -> Result<Module, Diagnostic> {
        if !module.functions.iter().any(mentions) {
            return Ok(module.clone());
        }
        let mut source = module.clone();
        let original = module.functions.len();
        let mut texts = vec![HELPERS];
        if needs_float_helpers(module) {
            texts.push(FLOAT_HELPERS);
        }
        for text in texts {
            let helpers = Module::from_text(text)?;
            let offset = source.functions.len() as u32;
            for mut helper in helpers.functions {
                for instruction in &mut helper.instructions {
                    if let Operation::Call(callee) = &mut instruction.operation {
                        *callee = FunctionId(callee.0 + offset);
                    }
                }
                helper.linkage = Linkage::Internal;
                source.functions.push(helper);
            }
        }
        let by_name = |name: &str| -> Result<FunctionId, Diagnostic> {
            source
                .functions
                .iter()
                .position(|function| function.name == name)
                .map(|at| FunctionId(at as u32))
                .ok_or_else(|| unsupported("helper functions"))
        };
        let unsigned_division = by_name("ir.wide.udivmod")?;
        let signed_division = by_name("ir.wide.sdivmod")?;
        let mut out = source.clone();
        for index in 0..source.functions.len() {
            let function = &source.functions[index];
            if !mentions(function) {
                continue;
            }
            if function.is_declaration() {
                out.functions[index].signature = narrowed(&function.signature);
                continue;
            }
            if function.signature.variadic && function.signature.result.is_some_and(is_wide) {
                return Err(unsupported("results of variadic functions"));
            }
            out.functions[index] =
                Rewriter::run(&source, function, unsigned_division, signed_division)?;
        }
        let _ = original;
        Ok(out)
    }

    struct Rewriter<'a> {
        module: &'a Module,
        function: Function,
        halves: BTreeMap<Value, (Value, Value)>,
        list: Vec<Inst>,
        out_pointer: Option<Value>,
        unsigned_division: FunctionId,
        signed_division: FunctionId,
    }

    impl Rewriter<'_> {
        fn run(
            module: &Module,
            source: &Function,
            unsigned_division: FunctionId,
            signed_division: FunctionId,
        ) -> Result<Function, Diagnostic> {
            let mut rewriter = Rewriter {
                module,
                function: source.clone(),
                halves: BTreeMap::new(),
                list: Vec::new(),
                out_pointer: None,
                unsigned_division,
                signed_division,
            };
            rewriter.function.signature = narrowed(&source.signature);
            for block in 0..rewriter.function.blocks.len() {
                let parameters = rewriter.function.blocks[block].parameters.clone();
                let mut widened = Vec::with_capacity(parameters.len() + 1);
                for parameter in parameters {
                    if is_wide(rewriter.function.type_of(parameter)) {
                        let low = rewriter.function.new_value(Type::I64);
                        let high = rewriter.function.new_value(Type::I64);
                        rewriter.halves.insert(parameter, (low, high));
                        widened.push(low);
                        widened.push(high);
                    } else {
                        widened.push(parameter);
                    }
                }
                if block == 0 && source.signature.result.is_some_and(is_wide) {
                    let pointer = rewriter.function.new_value(Type::POINTER);
                    rewriter.out_pointer = Some(pointer);
                    widened.push(pointer);
                }
                rewriter.function.blocks[block].parameters = widened;
            }
            for block in 0..rewriter.function.blocks.len() {
                let old = std::mem::take(&mut rewriter.function.blocks[block].instructions);
                rewriter.list = Vec::with_capacity(old.len());
                for inst in old {
                    let instruction = rewriter.function.instructions[inst.index()].clone();
                    rewriter.instruction(inst, instruction)?;
                }
                rewriter.terminator(block)?;
                rewriter.function.blocks[block].instructions = std::mem::take(&mut rewriter.list);
            }
            Ok(rewriter.function)
        }

        fn helper(&self, name: &str) -> Result<FunctionId, Diagnostic> {
            self.module
                .functions
                .iter()
                .position(|function| function.name == name)
                .map(|at| FunctionId(at as u32))
                .ok_or_else(|| unsupported("conversions without helper functions"))
        }

        fn wide(&self, value: Value) -> bool {
            is_wide(self.function.type_of(value))
        }

        fn half(&self, value: Value) -> Result<(Value, Value), Diagnostic> {
            self.halves
                .get(&value)
                .copied()
                .ok_or_else(|| unsupported("values that are used before they are defined"))
        }

        fn push(&mut self, operation: Operation, arguments: Vec<Value>, result: Option<Value>) {
            self.function.instructions.push(Instruction {
                operation,
                arguments,
                result,
                spot: None,
            });
            self.list
                .push(Inst(self.function.instructions.len() as u32 - 1));
        }

        fn emit(&mut self, operation: Operation, arguments: Vec<Value>, ty: Type) -> Value {
            let value = self.function.new_value(ty);
            self.push(operation, arguments, Some(value));
            value
        }

        fn int(&mut self, value: i128) -> Value {
            self.emit(Operation::IntegerConstant(value), Vec::new(), Type::I64)
        }

        fn binary(&mut self, op: BinaryOp, left: Value, right: Value) -> Value {
            self.emit(Operation::Binary(op), vec![left, right], Type::I64)
        }

        fn compare(&mut self, comparison: Comparison, left: Value, right: Value) -> Value {
            self.emit(
                Operation::Compare(comparison),
                vec![left, right],
                Type::BOOL,
            )
        }

        fn select(&mut self, condition: Value, yes: Value, no: Value, ty: Type) -> Value {
            self.emit(Operation::Select, vec![condition, yes, no], ty)
        }

        fn address(&mut self, base: Value, offset: i128) -> Value {
            if offset == 0 {
                return base;
            }
            let step = self.int(offset);
            self.emit(Operation::Offset, vec![base, step], Type::POINTER)
        }

        fn expanded(&self, values: &[Value]) -> Result<Vec<Value>, Diagnostic> {
            let mut out = Vec::with_capacity(values.len());
            for value in values {
                if self.wide(*value) {
                    let (low, high) = self.half(*value)?;
                    out.push(low);
                    out.push(high);
                } else {
                    out.push(*value);
                }
            }
            Ok(out)
        }

        fn involved(&self, instruction: &Instruction) -> bool {
            instruction.result.is_some_and(|value| self.wide(value))
                || instruction.arguments.iter().any(|value| self.wide(*value))
        }

        fn instruction(&mut self, inst: Inst, instruction: Instruction) -> Result<(), Diagnostic> {
            if !self.involved(&instruction) {
                self.list.push(inst);
                return Ok(());
            }
            let arguments = instruction.arguments.clone();
            let result = instruction.result;
            match &instruction.operation {
                Operation::IntegerConstant(value) => {
                    let low = self.int(i128::from(*value as i64));
                    let high = self.int(i128::from((*value >> 64) as i64));
                    self.bind(result, low, high)
                }
                Operation::Null => {
                    let zero = self.int(0);
                    self.bind(result, zero, zero)
                }
                Operation::Poison => {
                    let low = self.emit(Operation::Poison, Vec::new(), Type::I64);
                    let high = self.emit(Operation::Poison, Vec::new(), Type::I64);
                    self.bind(result, low, high)
                }
                Operation::Binary(op) => self.binary_operation(*op, &arguments, result),
                Operation::Unary(op) => self.unary_operation(*op, &arguments, result),
                Operation::Compare(comparison) => {
                    self.compare_operation(*comparison, &arguments, result)
                }
                Operation::Convert(conversion) => {
                    self.convert_operation(*conversion, &arguments, result)
                }
                Operation::Select => {
                    let (low_yes, high_yes) = self.half(arguments[1])?;
                    let (low_no, high_no) = self.half(arguments[2])?;
                    let low = self.select(arguments[0], low_yes, low_no, Type::I64);
                    let high = self.select(arguments[0], high_yes, high_no, Type::I64);
                    self.bind(result, low, high)
                }
                Operation::Load {
                    align,
                    volatile,
                    ordering: None,
                } => {
                    let align = (*align).min(8);
                    let operation = Operation::Load {
                        align,
                        volatile: *volatile,
                        ordering: None,
                    };
                    let low = self.emit(operation.clone(), vec![arguments[0]], Type::I64);
                    let upper = self.address(arguments[0], 8);
                    let high = self.emit(operation, vec![upper], Type::I64);
                    self.bind(result, low, high)
                }
                Operation::Store {
                    align,
                    volatile,
                    ordering: None,
                } if self.wide(arguments[0]) => {
                    let (low, high) = self.half(arguments[0])?;
                    let operation = Operation::Store {
                        align: (*align).min(8),
                        volatile: *volatile,
                        ordering: None,
                    };
                    self.push(operation.clone(), vec![low, arguments[1]], None);
                    let upper = self.address(arguments[1], 8);
                    self.push(operation, vec![high, upper], None);
                    Ok(())
                }
                Operation::Call(callee) => {
                    let signature = self.module.functions[callee.index()].signature.clone();
                    self.call(Operation::Call(*callee), &signature, &arguments, result)
                }
                Operation::CallIndirect(signature) => {
                    let signature = signature.clone();
                    let narrow = narrowed(&signature);
                    self.call_with(
                        Operation::CallIndirect(narrow),
                        &signature,
                        &arguments,
                        result,
                        1,
                    )
                }
                _ => Err(unsupported("operations beyond arithmetic")),
            }
        }

        fn bind(
            &mut self,
            result: Option<Value>,
            low: Value,
            high: Value,
        ) -> Result<(), Diagnostic> {
            if let Some(result) = result {
                self.halves.insert(result, (low, high));
            }
            Ok(())
        }

        fn call(
            &mut self,
            operation: Operation,
            signature: &Signature,
            arguments: &[Value],
            result: Option<Value>,
        ) -> Result<(), Diagnostic> {
            self.call_with(operation, signature, arguments, result, 0)
        }

        fn call_with(
            &mut self,
            operation: Operation,
            signature: &Signature,
            arguments: &[Value],
            result: Option<Value>,
            leading: usize,
        ) -> Result<(), Diagnostic> {
            let mut passed: Vec<Value> = arguments[..leading].to_vec();
            passed.extend(self.expanded(&arguments[leading..])?);
            let wide_result = signature.result.is_some_and(is_wide);
            let slot = if wide_result {
                let slot = self.emit(
                    Operation::StackSlot { size: 8, align: 8 },
                    Vec::new(),
                    Type::POINTER,
                );
                passed.push(slot);
                Some(slot)
            } else {
                None
            };
            if wide_result {
                let low = self.function.new_value(Type::I64);
                self.push(operation, passed, Some(low));
                let high = self.emit(
                    Operation::Load {
                        align: 8,
                        volatile: false,
                        ordering: None,
                    },
                    vec![slot.unwrap_or(low)],
                    Type::I64,
                );
                self.bind(result, low, high)
            } else {
                self.push(operation, passed, result);
                Ok(())
            }
        }

        fn compare_operation(
            &mut self,
            comparison: Comparison,
            arguments: &[Value],
            result: Option<Value>,
        ) -> Result<(), Diagnostic> {
            let left = self.half(arguments[0])?;
            let right = self.half(arguments[1])?;
            let Some(result) = result else {
                return Ok(());
            };
            self.compare_into(comparison, left, right, result);
            Ok(())
        }

        fn compare_value(
            &mut self,
            comparison: Comparison,
            left: (Value, Value),
            right: (Value, Value),
        ) -> Value {
            let result = self.function.new_value(Type::BOOL);
            self.compare_into(comparison, left, right, result);
            result
        }

        fn compare_into(
            &mut self,
            comparison: Comparison,
            left: (Value, Value),
            right: (Value, Value),
            result: Value,
        ) {
            let (left_low, left_high) = left;
            let (right_low, right_high) = right;
            match comparison {
                Comparison::Equal | Comparison::NotEqual => {
                    let low = self.binary(BinaryOp::Xor, left_low, right_low);
                    let high = self.binary(BinaryOp::Xor, left_high, right_high);
                    let both = self.binary(BinaryOp::Or, low, high);
                    let zero = self.int(0);
                    self.push(
                        Operation::Compare(comparison),
                        vec![both, zero],
                        Some(result),
                    );
                }
                _ => {
                    let (upper, lower) = match comparison {
                        Comparison::LessSigned => {
                            (Comparison::LessSigned, Comparison::LessUnsigned)
                        }
                        Comparison::LessEqualSigned => {
                            (Comparison::LessSigned, Comparison::LessEqualUnsigned)
                        }
                        Comparison::GreaterSigned => {
                            (Comparison::GreaterSigned, Comparison::GreaterUnsigned)
                        }
                        Comparison::GreaterEqualSigned => {
                            (Comparison::GreaterSigned, Comparison::GreaterEqualUnsigned)
                        }
                        Comparison::LessUnsigned => {
                            (Comparison::LessUnsigned, Comparison::LessUnsigned)
                        }
                        Comparison::LessEqualUnsigned => {
                            (Comparison::LessUnsigned, Comparison::LessEqualUnsigned)
                        }
                        Comparison::GreaterUnsigned => {
                            (Comparison::GreaterUnsigned, Comparison::GreaterUnsigned)
                        }
                        _ => (
                            Comparison::GreaterUnsigned,
                            Comparison::GreaterEqualUnsigned,
                        ),
                    };
                    let decided = self.compare(upper, left_high, right_high);
                    let same = self.compare(Comparison::Equal, left_high, right_high);
                    let tie = self.compare(lower, left_low, right_low);
                    self.push(Operation::Select, vec![same, tie, decided], Some(result));
                }
            }
        }

        fn convert_operation(
            &mut self,
            conversion: Conversion,
            arguments: &[Value],
            result: Option<Value>,
        ) -> Result<(), Diagnostic> {
            let source = arguments[0];
            match conversion {
                Conversion::ZeroExtend | Conversion::SignExtend if !self.wide(source) => {
                    let narrow = self.function.type_of(source);
                    let low = if narrow == Type::I64 {
                        source
                    } else {
                        self.emit(Operation::Convert(conversion), vec![source], Type::I64)
                    };
                    let high = if conversion == Conversion::ZeroExtend {
                        self.int(0)
                    } else {
                        let shift = self.int(63);
                        self.binary(BinaryOp::ShiftRightArithmetic, low, shift)
                    };
                    self.bind(result, low, high)
                }
                Conversion::SignedToFloat | Conversion::UnsignedToFloat if self.wide(source) => {
                    let (low, high) = self.half(source)?;
                    let Some(result) = result else {
                        return Ok(());
                    };
                    let single = self.function.type_of(result) == Type::F32;
                    let name = match (conversion == Conversion::SignedToFloat, single) {
                        (true, false) => "ir.wide.s128_to_f64",
                        (true, true) => "ir.wide.s128_to_f32",
                        (false, false) => "ir.wide.u128_to_f64",
                        (false, true) => "ir.wide.u128_to_f32",
                    };
                    let callee = self.helper(name)?;
                    self.push(Operation::Call(callee), vec![low, high], Some(result));
                    Ok(())
                }
                Conversion::FloatToSigned
                | Conversion::FloatToUnsigned
                | Conversion::FloatToSignedSaturating
                | Conversion::FloatToUnsignedSaturating
                    if result.is_some_and(|value| self.wide(value)) =>
                {
                    let name = match conversion {
                        Conversion::FloatToSigned => "ir.wide.f64_to_s128",
                        Conversion::FloatToUnsigned => "ir.wide.f64_to_u128",
                        Conversion::FloatToSignedSaturating => "ir.wide.f64_to_s128_sat",
                        _ => "ir.wide.f64_to_u128_sat",
                    };
                    let callee = self.helper(name)?;
                    let operand = if self.function.type_of(source) == Type::F32 {
                        self.emit(
                            Operation::Convert(Conversion::FloatExtend),
                            vec![source],
                            Type::F64,
                        )
                    } else {
                        source
                    };
                    let signature = Signature::new(vec![Type::F64], Some(Type::I128));
                    self.call_with(Operation::Call(callee), &signature, &[operand], result, 0)
                }
                Conversion::Truncate if self.wide(source) => {
                    let (low, _) = self.half(source)?;
                    let Some(result) = result else {
                        return Ok(());
                    };
                    if self.function.type_of(result) == Type::I64 {
                        let zero = self.int(0);
                        self.push(
                            Operation::Binary(BinaryOp::Or),
                            vec![low, zero],
                            Some(result),
                        );
                    } else {
                        self.push(
                            Operation::Convert(Conversion::Truncate),
                            vec![low],
                            Some(result),
                        );
                    }
                    Ok(())
                }
                _ => Err(unsupported("conversions")),
            }
        }

        fn unary(&mut self, op: UnaryOp, value: Value) -> Value {
            self.emit(Operation::Unary(op), vec![value], Type::I64)
        }

        fn unary_operation(
            &mut self,
            op: UnaryOp,
            arguments: &[Value],
            result: Option<Value>,
        ) -> Result<(), Diagnostic> {
            let (low, high) = self.half(arguments[0])?;
            let zero = self.int(0);
            let (new_low, new_high) = match op {
                UnaryOp::Not => {
                    let all = self.int(-1);
                    (
                        self.binary(BinaryOp::Xor, low, all),
                        self.binary(BinaryOp::Xor, high, all),
                    )
                }
                UnaryOp::Negate => self.negated(low, high),
                UnaryOp::AbsoluteSigned => {
                    let (flipped_low, flipped_high) = self.negated(low, high);
                    let negative = self.compare(Comparison::LessSigned, high, zero);
                    (
                        self.select(negative, flipped_low, low, Type::I64),
                        self.select(negative, flipped_high, high, Type::I64),
                    )
                }
                UnaryOp::PopulationCount => {
                    let first = self.unary(UnaryOp::PopulationCount, low);
                    let second = self.unary(UnaryOp::PopulationCount, high);
                    (self.binary(BinaryOp::Add, first, second), zero)
                }
                UnaryOp::LeadingZeros => {
                    let upper = self.unary(UnaryOp::LeadingZeros, high);
                    let lower = self.unary(UnaryOp::LeadingZeros, low);
                    let sixty_four = self.int(64);
                    let beyond = self.binary(BinaryOp::Add, lower, sixty_four);
                    let empty = self.compare(Comparison::Equal, high, zero);
                    (self.select(empty, beyond, upper, Type::I64), zero)
                }
                UnaryOp::TrailingZeros => {
                    let upper = self.unary(UnaryOp::TrailingZeros, high);
                    let lower = self.unary(UnaryOp::TrailingZeros, low);
                    let sixty_four = self.int(64);
                    let beyond = self.binary(BinaryOp::Add, upper, sixty_four);
                    let empty = self.compare(Comparison::Equal, low, zero);
                    (self.select(empty, beyond, lower, Type::I64), zero)
                }
                UnaryOp::ByteSwap | UnaryOp::BitReverse => {
                    (self.unary(op, high), self.unary(op, low))
                }
                _ => return Err(unsupported("unary operations")),
            };
            self.bind(result, new_low, new_high)
        }

        fn negated(&mut self, low: Value, high: Value) -> (Value, Value) {
            let zero = self.int(0);
            let new_low = self.binary(BinaryOp::Subtract, zero, low);
            let borrow = self.compare(Comparison::NotEqual, low, zero);
            let borrowed = self.emit(
                Operation::Convert(Conversion::ZeroExtend),
                vec![borrow],
                Type::I64,
            );
            let inner = self.binary(BinaryOp::Subtract, zero, high);
            (new_low, self.binary(BinaryOp::Subtract, inner, borrowed))
        }

        fn binary_operation(
            &mut self,
            op: BinaryOp,
            arguments: &[Value],
            result: Option<Value>,
        ) -> Result<(), Diagnostic> {
            let (left_low, left_high) = self.half(arguments[0])?;
            let (right_low, right_high) = self.half(arguments[1])?;
            let left = (left_low, left_high);
            let right = (right_low, right_high);
            match op {
                BinaryOp::AddOverflowsUnsigned => {
                    let Some(result) = result else { return Ok(()) };
                    let sum = self.add_halves(left, right);
                    self.compare_into(Comparison::LessUnsigned, sum, left, result);
                    return Ok(());
                }
                BinaryOp::SubtractOverflowsUnsigned => {
                    let Some(result) = result else { return Ok(()) };
                    self.compare_into(Comparison::LessUnsigned, left, right, result);
                    return Ok(());
                }
                BinaryOp::AddOverflowsSigned | BinaryOp::SubtractOverflowsSigned => {
                    let Some(result) = result else { return Ok(()) };
                    let flag =
                        self.signed_overflow(op == BinaryOp::AddOverflowsSigned, left, right);
                    let zero = self.int(0);
                    self.push(
                        Operation::Compare(Comparison::LessSigned),
                        vec![flag, zero],
                        Some(result),
                    );
                    return Ok(());
                }
                BinaryOp::MultiplyOverflowsSigned | BinaryOp::MultiplyOverflowsUnsigned => {
                    let Some(result) = result else { return Ok(()) };
                    let name = if op == BinaryOp::MultiplyOverflowsSigned {
                        "ir.wide.smul_overflows"
                    } else {
                        "ir.wide.umul_overflows"
                    };
                    let callee = self.helper(name)?;
                    self.push(
                        Operation::Call(callee),
                        vec![left_low, left_high, right_low, right_high],
                        Some(result),
                    );
                    return Ok(());
                }
                _ => {}
            }
            let (low, high) = match op {
                BinaryOp::AddSaturatingUnsigned => {
                    let sum = self.add_halves(left, right);
                    let overflow = self.compare_value(Comparison::LessUnsigned, sum, left);
                    let all = self.int(-1);
                    (
                        self.select(overflow, all, sum.0, Type::I64),
                        self.select(overflow, all, sum.1, Type::I64),
                    )
                }
                BinaryOp::SubtractSaturatingUnsigned => {
                    let difference = self.sub_halves(left, right);
                    let overflow = self.compare_value(Comparison::LessUnsigned, left, right);
                    let zero = self.int(0);
                    (
                        self.select(overflow, zero, difference.0, Type::I64),
                        self.select(overflow, zero, difference.1, Type::I64),
                    )
                }
                BinaryOp::AddSaturatingSigned | BinaryOp::SubtractSaturatingSigned => {
                    let adding = op == BinaryOp::AddSaturatingSigned;
                    let value = if adding {
                        self.add_halves(left, right)
                    } else {
                        self.sub_halves(left, right)
                    };
                    let flag = self.signed_overflow(adding, left, right);
                    let zero = self.int(0);
                    let overflow = self.compare(Comparison::LessSigned, flag, zero);
                    let negative = self.compare(Comparison::LessSigned, left_high, zero);
                    let all = self.int(-1);
                    let top = self.int(i128::from(i64::MIN));
                    let max_high = self.int(i128::from(i64::MAX));
                    let sat_low = self.select(negative, zero, all, Type::I64);
                    let sat_high = self.select(negative, top, max_high, Type::I64);
                    (
                        self.select(overflow, sat_low, value.0, Type::I64),
                        self.select(overflow, sat_high, value.1, Type::I64),
                    )
                }
                BinaryOp::And | BinaryOp::Or | BinaryOp::Xor => (
                    self.binary(op, left_low, right_low),
                    self.binary(op, left_high, right_high),
                ),
                BinaryOp::Add => {
                    let low = self.binary(BinaryOp::Add, left_low, right_low);
                    let carry = self.compare(Comparison::LessUnsigned, low, left_low);
                    let carried = self.emit(
                        Operation::Convert(Conversion::ZeroExtend),
                        vec![carry],
                        Type::I64,
                    );
                    let sum = self.binary(BinaryOp::Add, left_high, right_high);
                    (low, self.binary(BinaryOp::Add, sum, carried))
                }
                BinaryOp::Subtract => {
                    let low = self.binary(BinaryOp::Subtract, left_low, right_low);
                    let borrow = self.compare(Comparison::LessUnsigned, left_low, right_low);
                    let borrowed = self.emit(
                        Operation::Convert(Conversion::ZeroExtend),
                        vec![borrow],
                        Type::I64,
                    );
                    let difference = self.binary(BinaryOp::Subtract, left_high, right_high);
                    (low, self.binary(BinaryOp::Subtract, difference, borrowed))
                }
                BinaryOp::Multiply => {
                    let low = self.binary(BinaryOp::Multiply, left_low, right_low);
                    let carry = self.binary(BinaryOp::MultiplyHighUnsigned, left_low, right_low);
                    let cross_left = self.binary(BinaryOp::Multiply, left_low, right_high);
                    let cross_right = self.binary(BinaryOp::Multiply, left_high, right_low);
                    let partial = self.binary(BinaryOp::Add, carry, cross_left);
                    (low, self.binary(BinaryOp::Add, partial, cross_right))
                }
                BinaryOp::ShiftLeft
                | BinaryOp::ShiftRightLogical
                | BinaryOp::ShiftRightArithmetic => self.shift(op, left_low, left_high, right_low),
                BinaryOp::RotateLeft | BinaryOp::RotateRight => {
                    let (forward, backward) = if op == BinaryOp::RotateLeft {
                        (BinaryOp::ShiftLeft, BinaryOp::ShiftRightLogical)
                    } else {
                        (BinaryOp::ShiftRightLogical, BinaryOp::ShiftLeft)
                    };
                    let (front_low, front_high) =
                        self.shift(forward, left_low, left_high, right_low);
                    let total = self.int(128);
                    let complement = self.binary(BinaryOp::Subtract, total, right_low);
                    let (back_low, back_high) =
                        self.shift(backward, left_low, left_high, complement);
                    (
                        self.binary(BinaryOp::Or, front_low, back_low),
                        self.binary(BinaryOp::Or, front_high, back_high),
                    )
                }
                BinaryOp::MinimumSigned
                | BinaryOp::MaximumSigned
                | BinaryOp::MinimumUnsigned
                | BinaryOp::MaximumUnsigned => {
                    let comparison = match op {
                        BinaryOp::MinimumSigned => Comparison::LessEqualSigned,
                        BinaryOp::MaximumSigned => Comparison::GreaterEqualSigned,
                        BinaryOp::MinimumUnsigned => Comparison::LessEqualUnsigned,
                        _ => Comparison::GreaterEqualUnsigned,
                    };
                    let left_first = self.compare_value(
                        comparison,
                        (left_low, left_high),
                        (right_low, right_high),
                    );
                    (
                        self.select(left_first, left_low, right_low, Type::I64),
                        self.select(left_first, left_high, right_high, Type::I64),
                    )
                }
                BinaryOp::DivideUnsigned
                | BinaryOp::DivideSigned
                | BinaryOp::RemainderUnsigned
                | BinaryOp::RemainderSigned => {
                    let slot = self.emit(
                        Operation::StackSlot { size: 32, align: 8 },
                        Vec::new(),
                        Type::POINTER,
                    );
                    let callee = if matches!(op, BinaryOp::DivideSigned | BinaryOp::RemainderSigned)
                    {
                        self.signed_division
                    } else {
                        self.unsigned_division
                    };
                    self.push(
                        Operation::Call(callee),
                        vec![left_low, left_high, right_low, right_high, slot],
                        None,
                    );
                    let base = if matches!(op, BinaryOp::DivideUnsigned | BinaryOp::DivideSigned) {
                        0
                    } else {
                        16
                    };
                    let load = Operation::Load {
                        align: 8,
                        volatile: false,
                        ordering: None,
                    };
                    let low_at = self.address(slot, base);
                    let high_at = self.address(slot, base + 8);
                    let low = self.emit(load.clone(), vec![low_at], Type::I64);
                    let high = self.emit(load, vec![high_at], Type::I64);
                    (low, high)
                }
                _ => return Err(unsupported("binary operations")),
            };
            self.bind(result, low, high)
        }

        fn add_halves(&mut self, left: (Value, Value), right: (Value, Value)) -> (Value, Value) {
            let low = self.binary(BinaryOp::Add, left.0, right.0);
            let carry = self.compare(Comparison::LessUnsigned, low, left.0);
            let carried = self.emit(
                Operation::Convert(Conversion::ZeroExtend),
                vec![carry],
                Type::I64,
            );
            let sum = self.binary(BinaryOp::Add, left.1, right.1);
            (low, self.binary(BinaryOp::Add, sum, carried))
        }

        fn sub_halves(&mut self, left: (Value, Value), right: (Value, Value)) -> (Value, Value) {
            let low = self.binary(BinaryOp::Subtract, left.0, right.0);
            let borrow = self.compare(Comparison::LessUnsigned, left.0, right.0);
            let borrowed = self.emit(
                Operation::Convert(Conversion::ZeroExtend),
                vec![borrow],
                Type::I64,
            );
            let difference = self.binary(BinaryOp::Subtract, left.1, right.1);
            (low, self.binary(BinaryOp::Subtract, difference, borrowed))
        }

        fn signed_overflow(
            &mut self,
            adding: bool,
            left: (Value, Value),
            right: (Value, Value),
        ) -> Value {
            let result = if adding {
                self.add_halves(left, right)
            } else {
                self.sub_halves(left, right)
            };
            let (first, second) = if adding {
                (
                    self.binary(BinaryOp::Xor, left.1, result.1),
                    self.binary(BinaryOp::Xor, right.1, result.1),
                )
            } else {
                (
                    self.binary(BinaryOp::Xor, left.1, right.1),
                    self.binary(BinaryOp::Xor, left.1, result.1),
                )
            };
            self.binary(BinaryOp::And, first, second)
        }

        fn shift(
            &mut self,
            op: BinaryOp,
            low: Value,
            high: Value,
            amount: Value,
        ) -> (Value, Value) {
            let zero = self.int(0);
            let mask = self.int(127);
            let n = self.binary(BinaryOp::And, amount, mask);
            let sixty_three = self.int(63);
            let m = self.binary(BinaryOp::And, n, sixty_three);
            let sixty_four = self.int(64);
            let big_bit = self.binary(BinaryOp::And, n, sixty_four);
            let big = self.compare(Comparison::NotEqual, big_bit, zero);
            let inverse = self.binary(BinaryOp::Subtract, sixty_four, m);
            let none = self.compare(Comparison::Equal, m, zero);
            match op {
                BinaryOp::ShiftLeft => {
                    let spilled = self.binary(BinaryOp::ShiftRightLogical, low, inverse);
                    let spill = self.select(none, zero, spilled, Type::I64);
                    let moved = self.binary(BinaryOp::ShiftLeft, high, m);
                    let small_high = self.binary(BinaryOp::Or, moved, spill);
                    let small_low = self.binary(BinaryOp::ShiftLeft, low, m);
                    let big_high = self.binary(BinaryOp::ShiftLeft, low, m);
                    (
                        self.select(big, zero, small_low, Type::I64),
                        self.select(big, big_high, small_high, Type::I64),
                    )
                }
                BinaryOp::ShiftRightLogical => {
                    let spilled = self.binary(BinaryOp::ShiftLeft, high, inverse);
                    let spill = self.select(none, zero, spilled, Type::I64);
                    let moved = self.binary(BinaryOp::ShiftRightLogical, low, m);
                    let small_low = self.binary(BinaryOp::Or, moved, spill);
                    let small_high = self.binary(BinaryOp::ShiftRightLogical, high, m);
                    let big_low = self.binary(BinaryOp::ShiftRightLogical, high, m);
                    (
                        self.select(big, big_low, small_low, Type::I64),
                        self.select(big, zero, small_high, Type::I64),
                    )
                }
                _ => {
                    let spilled = self.binary(BinaryOp::ShiftLeft, high, inverse);
                    let spill = self.select(none, zero, spilled, Type::I64);
                    let moved = self.binary(BinaryOp::ShiftRightLogical, low, m);
                    let small_low = self.binary(BinaryOp::Or, moved, spill);
                    let small_high = self.binary(BinaryOp::ShiftRightArithmetic, high, m);
                    let big_low = self.binary(BinaryOp::ShiftRightArithmetic, high, m);
                    let big_high = self.binary(BinaryOp::ShiftRightArithmetic, high, sixty_three);
                    (
                        self.select(big, big_low, small_low, Type::I64),
                        self.select(big, big_high, small_high, Type::I64),
                    )
                }
            }
        }

        fn terminator(&mut self, block: usize) -> Result<(), Diagnostic> {
            let mut terminator = self.function.blocks[block].terminator.clone();
            match &mut terminator {
                Terminator::Jump(call) => call.arguments = self.expanded(&call.arguments)?,
                Terminator::Branch {
                    taken, otherwise, ..
                } => {
                    taken.arguments = self.expanded(&taken.arguments)?;
                    otherwise.arguments = self.expanded(&otherwise.arguments)?;
                }
                Terminator::Switch {
                    value,
                    cases,
                    default,
                } => {
                    if self.wide(*value) {
                        return Err(unsupported("switches"));
                    }
                    for (_, call) in cases.iter_mut() {
                        call.arguments = self.expanded(&call.arguments)?;
                    }
                    default.arguments = self.expanded(&default.arguments)?;
                }
                Terminator::Return(Some(value)) if self.wide(*value) => {
                    let (low, high) = self.half(*value)?;
                    let pointer = self
                        .out_pointer
                        .ok_or_else(|| unsupported("returns without a result slot"))?;
                    self.push(
                        Operation::Store {
                            align: 8,
                            volatile: false,
                            ordering: None,
                        },
                        vec![high, pointer],
                        None,
                    );
                    terminator = Terminator::Return(Some(low));
                }
                Terminator::Invoke { .. } => return Err(unsupported("exception edges")),
                Terminator::Throw(value) if self.wide(*value) => {
                    return Err(unsupported("thrown values"))
                }
                _ => {}
            }
            self.function.blocks[block].terminator = terminator;
            Ok(())
        }
    }
}
