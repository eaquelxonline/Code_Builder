use crate::diag::{Diagnostic, Location, Severity};
use crate::FailureClass;
use std::collections::BTreeSet;

pub const ORIGIN: &str = "web.typescript";

fn error(code: &str, sentence: &str) -> Diagnostic {
    Diagnostic::new(
        code,
        Severity::Error,
        FailureClass::UserError,
        ORIGIN,
        sentence,
    )
}

#[derive(Clone, Debug, PartialEq)]
pub struct Fault {
    pub at: usize,
    pub message: String,
    pub unsupported: bool,
}

type Step<T> = Result<T, Fault>;

fn fault<T>(at: usize, message: &str) -> Step<T> {
    Err(Fault {
        at,
        message: message.to_string(),
        unsupported: false,
    })
}

fn unsupported<T>(at: usize, message: &str) -> Step<T> {
    Err(Fault {
        at,
        message: message.to_string(),
        unsupported: true,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Name,
    Private,
    Num,
    BigInt,
    Str,
    Template,
    Punct,
    End,
}

#[derive(Clone, Debug)]
struct Tok {
    kind: Kind,
    start: usize,
    end: usize,
    nl: bool,
    text: String,
}

const PUNCTUATORS: &[&str] = &[
    ">>>=", "...", "===", "!==", "**=", "<<=", ">>=", ">>>", "&&=", "||=", "??=", "=>", "==", "!=",
    "<=", ">=", "&&", "||", "??", "?.", "++", "--", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=",
    "<<", ">>", "**", "{", "}", "(", ")", "[", "]", ";", ",", "<", ">", "+", "-", "*", "/", "%",
    "&", "|", "^", "!", "~", "?", ":", "=", ".", "@", "#",
];

fn id_start(character: char) -> bool {
    character.is_ascii_alphabetic()
        || character == '$'
        || character == '_'
        || (!character.is_ascii() && character.is_alphabetic())
}

fn id_part(character: char) -> bool {
    id_start(character)
        || character.is_ascii_digit()
        || character == '\u{200c}'
        || character == '\u{200d}'
        || (!character.is_ascii() && character.is_alphanumeric())
}

fn is_line_break(character: char) -> bool {
    matches!(character, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

fn is_blank(character: char) -> bool {
    character.is_whitespace() || character == '\u{feff}'
}

struct Source {
    chars: Vec<char>,
}

impl Source {
    fn at(&self, index: usize) -> Option<char> {
        self.chars.get(index).copied()
    }

    fn lex(&self, from: usize) -> Step<Tok> {
        let mut at = from;
        let mut nl = false;
        if at == 0 && self.at(0) == Some('#') && self.at(1) == Some('!') {
            while self.at(at).is_some_and(|c| !is_line_break(c)) {
                at += 1;
            }
        }
        loop {
            match self.at(at) {
                Some(c) if is_line_break(c) => {
                    nl = true;
                    at += 1;
                }
                Some(c) if is_blank(c) => at += 1,
                Some('/') if self.at(at + 1) == Some('/') => {
                    while self.at(at).is_some_and(|c| !is_line_break(c)) {
                        at += 1;
                    }
                }
                Some('/') if self.at(at + 1) == Some('*') => {
                    let begin = at;
                    at += 2;
                    loop {
                        match self.at(at) {
                            None => return fault(begin, "A comment is not closed."),
                            Some('*') if self.at(at + 1) == Some('/') => {
                                at += 2;
                                break;
                            }
                            Some(c) => {
                                if is_line_break(c) {
                                    nl = true;
                                }
                                at += 1;
                            }
                        }
                    }
                }
                _ => break,
            }
        }
        let start = at;
        let Some(first) = self.at(at) else {
            return Ok(Tok {
                kind: Kind::End,
                start,
                end: start,
                nl,
                text: String::new(),
            });
        };
        let text_of = |end: usize| self.chars[start..end].iter().collect::<String>();
        if id_start(first) || first == '\\' {
            at = self.scan_name(at)?;
            return Ok(Tok {
                kind: Kind::Name,
                start,
                end: at,
                nl,
                text: text_of(at),
            });
        }
        if first == '#' && self.at(at + 1).is_some_and(|c| id_start(c) || c == '\\') {
            at = self.scan_name(at + 1)?;
            return Ok(Tok {
                kind: Kind::Private,
                start,
                end: at,
                nl,
                text: text_of(at),
            });
        }
        if first.is_ascii_digit()
            || (first == '.' && self.at(at + 1).is_some_and(|c| c.is_ascii_digit()))
        {
            let (end, big) = self.scan_number(at)?;
            return Ok(Tok {
                kind: if big { Kind::BigInt } else { Kind::Num },
                start,
                end,
                nl,
                text: text_of(end),
            });
        }
        if first == '"' || first == '\'' {
            at += 1;
            loop {
                match self.at(at) {
                    None => return fault(start, "A string is not closed."),
                    Some(c) if c == first => {
                        at += 1;
                        break;
                    }
                    Some('\\') => {
                        at += 1;
                        if self.at(at) == Some('\r') && self.at(at + 1) == Some('\n') {
                            at += 1;
                        }
                        at += 1;
                    }
                    Some(c) if c == '\n' || c == '\r' => {
                        return fault(start, "A string is not closed.");
                    }
                    Some(_) => at += 1,
                }
            }
            return Ok(Tok {
                kind: Kind::Str,
                start,
                end: at,
                nl,
                text: text_of(at),
            });
        }
        if first == '`' {
            return Ok(Tok {
                kind: Kind::Template,
                start,
                end: at + 1,
                nl,
                text: "`".to_string(),
            });
        }
        for punct in PUNCTUATORS {
            let width = punct.chars().count();
            if self.chars[at..].len() >= width
                && self.chars[at..at + width].iter().copied().eq(punct.chars())
            {
                if *punct == "?." && self.at(at + 2).is_some_and(|c| c.is_ascii_digit()) {
                    continue;
                }
                return Ok(Tok {
                    kind: Kind::Punct,
                    start,
                    end: at + width,
                    nl,
                    text: (*punct).to_string(),
                });
            }
        }
        fault(start, "A character is not part of TypeScript.")
    }

    fn scan_name(&self, mut at: usize) -> Step<usize> {
        if self.at(at) == Some('\\') && self.at(at + 1) != Some('u') {
            return fault(at, "A backslash escapes nothing here.");
        }
        loop {
            match self.at(at) {
                Some(c) if id_part(c) => at += 1,
                Some('\\') if self.at(at + 1) == Some('u') => {
                    at += 2;
                    if self.at(at) == Some('{') {
                        while self.at(at).is_some_and(|c| c != '}') {
                            at += 1;
                        }
                        at += 1;
                    } else {
                        at += 4;
                    }
                }
                _ => return Ok(at),
            }
        }
    }

    fn scan_number(&self, from: usize) -> Step<(usize, bool)> {
        let mut at = from;
        let digit = |c: char| c.is_ascii_alphanumeric() || c == '_';
        if self.at(at) == Some('0')
            && matches!(self.at(at + 1), Some('x' | 'X' | 'o' | 'O' | 'b' | 'B'))
        {
            at += 2;
            while self.at(at).is_some_and(digit) {
                at += 1;
            }
        } else {
            while self.at(at).is_some_and(|c| c.is_ascii_digit() || c == '_') {
                at += 1;
            }
            if self.at(at) == Some('.') {
                at += 1;
                while self.at(at).is_some_and(|c| c.is_ascii_digit() || c == '_') {
                    at += 1;
                }
            }
            if matches!(self.at(at), Some('e' | 'E')) {
                let mut look = at + 1;
                if matches!(self.at(look), Some('+' | '-')) {
                    look += 1;
                }
                if self.at(look).is_some_and(|c| c.is_ascii_digit()) {
                    at = look;
                    while self.at(at).is_some_and(|c| c.is_ascii_digit() || c == '_') {
                        at += 1;
                    }
                }
            }
        }
        let big = self.at(at) == Some('n');
        if big {
            at += 1;
        }
        Ok((at, big))
    }

    fn scan_regex(&self, from: usize) -> Step<usize> {
        let mut at = from + 1;
        let mut in_class = false;
        loop {
            match self.at(at) {
                None => return fault(from, "A regular expression is not closed."),
                Some(c) if is_line_break(c) => {
                    return fault(from, "A regular expression is not closed.")
                }
                Some('\\') => at += 2,
                Some('[') => {
                    in_class = true;
                    at += 1;
                }
                Some(']') => {
                    in_class = false;
                    at += 1;
                }
                Some('/') if !in_class => {
                    at += 1;
                    break;
                }
                Some(_) => at += 1,
            }
        }
        while self.at(at).is_some_and(id_part) {
            at += 1;
        }
        Ok(at)
    }
}

#[derive(Clone, Debug)]
struct Edit {
    start: usize,
    end: usize,
    text: Option<String>,
    pad: bool,
}

#[derive(Clone, Debug)]
struct ImportStatement {
    start: usize,
    end: usize,
    clause: (usize, usize),
    default: Option<String>,
    namespace: Option<String>,
    named: Vec<(String, String)>,
    braces: bool,
    dropped_types: bool,
}

#[derive(Clone, Debug)]
struct AliasImport {
    start: usize,
    import_start: usize,
    import_end: usize,
    end: usize,
    name: String,
    is_require: bool,
    exported: bool,
}

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub keep_const_enums: bool,
    pub jsx: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    pub code: String,
    pub erased: usize,
}

struct Mark {
    tok: Tok,
    prev_end: usize,
    edits: usize,
    imports: usize,
    used: usize,
}

#[derive(Clone, Copy, Default)]
struct Context {
    is_async: bool,
    is_generator: bool,
    in_function: bool,
    no_in: bool,
}

struct Parser {
    source: Source,
    tok: Tok,
    prev_end: usize,
    edits: Vec<Edit>,
    used: Vec<String>,
    imports: Vec<ImportStatement>,
    type_names: BTreeSet<String>,
    declared: BTreeSet<String>,
    top_values: BTreeSet<String>,
    context: Context,
    saw_module_syntax: bool,
    kept_module_syntax: bool,
    capture: Option<Vec<(usize, usize, String)>>,
    depth: usize,
    in_extends: bool,
    values: usize,
    names: Vec<String>,
    namespace: Option<String>,
    emitted: BTreeSet<String>,
    scopes: Vec<usize>,
    scope_counter: usize,
    constructor_super: Option<usize>,
    jsx: bool,
    exports: Vec<ExportList>,
    aliases: Vec<AliasImport>,
}

const RESERVED: &[&str] = &[
    "break",
    "case",
    "catch",
    "class",
    "const",
    "continue",
    "debugger",
    "default",
    "delete",
    "do",
    "else",
    "enum",
    "export",
    "extends",
    "false",
    "finally",
    "for",
    "function",
    "if",
    "import",
    "in",
    "instanceof",
    "new",
    "null",
    "return",
    "super",
    "switch",
    "this",
    "throw",
    "true",
    "try",
    "typeof",
    "var",
    "void",
    "while",
    "with",
];

const ASSIGNMENT: &[&str] = &[
    "=", "+=", "-=", "*=", "/=", "%=", "**=", "<<=", ">>=", ">>>=", "&=", "|=", "^=", "&&=", "||=",
    "??=",
];

fn is_reserved(word: &str) -> bool {
    RESERVED.contains(&word)
}

fn binary_precedence(token: &Tok, no_in: bool) -> Option<(u8, bool)> {
    match token.kind {
        Kind::Punct => Some(match token.text.as_str() {
            "??" => (1, false),
            "||" => (2, false),
            "&&" => (3, false),
            "|" => (4, false),
            "^" => (5, false),
            "&" => (6, false),
            "==" | "!=" | "===" | "!==" => (7, false),
            "<" | ">" | "<=" | ">=" => (8, false),
            "<<" | ">>" | ">>>" => (9, false),
            "+" | "-" => (10, false),
            "*" | "/" | "%" => (11, false),
            "**" => (12, true),
            _ => return None,
        }),
        Kind::Name => match token.text.as_str() {
            "instanceof" => Some((8, false)),
            "in" if !no_in => Some((8, false)),
            _ => None,
        },
        _ => None,
    }
}

impl Parser {
    fn new(text: &str) -> Step<Parser> {
        let source = Source {
            chars: text.chars().collect(),
        };
        let tok = source.lex(0)?;
        Ok(Parser {
            source,
            tok,
            prev_end: 0,
            edits: Vec::new(),
            used: Vec::new(),
            imports: Vec::new(),
            type_names: BTreeSet::new(),
            declared: BTreeSet::new(),
            top_values: BTreeSet::new(),
            context: Context::default(),
            saw_module_syntax: false,
            kept_module_syntax: false,
            capture: None,
            depth: 0,
            in_extends: false,
            values: 0,
            names: Vec::new(),
            namespace: None,
            emitted: BTreeSet::new(),
            scopes: Vec::new(),
            scope_counter: 0,
            constructor_super: None,
            jsx: false,
            exports: Vec::new(),
            aliases: Vec::new(),
        })
    }

    fn advance(&mut self) -> Step<()> {
        self.prev_end = self.tok.end;
        self.tok = self.source.lex(self.tok.end)?;
        Ok(())
    }

    fn peek(&self) -> Step<Tok> {
        self.source.lex(self.tok.end)
    }

    fn peek_after(&self, token: &Tok) -> Step<Tok> {
        self.source.lex(token.end)
    }

    fn is_p(&self, text: &str) -> bool {
        self.tok.kind == Kind::Punct && self.tok.text == text
    }

    fn is_n(&self, text: &str) -> bool {
        self.tok.kind == Kind::Name && self.tok.text == text
    }

    fn eat_p(&mut self, text: &str) -> Step<bool> {
        if self.is_p(text) {
            self.advance()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn eat_n(&mut self, text: &str) -> Step<bool> {
        if self.is_n(text) {
            self.advance()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn expect_p(&mut self, text: &str) -> Step<()> {
        if self.is_p(text) {
            self.advance()
        } else {
            fault(self.tok.start, &format!("`{text}` was expected."))
        }
    }

    fn expect_gt(&mut self) -> Step<()> {
        if self.tok.kind == Kind::Punct && self.tok.text.starts_with('>') {
            if self.tok.text == ">" {
                return self.advance();
            }
            self.prev_end = self.tok.start + 1;
            self.tok = Tok {
                kind: Kind::Punct,
                start: self.tok.start + 1,
                end: self.tok.end,
                nl: false,
                text: self.tok.text[1..].to_string(),
            };
            return Ok(());
        }
        fault(self.tok.start, "`>` was expected.")
    }

    fn expect_name(&mut self) -> Step<String> {
        if self.tok.kind == Kind::Name {
            let text = self.tok.text.clone();
            self.advance()?;
            Ok(text)
        } else {
            fault(self.tok.start, "A name was expected.")
        }
    }

    fn mark(&self) -> Mark {
        Mark {
            tok: self.tok.clone(),
            prev_end: self.prev_end,
            edits: self.edits.len(),
            imports: self.imports.len(),
            used: self.used.len(),
        }
    }

    fn reset(&mut self, mark: Mark) {
        self.tok = mark.tok;
        self.prev_end = mark.prev_end;
        self.edits.truncate(mark.edits);
        self.imports.truncate(mark.imports);
        self.used.truncate(mark.used);
    }

    fn reposition(&mut self, at: usize) -> Step<()> {
        self.prev_end = at;
        self.tok = self.source.lex(at)?;
        Ok(())
    }

    fn erase(&mut self, start: usize, end: usize) {
        if end > start {
            self.edits.push(Edit {
                start,
                end,
                text: None,
                pad: true,
            });
        }
    }

    fn replace(&mut self, start: usize, end: usize, text: String) {
        self.edits.push(Edit {
            start,
            end,
            text: Some(text),
            pad: true,
        });
    }

    fn insert(&mut self, at: usize, text: String) {
        self.edits.push(Edit {
            start: at,
            end: at,
            text: Some(text),
            pad: true,
        });
    }

    fn use_name(&mut self, name: &str) {
        self.used.push(name.to_string());
    }

    fn enter(&mut self) -> Step<()> {
        self.depth += 1;
        if self.depth > 400 {
            return fault(self.tok.start, "The source nests too deeply.");
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.depth -= 1;
    }

    fn can_start_type(token: &Tok) -> bool {
        match token.kind {
            Kind::Name | Kind::Str | Kind::Num | Kind::BigInt | Kind::Template => true,
            Kind::Punct => matches!(token.text.as_str(), "(" | "[" | "{" | "-" | "<"),
            _ => false,
        }
    }

    fn ty(&mut self) -> Step<()> {
        self.enter()?;
        let outcome = self.ty_inner();
        self.leave();
        outcome
    }

    fn ty_inner(&mut self) -> Step<()> {
        if self.at_function_type()? {
            return self.ty_function();
        }
        self.ty_union()?;
        if self.is_n("extends") && !self.tok.nl {
            self.advance()?;
            let saved = std::mem::replace(&mut self.in_extends, true);
            let outcome = self.ty_without_condition();
            self.in_extends = saved;
            outcome?;
            self.expect_p("?")?;
            self.ty()?;
            self.expect_p(":")?;
            self.ty()?;
        }
        Ok(())
    }

    fn ty_without_condition(&mut self) -> Step<()> {
        if self.at_function_type()? {
            return self.ty_function();
        }
        self.ty_union()
    }

    fn at_function_type(&mut self) -> Step<bool> {
        if self.is_p("<") {
            return Ok(true);
        }
        if self.is_n("new") {
            return Ok(true);
        }
        if self.is_n("abstract") {
            let next = self.peek()?;
            return Ok(next.kind == Kind::Name && next.text == "new");
        }
        if self.is_p("(") {
            let mark = self.mark();
            let found = self.ty_parameters().is_ok() && self.is_p("=>");
            self.reset(mark);
            return Ok(found);
        }
        Ok(false)
    }

    fn ty_function(&mut self) -> Step<()> {
        self.eat_n("abstract")?;
        self.eat_n("new")?;
        if self.is_p("<") {
            self.ty_parameter_declarations()?;
        }
        self.ty_parameters()?;
        self.expect_p("=>")?;
        self.ty_return()
    }

    fn ty_return(&mut self) -> Step<()> {
        if self.is_n("asserts") {
            let next = self.peek()?;
            if next.kind == Kind::Name && !next.nl {
                self.advance()?;
                self.advance()?;
                if self.is_n("is") && !self.tok.nl {
                    self.advance()?;
                    self.ty()?;
                }
                return Ok(());
            }
        }
        if self.tok.kind == Kind::Name {
            let next = self.peek()?;
            if next.kind == Kind::Name && next.text == "is" && !next.nl {
                self.advance()?;
                self.advance()?;
                return self.ty();
            }
        }
        self.ty()
    }

    fn ty_parameters(&mut self) -> Step<()> {
        self.expect_p("(")?;
        while !self.is_p(")") {
            while self.is_p("@") {
                self.decorator()?;
            }
            self.eat_p("...")?;
            while matches!(
                self.tok.text.as_str(),
                "public" | "private" | "protected" | "readonly" | "override"
            ) && self.tok.kind == Kind::Name
            {
                let next = self.peek()?;
                if next.kind == Kind::Name || next.text == "{" || next.text == "[" {
                    self.advance()?;
                } else {
                    break;
                }
            }
            match (self.tok.kind, self.tok.text.as_str()) {
                (Kind::Punct, "{") | (Kind::Punct, "[") => self.skip_balanced()?,
                (Kind::Name, _) => self.advance()?,
                _ => return fault(self.tok.start, "A parameter was expected."),
            }
            self.eat_p("?")?;
            if self.eat_p(":")? {
                self.ty()?;
            }
            if self.eat_p("=")? {
                self.assign()?;
            }
            if !self.eat_p(",")? {
                break;
            }
        }
        self.expect_p(")")
    }

    fn skip_balanced(&mut self) -> Step<()> {
        let mut depth = 0usize;
        loop {
            match (self.tok.kind, self.tok.text.as_str()) {
                (Kind::End, _) => return fault(self.tok.start, "A bracket is not closed."),
                (Kind::Punct, "{") | (Kind::Punct, "[") | (Kind::Punct, "(") => depth += 1,
                (Kind::Punct, "}") | (Kind::Punct, "]") | (Kind::Punct, ")") => {
                    depth -= 1;
                    if depth == 0 {
                        return self.advance();
                    }
                }
                (Kind::Template, _) => {
                    self.template(false)?;
                    continue;
                }
                _ => {}
            }
            self.advance()?;
        }
    }

    fn ty_parameter_declarations(&mut self) -> Step<()> {
        self.expect_p("<")?;
        while !self.is_p(">") && self.tok.kind != Kind::End {
            while matches!(self.tok.text.as_str(), "const" | "in" | "out")
                && self.tok.kind == Kind::Name
            {
                let next = self.peek()?;
                if next.kind == Kind::Name {
                    self.advance()?;
                } else {
                    break;
                }
            }
            self.expect_name()?;
            if self.eat_n("extends")? {
                self.ty()?;
            }
            if self.eat_p("=")? {
                self.ty()?;
            }
            if !self.eat_p(",")? {
                break;
            }
        }
        self.expect_gt()
    }

    fn ty_arguments(&mut self) -> Step<()> {
        self.expect_p("<")?;
        while !self.tok.text.starts_with('>') || self.tok.kind != Kind::Punct {
            self.ty()?;
            if !self.eat_p(",")? {
                break;
            }
        }
        self.expect_gt()
    }

    fn ty_union(&mut self) -> Step<()> {
        self.eat_p("|")?;
        self.ty_intersection()?;
        while self.is_p("|") {
            self.advance()?;
            self.ty_intersection()?;
        }
        Ok(())
    }

    fn ty_intersection(&mut self) -> Step<()> {
        self.eat_p("&")?;
        self.ty_operator()?;
        while self.is_p("&") {
            self.advance()?;
            self.ty_operator()?;
        }
        Ok(())
    }

    fn ty_operator(&mut self) -> Step<()> {
        if self.tok.kind == Kind::Name
            && matches!(self.tok.text.as_str(), "keyof" | "unique" | "readonly")
        {
            let next = self.peek()?;
            if Self::can_start_type(&next) && !(next.kind == Kind::Punct && next.text == "<") {
                self.advance()?;
                return self.ty_operator();
            }
        }
        if self.is_n("infer") {
            let next = self.peek()?;
            if next.kind == Kind::Name {
                self.advance()?;
                self.advance()?;
                if self.is_n("extends") {
                    let mark = self.mark();
                    self.advance()?;
                    let saved = std::mem::replace(&mut self.in_extends, false);
                    let parsed = self.ty_without_condition().is_ok();
                    self.in_extends = saved;
                    if parsed && (saved || !self.is_p("?")) {
                        return Ok(());
                    }
                    self.reset(mark);
                }
                return Ok(());
            }
        }
        self.ty_postfix()
    }

    fn ty_postfix(&mut self) -> Step<()> {
        self.ty_primary()?;
        while self.is_p("[") && !self.tok.nl {
            self.advance()?;
            if !self.is_p("]") {
                self.ty()?;
            }
            self.expect_p("]")?;
        }
        Ok(())
    }

    fn ty_primary(&mut self) -> Step<()> {
        match self.tok.kind {
            Kind::Str | Kind::Num | Kind::BigInt => self.advance(),
            Kind::Template => self.template(true),
            Kind::Punct => match self.tok.text.as_str() {
                "-" => {
                    self.advance()?;
                    if matches!(self.tok.kind, Kind::Num | Kind::BigInt) {
                        self.advance()
                    } else {
                        fault(self.tok.start, "A number was expected.")
                    }
                }
                "(" => {
                    self.advance()?;
                    self.ty()?;
                    self.expect_p(")")
                }
                "[" => self.ty_tuple(),
                "{" => self.ty_object(),
                _ => fault(self.tok.start, "A type was expected."),
            },
            Kind::Name => match self.tok.text.as_str() {
                "typeof" => {
                    self.advance()?;
                    if self.is_n("import") {
                        return self.ty_import();
                    }
                    self.ty_qualified()?;
                    if self.is_p("<") && !self.tok.nl {
                        self.ty_arguments()?;
                    }
                    Ok(())
                }
                "import" => self.ty_import(),
                _ => {
                    self.ty_qualified()?;
                    if self.is_p("<") && !self.tok.nl {
                        self.ty_arguments()?;
                    }
                    Ok(())
                }
            },
            _ => fault(self.tok.start, "A type was expected."),
        }
    }

    fn ty_import(&mut self) -> Step<()> {
        self.advance()?;
        self.expect_p("(")?;
        self.assign()?;
        if self.eat_p(",")? && !self.is_p(")") {
            self.assign()?;
            self.eat_p(",")?;
        }
        self.expect_p(")")?;
        if self.eat_p(".")? {
            self.ty_qualified()?;
        }
        if self.is_p("<") && !self.tok.nl {
            self.ty_arguments()?;
        }
        Ok(())
    }

    fn ty_qualified(&mut self) -> Step<()> {
        if self.tok.kind != Kind::Name {
            return fault(self.tok.start, "A type name was expected.");
        }
        self.advance()?;
        while self.is_p(".") {
            self.advance()?;
            if self.tok.kind != Kind::Name && self.tok.kind != Kind::Private {
                return fault(self.tok.start, "A type name was expected.");
            }
            self.advance()?;
        }
        Ok(())
    }

    fn ty_tuple(&mut self) -> Step<()> {
        self.expect_p("[")?;
        while !self.is_p("]") {
            self.eat_p("...")?;
            if self.tok.kind == Kind::Name {
                let next = self.peek()?;
                let named = (next.kind == Kind::Punct && next.text == ":")
                    || (next.kind == Kind::Punct
                        && next.text == "?"
                        && self.peek_after(&next)?.text == ":");
                if named {
                    self.advance()?;
                    self.eat_p("?")?;
                    self.expect_p(":")?;
                }
            }
            self.ty()?;
            self.eat_p("?")?;
            if !self.eat_p(",")? {
                break;
            }
        }
        self.expect_p("]")
    }

    fn ty_object(&mut self) -> Step<()> {
        self.expect_p("{")?;
        while !self.is_p("}") {
            if self.tok.kind == Kind::End {
                return fault(self.tok.start, "A type is not closed.");
            }
            self.ty_member()?;
            if !(self.eat_p(";")? || self.eat_p(",")?) && !self.is_p("}") && !self.tok.nl {
                return fault(self.tok.start, "A member separator was expected.");
            }
        }
        self.expect_p("}")
    }

    fn ty_member(&mut self) -> Step<()> {
        for sign in ["+", "-"] {
            if self.is_p(sign) {
                self.advance()?;
            }
        }
        if self.is_n("readonly") {
            let next = self.peek()?;
            let key_follows = matches!(
                next.kind,
                Kind::Name | Kind::Str | Kind::Num | Kind::Private
            ) || (next.kind == Kind::Punct && next.text == "[");
            if key_follows {
                self.advance()?;
            }
        }
        if self.is_p("[") {
            let next = self.peek()?;
            if next.kind == Kind::Name {
                let after = self.peek_after(&next)?;
                if after.kind == Kind::Name && after.text == "in" {
                    self.advance()?;
                    self.advance()?;
                    self.advance()?;
                    self.ty()?;
                    if self.eat_n("as")? {
                        self.ty()?;
                    }
                    self.expect_p("]")?;
                    for sign in ["+", "-"] {
                        if self.is_p(sign) {
                            self.advance()?;
                        }
                    }
                    self.eat_p("?")?;
                    if self.eat_p(":")? {
                        self.ty()?;
                    }
                    return Ok(());
                }
                if after.kind == Kind::Punct && (after.text == ":" || after.text == "?") {
                    let third = self.peek_after(&after)?;
                    if after.text == ":" || third.text == ":" {
                        self.advance()?;
                        self.advance()?;
                        self.eat_p("?")?;
                        self.eat_p(":")?;
                        self.ty()?;
                        self.expect_p("]")?;
                        self.eat_p("?")?;
                        if self.eat_p(":")? {
                            self.ty()?;
                        }
                        return Ok(());
                    }
                }
            }
        }
        if self.is_p("(") || self.is_p("<") {
            return self.ty_signature_rest();
        }
        if self.is_n("new") {
            let next = self.peek()?;
            if next.kind == Kind::Punct && (next.text == "(" || next.text == "<") {
                self.advance()?;
                return self.ty_signature_rest();
            }
        }
        if (self.is_n("get") || self.is_n("set")) && !self.peek()?.nl {
            let next = self.peek()?;
            let key_follows = matches!(
                next.kind,
                Kind::Name | Kind::Str | Kind::Num | Kind::Private
            ) || (next.kind == Kind::Punct && next.text == "[");
            if key_follows {
                self.advance()?;
            }
        }
        match self.tok.kind {
            Kind::Name | Kind::Str | Kind::Num | Kind::BigInt | Kind::Private => self.advance()?,
            Kind::Punct if self.tok.text == "[" => {
                self.advance()?;
                self.assign()?;
                self.expect_p("]")?;
            }
            _ => return fault(self.tok.start, "A member name was expected."),
        }
        self.eat_p("?")?;
        if self.is_p("(") || self.is_p("<") {
            return self.ty_signature_rest();
        }
        if self.eat_p(":")? {
            self.ty()?;
        }
        Ok(())
    }

    fn ty_signature_rest(&mut self) -> Step<()> {
        if self.is_p("<") {
            self.ty_parameter_declarations()?;
        }
        self.ty_parameters()?;
        if self.eat_p(":")? {
            self.ty_return()?;
        }
        Ok(())
    }

    fn template(&mut self, as_type: bool) -> Step<()> {
        let mut at = self.tok.end;
        loop {
            match self.source.at(at) {
                None => return fault(self.tok.start, "A template is not closed."),
                Some('\\') => at += 2,
                Some('`') => {
                    at += 1;
                    break;
                }
                Some('$') if self.source.at(at + 1) == Some('{') => {
                    self.reposition(at + 2)?;
                    if as_type {
                        self.ty()?;
                    } else {
                        self.expression()?;
                    }
                    if !self.is_p("}") {
                        return fault(self.tok.start, "`}` was expected.");
                    }
                    at = self.tok.end;
                }
                Some(_) => at += 1,
            }
        }
        self.prev_end = at;
        self.tok = self.source.lex(at)?;
        Ok(())
    }
}

impl Parser {
    fn expression(&mut self) -> Step<()> {
        self.assign()?;
        while self.is_p(",") {
            self.advance()?;
            self.assign()?;
        }
        Ok(())
    }

    fn assign(&mut self) -> Step<()> {
        self.enter()?;
        let outcome = self.assign_inner();
        self.leave();
        outcome
    }

    fn assign_inner(&mut self) -> Step<()> {
        if self.is_n("yield") && self.context.is_generator {
            self.advance()?;
            if !self.tok.nl && !self.ends_expression() {
                self.eat_p("*")?;
                self.assign()?;
            }
            return Ok(());
        }
        if self.try_arrow()? {
            return Ok(());
        }
        self.conditional()?;
        if self.tok.kind == Kind::Punct && ASSIGNMENT.contains(&self.tok.text.as_str()) {
            self.advance()?;
            self.assign()?;
        }
        Ok(())
    }

    fn ends_expression(&self) -> bool {
        match self.tok.kind {
            Kind::End => true,
            Kind::Punct => matches!(
                self.tok.text.as_str(),
                ")" | "]" | "}" | "," | ";" | ":" | "=>" | "=" | "?"
            ),
            Kind::Name => matches!(self.tok.text.as_str(), "of" | "in" | "as" | "satisfies"),
            _ => false,
        }
    }

    fn conditional(&mut self) -> Step<()> {
        self.binary(1)?;
        if self.is_p("?") {
            self.advance()?;
            let saved = self.context.no_in;
            self.context.no_in = false;
            self.assign()?;
            self.context.no_in = saved;
            self.expect_p(":")?;
            self.assign()?;
        }
        Ok(())
    }

    fn binary(&mut self, minimum: u8) -> Step<()> {
        self.unary()?;
        loop {
            if self.tok.kind == Kind::Name
                && (self.tok.text == "as" || self.tok.text == "satisfies")
                && !self.tok.nl
                && minimum <= 8
            {
                let start = self.tok.start;
                self.advance()?;
                self.ty()?;
                let end = self.prev_end;
                self.erase(start, end);
                continue;
            }
            let Some((precedence, right)) = binary_precedence(&self.tok, self.context.no_in) else {
                return Ok(());
            };
            if precedence < minimum {
                return Ok(());
            }
            self.advance()?;
            self.binary(if right { precedence } else { precedence + 1 })?;
        }
    }

    fn unary(&mut self) -> Step<()> {
        self.enter()?;
        let outcome = self.unary_inner();
        self.leave();
        outcome
    }

    fn unary_inner(&mut self) -> Step<()> {
        match self.tok.kind {
            Kind::Punct => match self.tok.text.as_str() {
                "!" | "~" | "+" | "-" | "++" | "--" => {
                    self.advance()?;
                    return self.unary();
                }
                "<" => {
                    let start = self.tok.start;
                    if self.jsx {
                        return unsupported(
                            start,
                            "A JSX element is not something this transpiles.",
                        );
                    }
                    self.advance()?;
                    if self.ty().is_err() || self.expect_gt().is_err() {
                        return unsupported(
                            start,
                            "A JSX element is not something this transpiles.",
                        );
                    }
                    let end = self.prev_end;
                    self.erase(start, end);
                    return self.unary();
                }
                _ => {}
            },
            Kind::Name => match self.tok.text.as_str() {
                "typeof" | "void" | "delete" => {
                    self.advance()?;
                    return self.unary();
                }
                "await" if self.awaiting()? => {
                    self.advance()?;
                    return self.unary();
                }
                _ => {}
            },
            _ => {}
        }
        self.postfix()
    }

    fn awaiting(&self) -> Step<bool> {
        if self.context.is_async {
            return Ok(true);
        }
        if self.context.in_function {
            return Ok(false);
        }
        let next = self.peek()?;
        if next.nl {
            return Ok(false);
        }
        Ok(match next.kind {
            Kind::Name => !matches!(
                next.text.as_str(),
                "of" | "in" | "as" | "instanceof" | "satisfies"
            ),
            Kind::Num | Kind::Str | Kind::BigInt | Kind::Template | Kind::Private => true,
            Kind::Punct => matches!(
                next.text.as_str(),
                "(" | "[" | "{" | "!" | "~" | "+" | "-" | "/"
            ),
            _ => false,
        })
    }

    fn postfix(&mut self) -> Step<()> {
        self.call_member()?;
        if (self.is_p("++") || self.is_p("--")) && !self.tok.nl {
            self.advance()?;
        }
        Ok(())
    }

    fn type_arguments_here(&mut self, heritage: bool) -> Step<bool> {
        let mark = self.mark();
        let start = self.tok.start;
        if self.ty_arguments().is_ok() {
            let next = &self.tok;
            let accepted = match next.kind {
                Kind::Template => true,
                Kind::Punct => match next.text.as_str() {
                    "(" => true,
                    "{" => heritage,
                    ")" | "]" | "}" | ";" | "," | ":" | "?" | "=" | "==" | "===" | "!=" | "!=="
                    | "&&" | "||" | "??" | "." | "?." => true,
                    _ => false,
                },
                Kind::End => true,
                Kind::Name => heritage && matches!(next.text.as_str(), "implements" | "extends"),
                _ => false,
            } || next.nl;
            if accepted {
                let end = self.prev_end;
                self.erase(start, end);
                return Ok(true);
            }
        }
        self.reset(mark);
        Ok(false)
    }

    fn call_member(&mut self) -> Step<()> {
        self.call_member_with(false)
    }

    fn call_member_with(&mut self, heritage: bool) -> Step<()> {
        self.primary()?;
        loop {
            match (self.tok.kind, self.tok.text.as_str()) {
                (Kind::Punct, ".") => {
                    self.advance()?;
                    if self.tok.kind == Kind::Name || self.tok.kind == Kind::Private {
                        self.advance()?;
                    } else {
                        return fault(self.tok.start, "A property name was expected.");
                    }
                }
                (Kind::Punct, "?.") => {
                    self.advance()?;
                    match (self.tok.kind, self.tok.text.as_str()) {
                        (Kind::Punct, "(") => self.arguments()?,
                        (Kind::Punct, "[") => {
                            self.advance()?;
                            self.expression()?;
                            self.expect_p("]")?;
                        }
                        (Kind::Punct, "<") => {
                            let start = self.tok.start;
                            self.ty_arguments()?;
                            let end = self.prev_end;
                            self.erase(start, end);
                        }
                        (Kind::Name, _) | (Kind::Private, _) => self.advance()?,
                        _ => return fault(self.tok.start, "A property name was expected."),
                    }
                }
                (Kind::Punct, "[") => {
                    self.advance()?;
                    self.expression()?;
                    self.expect_p("]")?;
                }
                (Kind::Punct, "(") => self.arguments()?,
                (Kind::Punct, "!") if !self.tok.nl => {
                    let start = self.tok.start;
                    self.advance()?;
                    self.erase(start, start + 1);
                }
                (Kind::Template, _) => self.template(false)?,
                (Kind::Punct, "<") => {
                    if !self.type_arguments_here(heritage)? {
                        return Ok(());
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn arguments(&mut self) -> Step<()> {
        self.expect_p("(")?;
        let saved = self.context.no_in;
        self.context.no_in = false;
        while !self.is_p(")") {
            self.eat_p("...")?;
            self.assign()?;
            if !self.eat_p(",")? {
                break;
            }
        }
        self.context.no_in = saved;
        self.expect_p(")")
    }

    fn decorator(&mut self) -> Step<()> {
        self.expect_p("@")?;
        if self.eat_p("(")? {
            self.expression()?;
            return self.expect_p(")");
        }
        let name = self.expect_name()?;
        self.use_name(&name);
        while self.is_p(".") {
            self.advance()?;
            self.expect_name()?;
        }
        if self.is_p("<") {
            let start = self.tok.start;
            self.ty_arguments()?;
            let end = self.prev_end;
            self.erase(start, end);
        }
        if self.is_p("(") {
            self.arguments()?;
        }
        Ok(())
    }

    fn try_arrow(&mut self) -> Step<bool> {
        if self.tok.kind == Kind::Name {
            let next = self.peek()?;
            if next.kind == Kind::Punct
                && next.text == "=>"
                && !next.nl
                && !is_reserved(&self.tok.text)
            {
                self.advance()?;
                self.arrow_body(false)?;
                return Ok(true);
            }
            if self.tok.text == "async" && !next.nl {
                if next.kind == Kind::Name && next.text != "function" {
                    let after = self.peek_after(&next)?;
                    if after.kind == Kind::Punct && after.text == "=>" && !after.nl {
                        self.advance()?;
                        self.advance()?;
                        self.arrow_body(true)?;
                        return Ok(true);
                    }
                }
                if next.kind == Kind::Punct && (next.text == "(" || next.text == "<") {
                    let mark = self.mark();
                    self.advance()?;
                    if self.arrow_head().is_ok() {
                        self.arrow_body(true)?;
                        return Ok(true);
                    }
                    self.reset(mark);
                }
            }
            return Ok(false);
        }
        if self.is_p("(") || self.is_p("<") {
            let mark = self.mark();
            if self.arrow_head().is_ok() {
                self.arrow_body(false)?;
                return Ok(true);
            }
            self.reset(mark);
        }
        Ok(false)
    }

    fn arrow_head(&mut self) -> Step<()> {
        if self.is_p("<") {
            let start = self.tok.start;
            self.ty_parameter_declarations()?;
            let end = self.prev_end;
            self.erase(start, end);
        }
        if !self.is_p("(") {
            return fault(self.tok.start, "`(` was expected.");
        }
        self.parameters(None)?;
        if self.is_p(":") {
            let start = self.tok.start;
            self.advance()?;
            self.ty_return()?;
            let end = self.prev_end;
            let lines = self.source.chars[start..end]
                .iter()
                .filter(|c| **c == '\n')
                .count();
            if lines > 0 && self.is_p("=>") && !self.tok.nl {
                let blanks: String = (start..end).map(|_| ' ').collect();
                self.edits.push(Edit {
                    start,
                    end,
                    text: Some(blanks),
                    pad: false,
                });
                self.insert(self.tok.end, "\n".repeat(lines));
            } else {
                self.erase(start, end);
            }
        }
        if self.is_p("=>") && !self.tok.nl {
            Ok(())
        } else {
            fault(self.tok.start, "`=>` was expected.")
        }
    }

    fn arrow_body(&mut self, is_async: bool) -> Step<()> {
        self.expect_p("=>")?;
        let saved = self.context;
        self.context.is_async = is_async;
        self.context.is_generator = false;
        self.context.in_function = true;
        let outcome = if self.is_p("{") {
            self.function_block()
        } else {
            self.assign()
        };
        let no_in = saved.no_in;
        self.context = saved;
        self.context.no_in = no_in;
        outcome
    }

    fn enter_scope(&mut self) {
        self.scope_counter += 1;
        self.scopes.push(self.scope_counter);
    }

    fn leave_scope(&mut self) {
        self.scopes.pop();
    }

    fn function_block(&mut self) -> Step<()> {
        self.enter_scope();
        let outcome = self.function_block_inner();
        self.leave_scope();
        outcome
    }

    fn function_block_inner(&mut self) -> Step<()> {
        self.expect_p("{")?;
        let saved = self.context.no_in;
        self.context.no_in = false;
        while !self.is_p("}") {
            if self.tok.kind == Kind::End {
                return fault(self.tok.start, "A block is not closed.");
            }
            let calls_super = self.is_n("super") && {
                let next = self.peek()?;
                next.kind == Kind::Punct && next.text == "("
            };
            self.statement()?;
            if calls_super {
                self.constructor_super = Some(self.prev_end);
            }
        }
        self.context.no_in = saved;
        self.advance()
    }

    fn parameters(&mut self, properties: Option<&mut Vec<ParamProperty>>) -> Step<()> {
        let mut properties = properties;
        self.expect_p("(")?;
        let saved = self.context.no_in;
        self.context.no_in = false;
        while !self.is_p(")") {
            while self.is_p("@") {
                self.decorator()?;
            }
            let mut property = false;
            while self.tok.kind == Kind::Name
                && matches!(
                    self.tok.text.as_str(),
                    "public" | "private" | "protected" | "readonly" | "override"
                )
            {
                let next = self.peek()?;
                let binds = next.kind == Kind::Name
                    || (next.kind == Kind::Punct && (next.text == "{" || next.text == "["));
                if !binds {
                    break;
                }
                let (start, end) = (self.tok.start, self.tok.end);
                self.advance()?;
                self.erase(start, end);
                property = true;
            }
            self.eat_p("...")?;
            if self.is_n("this") {
                let next = self.peek()?;
                if next.kind == Kind::Punct
                    && (next.text == ":" || next.text == "," || next.text == ")")
                {
                    let start = self.tok.start;
                    self.advance()?;
                    if self.eat_p(":")? {
                        self.ty()?;
                    }
                    self.eat_p(",")?;
                    let end = self.prev_end;
                    self.erase(start, end);
                    continue;
                }
            }
            let name = if self.tok.kind == Kind::Name {
                Some(self.tok.text.clone())
            } else {
                None
            };
            self.binding_target()?;
            if self.is_p("?") {
                let start = self.tok.start;
                self.advance()?;
                self.erase(start, start + 1);
            }
            if self.is_p(":") {
                let start = self.tok.start;
                self.advance()?;
                self.ty()?;
                let end = self.prev_end;
                self.erase(start, end);
            }
            if self.eat_p("=")? {
                self.assign()?;
            }
            if property {
                if let (Some(list), Some(name)) = (properties.as_deref_mut(), name) {
                    list.push(ParamProperty { name });
                }
            }
            if !self.eat_p(",")? {
                break;
            }
        }
        self.context.no_in = saved;
        self.expect_p(")")
    }

    fn binding_target(&mut self) -> Step<()> {
        self.enter()?;
        let outcome = self.binding_target_inner();
        self.leave();
        outcome
    }

    fn binding_target_inner(&mut self) -> Step<()> {
        match (self.tok.kind, self.tok.text.as_str()) {
            (Kind::Name, _) => self.advance(),
            (Kind::Punct, "[") => {
                self.advance()?;
                while !self.is_p("]") {
                    if self.is_p(",") {
                        self.advance()?;
                        continue;
                    }
                    self.eat_p("...")?;
                    self.binding_target()?;
                    if self.eat_p("=")? {
                        self.assign()?;
                    }
                    if !self.eat_p(",")? {
                        break;
                    }
                }
                self.expect_p("]")
            }
            (Kind::Punct, "{") => {
                self.advance()?;
                while !self.is_p("}") {
                    if self.eat_p("...")? {
                        self.binding_target()?;
                    } else {
                        let shorthand = self.tok.kind == Kind::Name && {
                            let next = self.peek()?;
                            next.kind == Kind::Punct
                                && (next.text == "," || next.text == "}" || next.text == "=")
                        };
                        if shorthand {
                            self.advance()?;
                        } else {
                            self.property_key()?;
                            self.expect_p(":")?;
                            self.binding_target()?;
                        }
                        if self.eat_p("=")? {
                            self.assign()?;
                        }
                    }
                    if !self.eat_p(",")? {
                        break;
                    }
                }
                self.expect_p("}")
            }
            _ => fault(self.tok.start, "A binding was expected."),
        }
    }

    fn primary(&mut self) -> Step<()> {
        self.enter()?;
        let outcome = self.primary_inner();
        self.leave();
        outcome
    }

    fn primary_inner(&mut self) -> Step<()> {
        match self.tok.kind {
            Kind::Num | Kind::BigInt | Kind::Str | Kind::Private => self.advance(),
            Kind::Template => self.template(false),
            Kind::End => fault(self.tok.start, "An expression was expected."),
            Kind::Punct => match self.tok.text.as_str() {
                "(" => {
                    self.advance()?;
                    let saved = self.context.no_in;
                    self.context.no_in = false;
                    self.expression()?;
                    self.context.no_in = saved;
                    self.expect_p(")")
                }
                "[" => self.array_literal(),
                "{" => self.object_literal(),
                "/" | "/=" => {
                    let end = self.source.scan_regex(self.tok.start)?;
                    self.prev_end = end;
                    self.tok = self.source.lex(end)?;
                    Ok(())
                }
                "@" => {
                    while self.is_p("@") {
                        self.decorator()?;
                    }
                    self.primary()
                }
                _ => fault(self.tok.start, "An expression was expected."),
            },
            Kind::Name => {
                let word = self.tok.text.clone();
                match word.as_str() {
                    "function" => self.function_expression(false),
                    "async" => {
                        let next = self.peek()?;
                        if next.kind == Kind::Name && next.text == "function" && !next.nl {
                            self.advance()?;
                            self.function_expression(true)
                        } else {
                            self.use_name("async");
                            self.advance()
                        }
                    }
                    "class" => self.class(false),
                    "new" => self.new_expression(),
                    "import" => {
                        self.advance()?;
                        if self.is_p("(") {
                            self.arguments()
                        } else if self.eat_p(".")? {
                            self.expect_name()?;
                            Ok(())
                        } else {
                            fault(self.tok.start, "`(` or `.` was expected.")
                        }
                    }
                    "this" | "super" | "null" | "true" | "false" => self.advance(),
                    _ if is_reserved(&word) => fault(self.tok.start, "An expression was expected."),
                    _ => {
                        let (start, end) = (self.tok.start, self.tok.end);
                        if let Some(list) = self.capture.as_mut() {
                            list.push((start, end, word.clone()));
                        }
                        self.use_name(&word);
                        self.advance()
                    }
                }
            }
        }
    }

    fn new_expression(&mut self) -> Step<()> {
        self.advance()?;
        if self.is_p(".") {
            self.advance()?;
            self.expect_name()?;
            return Ok(());
        }
        self.enter()?;
        let callee = (|| {
            if self.is_n("new") {
                self.new_expression()?;
            } else {
                self.primary()?;
            }
            loop {
                match (self.tok.kind, self.tok.text.as_str()) {
                    (Kind::Punct, ".") => {
                        self.advance()?;
                        if self.tok.kind == Kind::Name || self.tok.kind == Kind::Private {
                            self.advance()?;
                        } else {
                            return fault(self.tok.start, "A property name was expected.");
                        }
                    }
                    (Kind::Punct, "[") => {
                        self.advance()?;
                        self.expression()?;
                        self.expect_p("]")?;
                    }
                    (Kind::Punct, "!") if !self.tok.nl => {
                        let start = self.tok.start;
                        self.advance()?;
                        self.erase(start, start + 1);
                    }
                    (Kind::Template, _) => self.template(false)?,
                    _ => return Ok(()),
                }
            }
        })();
        self.leave();
        callee?;
        if self.is_p("<") {
            let mark = self.mark();
            let start = self.tok.start;
            if self.ty_arguments().is_ok() {
                let end = self.prev_end;
                self.erase(start, end);
            } else {
                self.reset(mark);
            }
        }
        if self.is_p("(") {
            self.arguments()?;
        }
        Ok(())
    }

    fn array_literal(&mut self) -> Step<()> {
        self.expect_p("[")?;
        let saved = self.context.no_in;
        self.context.no_in = false;
        while !self.is_p("]") {
            if self.is_p(",") {
                self.advance()?;
                continue;
            }
            self.eat_p("...")?;
            self.assign()?;
            if !self.eat_p(",")? {
                break;
            }
        }
        self.context.no_in = saved;
        self.expect_p("]")
    }

    fn property_key(&mut self) -> Step<()> {
        match (self.tok.kind, self.tok.text.as_str()) {
            (Kind::Name, _)
            | (Kind::Str, _)
            | (Kind::Num, _)
            | (Kind::BigInt, _)
            | (Kind::Private, _) => self.advance(),
            (Kind::Punct, "[") => {
                self.advance()?;
                self.assign()?;
                self.expect_p("]")
            }
            _ => fault(self.tok.start, "A property name was expected."),
        }
    }

    fn object_literal(&mut self) -> Step<()> {
        self.expect_p("{")?;
        let saved = self.context.no_in;
        self.context.no_in = false;
        while !self.is_p("}") {
            if self.tok.kind == Kind::End {
                return fault(self.tok.start, "An object is not closed.");
            }
            if self.eat_p("...")? {
                self.assign()?;
            } else {
                self.object_member()?;
            }
            if !self.eat_p(",")? {
                break;
            }
        }
        self.context.no_in = saved;
        self.expect_p("}")
    }

    fn modifier_follows(&self) -> Step<bool> {
        let next = self.peek()?;
        Ok(match next.kind {
            Kind::Name | Kind::Str | Kind::Num | Kind::BigInt | Kind::Private => true,
            Kind::Punct => matches!(next.text.as_str(), "[" | "*" | "{"),
            _ => false,
        })
    }

    fn object_member(&mut self) -> Step<()> {
        let mut is_async = false;
        let mut is_generator = false;
        if (self.is_n("async") && !self.peek()?.nl && self.modifier_follows()?)
            || ((self.is_n("get") || self.is_n("set")) && self.modifier_follows()?)
        {
            is_async = self.is_n("async");
            self.advance()?;
        }
        if self.eat_p("*")? {
            is_generator = true;
        }
        let key_name = if self.tok.kind == Kind::Name {
            Some(self.tok.text.clone())
        } else {
            None
        };
        self.property_key()?;
        if self.is_p("?") {
            let start = self.tok.start;
            self.advance()?;
            self.erase(start, start + 1);
        }
        if self.is_p("(") || self.is_p("<") {
            return self.method_rest(is_async, is_generator, None).map(|_| ());
        }
        if self.eat_p(":")? {
            return self.assign();
        }
        if self.eat_p("=")? {
            return self.assign();
        }
        if let Some(name) = key_name {
            self.use_name(&name);
        }
        Ok(())
    }

    fn method_rest(
        &mut self,
        is_async: bool,
        is_generator: bool,
        properties: Option<&mut Vec<ParamProperty>>,
    ) -> Step<Option<(usize, usize)>> {
        if self.is_p("<") {
            let start = self.tok.start;
            self.ty_parameter_declarations()?;
            let end = self.prev_end;
            self.erase(start, end);
        }
        self.parameters(properties)?;
        if self.is_p(":") {
            let start = self.tok.start;
            self.advance()?;
            self.ty_return()?;
            let end = self.prev_end;
            self.erase(start, end);
        }
        if !self.is_p("{") {
            return Ok(None);
        }
        let saved = self.context;
        self.context.is_async = is_async;
        self.context.is_generator = is_generator;
        self.context.in_function = true;
        let open = self.tok.start;
        let outcome = self.function_block();
        self.context = saved;
        outcome?;
        Ok(Some((open, self.prev_end)))
    }

    fn function_expression(&mut self, is_async: bool) -> Step<()> {
        self.advance()?;
        let is_generator = self.eat_p("*")?;
        if self.tok.kind == Kind::Name {
            self.advance()?;
        }
        if self.method_rest(is_async, is_generator, None)?.is_none() {
            return fault(self.tok.start, "A function body was expected.");
        }
        Ok(())
    }
}

struct ParamProperty {
    name: String,
}

const MEMBER_WORDS: &[&str] = &[
    "public",
    "private",
    "protected",
    "readonly",
    "override",
    "abstract",
    "declare",
    "static",
    "async",
    "accessor",
    "get",
    "set",
];

#[derive(Clone, Debug)]
struct ExportList {
    start: usize,
    end: usize,
    open: usize,
    close: usize,
    specifiers: Vec<ExportName>,
    types_dropped: bool,
    has_source: bool,
}

#[derive(Clone, Debug)]
struct ExportName {
    local: String,
    text: String,
}

impl Parser {
    fn note_value_name(&mut self, name: &str) {
        let scope = self.scopes.last().copied().unwrap_or(0);
        self.emitted.insert(format!(
            "{scope}:{}:{name}",
            self.namespace.clone().unwrap_or_default()
        ));
        if self.scopes.is_empty() && self.namespace.is_none() {
            self.top_values.insert(name.to_string());
        }
    }

    fn semicolon(&mut self) -> Step<()> {
        if self.is_p(";") {
            return self.advance();
        }
        if self.is_p("}") || self.tok.kind == Kind::End || self.tok.nl {
            return Ok(());
        }
        fault(self.tok.start, "`;` was expected.")
    }

    fn erase_statement(&mut self, start: usize) {
        let end = self.prev_end;
        self.erase(start, end);
    }

    fn statement(&mut self) -> Step<()> {
        self.enter()?;
        let outcome = self.statement_inner();
        self.leave();
        outcome
    }

    fn name_follows(&self) -> Step<bool> {
        let next = self.peek()?;
        Ok(next.kind == Kind::Name && !next.nl)
    }

    fn statement_inner(&mut self) -> Step<()> {
        let start = self.tok.start;
        match self.tok.kind {
            Kind::Punct => match self.tok.text.as_str() {
                "{" => return self.block(),
                ";" => return self.advance(),
                "@" => {
                    while self.is_p("@") {
                        self.decorator()?;
                    }
                    if self.is_n("export") {
                        return self.export_statement(start);
                    }
                    self.values += 1;
                    if self.is_n("abstract") {
                        let abstract_start = self.tok.start;
                        let abstract_end = self.tok.end;
                        self.advance()?;
                        self.erase(abstract_start, abstract_end);
                    }
                    return self.class(true);
                }
                _ => {}
            },
            Kind::Name => {
                let word = self.tok.text.clone();
                match word.as_str() {
                    "var" | "const" => return self.variable_statement(start, false),
                    "using" => {
                        let next = self.peek()?;
                        if next.kind == Kind::Name
                            && !next.nl
                            && next.text != "in"
                            && next.text != "of"
                        {
                            return self.variable_statement(start, false);
                        }
                    }
                    "await" => {
                        let next = self.peek()?;
                        if next.kind == Kind::Name && next.text == "using" && !next.nl {
                            let after = self.peek_after(&next)?;
                            if after.kind == Kind::Name && !after.nl {
                                self.advance()?;
                                return self.variable_statement(start, false);
                            }
                        }
                    }
                    "let" => {
                        let next = self.peek()?;
                        let declares = next.kind == Kind::Name
                            || (next.kind == Kind::Punct && (next.text == "[" || next.text == "{"));
                        if declares {
                            return self.variable_statement(start, false);
                        }
                    }
                    "function" => return self.function_statement(start, false, false),
                    "async" => {
                        let next = self.peek()?;
                        if next.kind == Kind::Name && next.text == "function" && !next.nl {
                            self.advance()?;
                            return self.function_statement(start, true, false);
                        }
                    }
                    "class" => {
                        self.values += 1;
                        return self.class(true);
                    }
                    "abstract" => {
                        let next = self.peek()?;
                        if next.kind == Kind::Name && next.text == "class" {
                            let end = self.tok.end;
                            self.advance()?;
                            self.erase(start, end);
                            self.values += 1;
                            return self.class(true);
                        }
                    }
                    "if" => return self.if_statement(),
                    "for" => return self.for_statement(),
                    "while" => {
                        self.advance()?;
                        self.condition()?;
                        return self.statement();
                    }
                    "do" => {
                        self.advance()?;
                        self.statement()?;
                        if !self.eat_n("while")? {
                            return fault(self.tok.start, "`while` was expected.");
                        }
                        self.condition()?;
                        self.eat_p(";")?;
                        return Ok(());
                    }
                    "return" | "throw" => {
                        self.advance()?;
                        if !self.tok.nl
                            && !self.is_p(";")
                            && !self.is_p("}")
                            && self.tok.kind != Kind::End
                        {
                            self.expression()?;
                        }
                        return self.semicolon();
                    }
                    "break" | "continue" => {
                        self.advance()?;
                        if self.tok.kind == Kind::Name && !self.tok.nl {
                            self.advance()?;
                        }
                        return self.semicolon();
                    }
                    "debugger" => {
                        self.advance()?;
                        return self.semicolon();
                    }
                    "try" => return self.try_statement(),
                    "switch" => return self.switch_statement(),
                    "with" => {
                        self.advance()?;
                        self.condition()?;
                        return self.statement();
                    }
                    "import" => {
                        let next = self.peek()?;
                        if !(next.kind == Kind::Punct && (next.text == "(" || next.text == ".")) {
                            return self.import_statement(start, false);
                        }
                    }
                    "export" => return self.export_statement(start),
                    "interface" if self.name_follows()? => return self.interface_statement(start),
                    "type" if self.name_follows()? => {
                        let next = self.peek()?;
                        let after = self.peek_after(&next)?;
                        if after.kind == Kind::Punct && (after.text == "=" || after.text == "<") {
                            return self.type_alias_statement(start);
                        }
                    }
                    "enum" if self.name_follows()? => {
                        return self.enum_statement(start, false, false);
                    }
                    "declare" => {
                        let next = self.peek()?;
                        if next.kind == Kind::Name && !next.nl {
                            return self.ambient_statement(start);
                        }
                    }
                    "namespace" | "module" => {
                        let next = self.peek()?;
                        if (next.kind == Kind::Name || next.kind == Kind::Str) && !next.nl {
                            return self.namespace_statement(start, false, false);
                        }
                    }
                    "global" => {
                        let next = self.peek()?;
                        if next.kind == Kind::Punct && next.text == "{" {
                            return self.ambient_statement(start);
                        }
                    }
                    _ => {
                        if !is_reserved(&word) {
                            let next = self.peek()?;
                            if next.kind == Kind::Punct && next.text == ":" {
                                self.advance()?;
                                self.advance()?;
                                return self.statement();
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        self.values += 1;
        self.expression()?;
        self.semicolon()
    }

    fn block(&mut self) -> Step<()> {
        self.enter_scope();
        let outcome = self.block_inner();
        self.leave_scope();
        outcome
    }

    fn block_inner(&mut self) -> Step<()> {
        self.expect_p("{")?;
        while !self.is_p("}") {
            if self.tok.kind == Kind::End {
                return fault(self.tok.start, "A block is not closed.");
            }
            self.statement()?;
        }
        self.advance()
    }

    fn condition(&mut self) -> Step<()> {
        self.expect_p("(")?;
        let saved = self.context.no_in;
        self.context.no_in = false;
        self.expression()?;
        self.context.no_in = saved;
        self.expect_p(")")
    }

    fn if_statement(&mut self) -> Step<()> {
        self.advance()?;
        self.condition()?;
        self.statement()?;
        if self.eat_n("else")? {
            self.statement()?;
        }
        Ok(())
    }

    fn try_statement(&mut self) -> Step<()> {
        self.advance()?;
        self.block()?;
        if self.eat_n("catch")? {
            if self.eat_p("(")? {
                self.binding_target()?;
                if self.is_p(":") {
                    let start = self.tok.start;
                    self.advance()?;
                    self.ty()?;
                    let end = self.prev_end;
                    self.erase(start, end);
                }
                self.expect_p(")")?;
            }
            self.block()?;
        }
        if self.eat_n("finally")? {
            self.block()?;
        }
        Ok(())
    }

    fn switch_statement(&mut self) -> Step<()> {
        self.advance()?;
        self.condition()?;
        self.expect_p("{")?;
        while !self.is_p("}") {
            if self.eat_n("case")? {
                self.expression()?;
                self.expect_p(":")?;
            } else if self.eat_n("default")? {
                self.expect_p(":")?;
            } else if self.tok.kind == Kind::End {
                return fault(self.tok.start, "A switch is not closed.");
            } else {
                self.statement()?;
            }
        }
        self.advance()
    }

    fn for_statement(&mut self) -> Step<()> {
        self.advance()?;
        self.eat_n("await")?;
        self.expect_p("(")?;
        let saved = self.context.no_in;
        self.context.no_in = true;
        let outcome = self.for_head();
        self.context.no_in = saved;
        outcome?;
        self.expect_p(")")?;
        self.statement()
    }

    fn for_head(&mut self) -> Step<()> {
        if !self.is_p(";") {
            let declares = self.is_n("var")
                || self.is_n("const")
                || (self.is_n("using") && {
                    let next = self.peek()?;
                    next.kind == Kind::Name && next.text != "of" && next.text != "in"
                })
                || (self.is_n("let") && {
                    let next = self.peek()?;
                    next.kind == Kind::Name
                        || (next.kind == Kind::Punct && (next.text == "[" || next.text == "{"))
                });
            if declares {
                self.advance()?;
                self.declarators(true)?;
            } else {
                self.expression()?;
            }
            if self.is_n("of") || self.is_n("in") {
                let is_in = self.is_n("in");
                self.advance()?;
                self.context.no_in = false;
                if is_in {
                    return self.expression();
                }
                return self.assign();
            }
        }
        self.expect_p(";")?;
        self.context.no_in = false;
        if !self.is_p(";") {
            self.expression()?;
        }
        self.expect_p(";")?;
        if !self.is_p(")") {
            self.expression()?;
        }
        Ok(())
    }

    fn declarators(&mut self, for_head: bool) -> Step<()> {
        loop {
            let name = if self.tok.kind == Kind::Name {
                Some(self.tok.text.clone())
            } else {
                None
            };
            self.binding_target()?;
            if let Some(name) = name {
                if self.scopes.is_empty() && self.namespace.is_none() {
                    self.top_values.insert(name.clone());
                }
                self.declared.insert(name.clone());
                self.names.push(name);
            } else {
                self.names.push(String::new());
            }
            if self.is_p("!") {
                let start = self.tok.start;
                self.advance()?;
                self.erase(start, start + 1);
            }
            if self.is_p(":") {
                let start = self.tok.start;
                self.advance()?;
                self.ty()?;
                let end = self.prev_end;
                self.erase(start, end);
            }
            if self.eat_p("=")? {
                self.assign()?;
            }
            if for_head && (self.is_n("of") || self.is_n("in")) {
                return Ok(());
            }
            if !self.eat_p(",")? {
                return Ok(());
            }
        }
    }

    fn variable_statement(&mut self, start: usize, exported: bool) -> Step<()> {
        let _ = exported;
        if self.is_n("const") {
            let next = self.peek()?;
            if next.kind == Kind::Name && next.text == "enum" {
                self.advance()?;
                return self.enum_statement(start, exported, true);
            }
        }
        self.values += 1;
        self.advance()?;
        self.names.clear();
        self.declarators(false)?;
        self.semicolon()
    }

    fn function_statement(&mut self, start: usize, is_async: bool, exported: bool) -> Step<()> {
        let _ = exported;
        self.advance()?;
        let is_generator = self.eat_p("*")?;
        self.names.clear();
        if self.tok.kind == Kind::Name {
            let name = self.tok.text.clone();
            self.declared.insert(name.clone());
            self.note_value_name(&name);
            self.names.push(name);
            self.advance()?;
        }
        if self.method_rest(is_async, is_generator, None)?.is_none() {
            self.eat_p(";")?;
            self.erase_statement(start);
            self.names.clear();
            return Ok(());
        }
        self.values += 1;
        Ok(())
    }

    fn interface_statement(&mut self, start: usize) -> Step<()> {
        self.advance()?;
        let name = self.expect_name()?;
        self.type_names.insert(name);
        if self.is_p("<") {
            self.ty_parameter_declarations()?;
        }
        if self.eat_n("extends")? {
            loop {
                self.ty_qualified()?;
                if self.is_p("<") {
                    self.ty_arguments()?;
                }
                if !self.eat_p(",")? {
                    break;
                }
            }
        }
        self.ty_object()?;
        self.erase_statement(start);
        Ok(())
    }

    fn type_alias_statement(&mut self, start: usize) -> Step<()> {
        self.advance()?;
        let name = self.expect_name()?;
        self.type_names.insert(name);
        if self.is_p("<") {
            self.ty_parameter_declarations()?;
        }
        self.expect_p("=")?;
        self.ty()?;
        self.eat_p(";")?;
        self.erase_statement(start);
        Ok(())
    }

    fn ambient_statement(&mut self, start: usize) -> Step<()> {
        let edits = self.edits.len();
        let values = self.values;
        let imports = self.imports.len();
        let used = self.used.len();
        let declared = self.declared.clone();
        self.advance()?;
        if self.is_n("global") {
            self.advance()?;
            self.skip_ambient_body()?;
        } else {
            self.ambient_declaration()?;
        }
        self.edits.truncate(edits);
        self.imports.truncate(imports);
        self.used.truncate(used);
        self.values = values;
        self.declared = declared;
        self.erase_statement(start);
        Ok(())
    }

    fn ambient_declaration(&mut self) -> Step<()> {
        let start = self.tok.start;
        match self.tok.text.as_str() {
            "namespace" | "module" => {
                self.advance()?;
                if self.tok.kind == Kind::Str {
                    self.advance()?;
                } else {
                    self.expect_name()?;
                    while self.eat_p(".")? {
                        self.expect_name()?;
                    }
                }
                if self.is_p("{") {
                    self.skip_ambient_body()?;
                } else {
                    self.semicolon()?;
                }
                Ok(())
            }
            "enum" => {
                self.advance()?;
                self.expect_name()?;
                self.skip_balanced()
            }
            "const" => {
                let next = self.peek()?;
                if next.kind == Kind::Name && next.text == "enum" {
                    self.advance()?;
                    self.advance()?;
                    self.expect_name()?;
                    return self.skip_balanced();
                }
                self.statement()
            }
            "type" | "interface" => {
                if self.is_n("type") {
                    self.type_alias_statement(start)
                } else {
                    self.interface_statement(start)
                }
            }
            "export" => {
                self.advance()?;
                if self.eat_p("=")? {
                    self.expression()?;
                    return self.semicolon();
                }
                if self.is_p("{") || self.is_p("*") {
                    return self.export_statement_rest(start);
                }
                self.eat_n("default")?;
                self.ambient_declaration()
            }
            _ => self.statement(),
        }
    }

    fn skip_ambient_body(&mut self) -> Step<()> {
        self.expect_p("{")?;
        while !self.is_p("}") {
            if self.tok.kind == Kind::End {
                return fault(self.tok.start, "A declaration is not closed.");
            }
            self.ambient_declaration()?;
        }
        self.advance()
    }

    fn class(&mut self, is_declaration: bool) -> Step<()> {
        self.advance()?;
        self.names.clear();
        if self.tok.kind == Kind::Name && !self.is_n("extends") && !self.is_n("implements") {
            let name = self.tok.text.clone();
            if is_declaration {
                self.declared.insert(name.clone());
                self.note_value_name(&name);
            }
            self.names.push(name);
            self.advance()?;
        }
        let names = std::mem::take(&mut self.names);
        if self.is_p("<") {
            let start = self.tok.start;
            self.ty_parameter_declarations()?;
            let end = self.prev_end;
            self.erase(start, end);
        }
        let mut extends = false;
        if self.eat_n("extends")? {
            extends = true;
            self.call_member_with(true)?;
        }
        if self.is_n("implements") {
            let start = self.tok.start;
            self.advance()?;
            loop {
                self.ty_qualified()?;
                if self.is_p("<") {
                    self.ty_arguments()?;
                }
                if !self.eat_p(",")? {
                    break;
                }
            }
            let end = self.prev_end;
            self.erase(start, end);
        }
        let open = self.tok.start;
        self.expect_p("{")?;
        let mut fields: Vec<String> = Vec::new();
        while !self.is_p("}") {
            if self.tok.kind == Kind::End {
                return fault(self.tok.start, "A class is not closed.");
            }
            fields.extend(self.class_member(extends)?);
        }
        self.advance()?;
        if !fields.is_empty() {
            let text: String = fields.iter().map(|one| format!(" {one};")).collect();
            self.insert(open + 1, text);
        }
        self.names = names;
        Ok(())
    }

    fn class_member(&mut self, extends: bool) -> Step<Vec<String>> {
        let _ = extends;
        let member_start = self.tok.start;
        while self.is_p("@") {
            self.decorator()?;
        }
        if self.eat_p(";")? {
            return Ok(Vec::new());
        }
        let mut erase_member = false;
        let mut is_static = false;
        let mut is_async = false;
        while self.tok.kind == Kind::Name && MEMBER_WORDS.contains(&self.tok.text.as_str()) {
            let word = self.tok.text.clone();
            let next = self.peek()?;
            let follows = match next.kind {
                Kind::Name | Kind::Str | Kind::Num | Kind::BigInt | Kind::Private => true,
                Kind::Punct => {
                    matches!(next.text.as_str(), "[" | "*")
                        || (next.text == "{" && word == "static")
                }
                _ => false,
            };
            if !follows || (word == "async" && next.nl) {
                break;
            }
            let (start, end) = (self.tok.start, self.tok.end);
            match word.as_str() {
                "public" | "private" | "protected" | "readonly" | "override" => {
                    self.advance()?;
                    self.erase(start, end);
                }
                "abstract" | "declare" => {
                    erase_member = true;
                    self.advance()?;
                    self.erase(start, end);
                }
                "static" => {
                    is_static = true;
                    self.advance()?;
                    if self.is_p("{") {
                        let saved = self.context;
                        self.context.in_function = true;
                        self.context.is_async = false;
                        let outcome = self.function_block();
                        self.context = saved;
                        return outcome.map(|_| Vec::new());
                    }
                }
                "async" => {
                    is_async = true;
                    self.advance()?;
                }
                _ => self.advance()?,
            }
        }
        if self.is_p("[") {
            let next = self.peek()?;
            if next.kind == Kind::Name {
                let after = self.peek_after(&next)?;
                if after.kind == Kind::Punct && after.text == ":" {
                    self.advance()?;
                    self.advance()?;
                    self.advance()?;
                    self.ty()?;
                    self.expect_p("]")?;
                    self.eat_p("?")?;
                    if self.eat_p(":")? {
                        self.ty()?;
                    }
                    self.eat_p(";")?;
                    self.erase_statement(member_start);
                    return Ok(Vec::new());
                }
            }
        }
        let is_generator = self.eat_p("*")?;
        let key = if self.tok.kind == Kind::Name || self.tok.kind == Kind::Str {
            Some(
                self.tok
                    .text
                    .trim_matches(|c| c == '"' || c == '\'')
                    .to_string(),
            )
        } else {
            None
        };
        self.property_key()?;
        if self.is_p("?") || self.is_p("!") {
            let start = self.tok.start;
            self.advance()?;
            self.erase(start, start + 1);
        }
        if self.is_p("(") || self.is_p("<") {
            let constructor = !is_static && key.as_deref() == Some("constructor");
            let mut properties: Vec<ParamProperty> = Vec::new();
            self.constructor_super = None;
            let mut names: Vec<String> = Vec::new();
            let body = self.method_rest(
                is_async,
                is_generator,
                if constructor {
                    Some(&mut properties)
                } else {
                    None
                },
            )?;
            match body {
                None => {
                    self.eat_p(";")?;
                    self.erase_statement(member_start);
                }
                Some((open, _)) => {
                    if erase_member {
                        self.erase_statement(member_start);
                    } else if !properties.is_empty() {
                        names = properties.iter().map(|one| one.name.clone()).collect();
                        let at = self.constructor_super.unwrap_or(open + 1);
                        let text: String = properties
                            .iter()
                            .map(|one| format!(" this.{0} = {0};", one.name))
                            .collect();
                        let ended = at == open + 1
                            || self.source.chars[..at]
                                .iter()
                                .rev()
                                .find(|c| !c.is_whitespace())
                                .is_some_and(|c| *c == ';' || *c == '}');
                        self.insert(at, if ended { text } else { format!(";{text}") });
                    }
                }
            }
            return Ok(names);
        }
        if self.is_p(":") {
            let start = self.tok.start;
            self.advance()?;
            self.ty()?;
            let end = self.prev_end;
            self.erase(start, end);
        }
        if self.eat_p("=")? {
            self.assign()?;
        }
        if !self.eat_p(";")? {
            if !self.is_p("}") && !self.tok.nl {
                return fault(self.tok.start, "`;` was expected.");
            }
            let at = self.prev_end;
            self.insert(at, ";".to_string());
        }
        if erase_member {
            self.erase_statement(member_start);
        }
        Ok(Vec::new())
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Constant {
    Number(f64),
    Text(String),
}

fn js_number(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.is_infinite() {
        return if value > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    if value == value.trunc() && value.abs() < 1e21 {
        return format!("{}", value as i128);
    }
    format!("{value}")
}

fn js_string(text: &str) -> String {
    let mut out = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\u{b}' => out.push_str("\\v"),
            '\0' => out.push_str("\\0"),
            other if (other as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", other as u32)),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn unquote(raw: &str) -> String {
    let inner = &raw[1..raw.len() - 1];
    let mut out = String::new();
    let mut chars = inner.chars().peekable();
    while let Some(character) = chars.next() {
        if character != '\\' {
            out.push(character);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('b') => out.push('\u{8}'),
            Some('f') => out.push('\u{c}'),
            Some('v') => out.push('\u{b}'),
            Some('0') => out.push('\0'),
            Some('x') => {
                let digits: String = chars.by_ref().take(2).collect();
                if let Some(found) = u32::from_str_radix(&digits, 16)
                    .ok()
                    .and_then(char::from_u32)
                {
                    out.push(found);
                }
            }
            Some('u') => {
                let mut digits = String::new();
                if chars.peek() == Some(&'{') {
                    chars.next();
                    for next in chars.by_ref() {
                        if next == '}' {
                            break;
                        }
                        digits.push(next);
                    }
                } else {
                    digits = chars.by_ref().take(4).collect();
                }
                if let Some(found) = u32::from_str_radix(&digits, 16)
                    .ok()
                    .and_then(char::from_u32)
                {
                    out.push(found);
                }
            }
            Some('\n') => {}
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

fn parse_number(text: &str) -> Option<f64> {
    let cleaned: String = text.chars().filter(|c| *c != '_').collect();
    let lower = cleaned.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("0x") {
        return u64::from_str_radix(rest, 16).ok().map(|v| v as f64);
    }
    if let Some(rest) = lower.strip_prefix("0o") {
        return u64::from_str_radix(rest, 8).ok().map(|v| v as f64);
    }
    if let Some(rest) = lower.strip_prefix("0b") {
        return u64::from_str_radix(rest, 2).ok().map(|v| v as f64);
    }
    lower.parse::<f64>().ok()
}

fn to_int32(value: f64) -> i32 {
    if !value.is_finite() {
        return 0;
    }
    let wrapped = value.trunc().rem_euclid(4294967296.0);
    wrapped as u32 as i32
}

struct Folder<'a> {
    tokens: Vec<Tok>,
    at: usize,
    known: &'a [(String, Option<Constant>)],
    enum_name: &'a str,
}

impl Folder<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.at)
    }

    fn punct(&self, text: &str) -> bool {
        self.peek()
            .is_some_and(|one| one.kind == Kind::Punct && one.text == text)
    }

    fn binary(&mut self, minimum: u8) -> Option<Constant> {
        let mut left = self.unary()?;
        loop {
            let Some(token) = self.peek() else {
                return Some(left);
            };
            if token.kind != Kind::Punct {
                return Some(left);
            }
            let (precedence, right) = match token.text.as_str() {
                "|" => (4, false),
                "^" => (5, false),
                "&" => (6, false),
                "<<" | ">>" | ">>>" => (9, false),
                "+" | "-" => (10, false),
                "*" | "/" | "%" => (11, false),
                "**" => (12, true),
                _ => return Some(left),
            };
            if precedence < minimum {
                return Some(left);
            }
            let operator = token.text.clone();
            self.at += 1;
            let rhs = self.binary(if right { precedence } else { precedence + 1 })?;
            left = match (left, rhs) {
                (Constant::Text(a), Constant::Text(b)) if operator == "+" => {
                    Constant::Text(format!("{a}{b}"))
                }
                (Constant::Number(a), Constant::Number(b)) => {
                    Constant::Number(match operator.as_str() {
                        "+" => a + b,
                        "-" => a - b,
                        "*" => a * b,
                        "/" => a / b,
                        "%" => a % b,
                        "**" => a.powf(b),
                        "|" => f64::from(to_int32(a) | to_int32(b)),
                        "^" => f64::from(to_int32(a) ^ to_int32(b)),
                        "&" => f64::from(to_int32(a) & to_int32(b)),
                        "<<" => f64::from(to_int32(a).wrapping_shl(to_int32(b) as u32 & 31)),
                        ">>" => f64::from(to_int32(a).wrapping_shr(to_int32(b) as u32 & 31)),
                        _ => f64::from((to_int32(a) as u32).wrapping_shr(to_int32(b) as u32 & 31)),
                    })
                }
                _ => return None,
            };
        }
    }

    fn unary(&mut self) -> Option<Constant> {
        let token = self.peek()?.clone();
        match token.kind {
            Kind::Num => {
                self.at += 1;
                parse_number(&token.text).map(Constant::Number)
            }
            Kind::Str => {
                self.at += 1;
                Some(Constant::Text(unquote(&token.text)))
            }
            Kind::Name => {
                self.at += 1;
                let name = if token.text == self.enum_name && self.punct(".") {
                    self.at += 1;
                    let member = self.peek()?.clone();
                    self.at += 1;
                    member.text
                } else if token.text == self.enum_name && self.punct("[") {
                    self.at += 1;
                    let member = self.peek()?.clone();
                    self.at += 1;
                    if !self.punct("]") {
                        return None;
                    }
                    self.at += 1;
                    unquote(&member.text)
                } else {
                    token.text
                };
                self.known
                    .iter()
                    .find(|(known, _)| *known == name)
                    .and_then(|(_, value)| value.clone())
            }
            Kind::Punct => match token.text.as_str() {
                "(" => {
                    self.at += 1;
                    let inner = self.binary(1)?;
                    if !self.punct(")") {
                        return None;
                    }
                    self.at += 1;
                    Some(inner)
                }
                "-" | "+" | "~" => {
                    self.at += 1;
                    match self.unary()? {
                        Constant::Number(value) => {
                            Some(Constant::Number(match token.text.as_str() {
                                "-" => -value,
                                "+" => value,
                                _ => f64::from(!to_int32(value)),
                            }))
                        }
                        Constant::Text(_) => None,
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }
}

impl Parser {
    fn text_between(&self, start: usize, end: usize) -> String {
        self.source.chars[start..end].iter().collect()
    }

    fn newlines_between(&self, start: usize, end: usize) -> usize {
        self.source.chars[start..end]
            .iter()
            .filter(|c| **c == '\n')
            .count()
    }

    fn enum_statement(&mut self, start: usize, exported: bool, is_const: bool) -> Step<()> {
        let _ = is_const;
        self.advance()?;
        let name = self.expect_name()?;
        self.declared.insert(name.clone());
        self.expect_p("{")?;
        let mut members: Vec<(String, Option<Constant>, String)> = Vec::new();
        let mut next_value: Option<f64> = Some(0.0);
        let mut lines: Vec<String> = Vec::new();
        while !self.is_p("}") {
            if self.tok.kind == Kind::End {
                return fault(self.tok.start, "An enum is not closed.");
            }
            let member = match self.tok.kind {
                Kind::Name => self.tok.text.clone(),
                Kind::Str => unquote(&self.tok.text),
                _ => return fault(self.tok.start, "An enum member name was expected."),
            };
            self.advance()?;
            let key = if is_identifier_name(&member) {
                format!("{name}[\"{member}\"]")
            } else {
                format!("{name}[{}]", js_string(&member))
            };
            if self.eat_p("=")? {
                let from = self.tok.start;
                let saved = self.capture.replace(Vec::new());
                let before = self.used.len();
                let outcome = self.assign();
                let captured = std::mem::replace(&mut self.capture, saved).unwrap_or_default();
                self.used.truncate(before);
                outcome?;
                let to = self.prev_end;
                let mut tokens = Vec::new();
                let mut at = from;
                while at < to {
                    let one = self.source.lex(at)?;
                    if one.start >= to || one.kind == Kind::End || one.end <= at {
                        break;
                    }
                    at = one.end;
                    if one.kind == Kind::Template {
                        let mut look = one.end;
                        let mut plain = true;
                        while let Some(c) = self.source.at(look) {
                            if c == '`' {
                                break;
                            }
                            if c == '\\' {
                                look += 1;
                            } else if c == '$' && self.source.at(look + 1) == Some('{') {
                                plain = false;
                                break;
                            }
                            look += 1;
                        }
                        if plain && self.source.at(look) == Some('`') {
                            let raw = self.text_between(one.end, look);
                            tokens.push(Tok {
                                kind: Kind::Str,
                                start: one.start,
                                end: look + 1,
                                nl: false,
                                text: format!("\"{raw}\""),
                            });
                            at = look + 1;
                            continue;
                        }
                    }
                    tokens.push(one);
                }
                let known: Vec<(String, Option<Constant>)> = members
                    .iter()
                    .map(|(n, c, _)| (n.clone(), c.clone()))
                    .collect();
                let mut folder = Folder {
                    tokens,
                    at: 0,
                    known: &known,
                    enum_name: &name,
                };
                let folded = folder
                    .binary(1)
                    .filter(|_| folder.at == folder.tokens.len());
                match folded {
                    Some(Constant::Number(value)) => {
                        lines.push(format!(
                            "{name}[{key} = {}] = {};",
                            js_number(value),
                            js_string(&member)
                        ));
                        next_value = Some(value + 1.0);
                        members.push((member.clone(), Some(Constant::Number(value)), key));
                    }
                    Some(Constant::Text(text)) => {
                        lines.push(format!("{key} = {};", js_string(&text)));
                        next_value = None;
                        members.push((member.clone(), Some(Constant::Text(text)), key));
                    }
                    None => {
                        let mut text = String::new();
                        let mut cursor = from;
                        for (ref_start, ref_end, ref_name) in &captured {
                            if *ref_start < from || *ref_end > to {
                                continue;
                            }
                            let preceded_by_dot = self.source.chars[..*ref_start]
                                .iter()
                                .rev()
                                .find(|c| !c.is_whitespace())
                                == Some(&'.');
                            if preceded_by_dot || !members.iter().any(|(n, _, _)| n == ref_name) {
                                continue;
                            }
                            text.push_str(&self.text_between(cursor, *ref_start));
                            text.push_str(&if is_identifier_name(ref_name) {
                                format!("{name}.{ref_name}")
                            } else {
                                format!("{name}[{}]", js_string(ref_name))
                            });
                            cursor = *ref_end;
                        }
                        text.push_str(&self.text_between(cursor, to));
                        let is_text = matches!(
                            self.source.lex(from)?,
                            Tok { kind: Kind::Str, end, .. } if end == to
                        );
                        if is_text {
                            lines.push(format!("{key} = {text};"));
                        } else {
                            lines.push(format!("{name}[{key} = {text}] = {};", js_string(&member)));
                        }
                        next_value = None;
                        members.push((member.clone(), None, key));
                    }
                }
            } else {
                match next_value {
                    Some(value) => {
                        lines.push(format!(
                            "{name}[{key} = {}] = {};",
                            js_number(value),
                            js_string(&member)
                        ));
                        members.push((member.clone(), Some(Constant::Number(value)), key));
                        next_value = Some(value + 1.0);
                    }
                    None => {
                        let previous = members
                            .last()
                            .map(|(_, _, key)| key.clone())
                            .unwrap_or_default();
                        lines.push(format!(
                            "{name}[{key} = {previous} + 1] = {};",
                            js_string(&member)
                        ));
                        members.push((member.clone(), None, key));
                    }
                }
            }
            if !self.eat_p(",")? {
                break;
            }
        }
        self.expect_p("}")?;
        let end = self.prev_end;
        let inside = self.namespace.clone();
        let mut text = String::new();
        let nested = inside.is_some() || !self.scopes.is_empty();
        let keyword = if nested { "let" } else { "var" };
        let scope = self.scopes.last().copied().unwrap_or(0);
        let first = self.emitted.insert(format!(
            "{scope}:{}:{name}",
            inside.clone().unwrap_or_default()
        ));
        if first {
            if exported && !nested {
                text.push_str(&format!("export {keyword} {name}; "));
            } else {
                text.push_str(&format!("{keyword} {name}; "));
            }
        }
        text.push_str(&format!("(function ({name}) {{ "));
        text.push_str(&lines.join(" "));
        let tail = match (&inside, exported) {
            (Some(outer), true) => {
                format!(" }})({name} = {outer}.{name} || ({outer}.{name} = {{}}));")
            }
            _ => format!(" }})({name} || ({name} = {{}}));"),
        };
        text.push_str(&tail);
        let pad = self.newlines_between(start, end);
        self.replace(start, end, format!("{text}{}", "\n".repeat(pad)));
        if exported && !nested {
            self.kept_module_syntax = true;
        }
        self.values += 1;
        self.names.clear();
        self.names.push(name);
        Ok(())
    }

    fn namespace_statement(&mut self, start: usize, exported: bool, declare: bool) -> Step<()> {
        let _ = declare;
        self.advance()?;
        if self.tok.kind == Kind::Str {
            let text = self.tok.start;
            self.advance()?;
            let _ = text;
            if self.is_p("{") {
                self.skip_ambient_body()?;
            } else {
                self.semicolon()?;
            }
            self.erase_statement(start);
            return Ok(());
        }
        let mut path = vec![self.expect_name()?];
        while self.eat_p(".")? {
            path.push(self.expect_name()?);
        }
        let open = self.tok.start;
        self.expect_p("{")?;
        let header_end = self.prev_end;
        let before_values = self.values;
        let outer = self.namespace.clone();
        let first_name = path[0].clone();
        let mut inner_name = outer.clone();
        for (index, one) in path.iter().enumerate() {
            let _ = index;
            inner_name = Some(one.clone());
        }
        self.namespace = inner_name;
        while !self.is_p("}") {
            if self.tok.kind == Kind::End {
                return fault(self.tok.start, "A namespace is not closed.");
            }
            self.statement()?;
        }
        self.namespace = outer.clone();
        let close = self.tok.start;
        self.advance()?;
        let end = self.prev_end;
        let _ = open;
        if self.values == before_values {
            self.erase_statement(start);
            return Ok(());
        }
        let mut head = String::new();
        let mut tail = String::new();
        let top = outer.is_none() && self.scopes.is_empty();
        let keyword = if top { "var" } else { "let" };
        let scope = self.scopes.last().copied().unwrap_or(0);
        let first_seen = self.emitted.insert(format!(
            "{scope}:{}:{first_name}",
            outer.clone().unwrap_or_default()
        ));
        if first_seen {
            if top && exported {
                head.push_str(&format!("export {keyword} {first_name}; "));
            } else {
                head.push_str(&format!("{keyword} {first_name}; "));
            }
        }
        head.push_str(&format!("(function ({first_name}) {{ "));
        let mut parent = first_name.clone();
        for one in &path[1..] {
            head.push_str(&format!("let {one}; (function ({one}) {{ "));
            tail = format!(" }})({one} = {parent}.{one} || ({parent}.{one} = {{}}));{tail}");
            parent = one.clone();
        }
        let closing = match (&outer, exported) {
            (Some(up), true) => {
                format!("}})({first_name} = {up}.{first_name} || ({up}.{first_name} = {{}}));")
            }
            _ => format!("}})({first_name} || ({first_name} = {{}}));"),
        };
        let pad_head = self.newlines_between(start, header_end);
        self.replace(
            start,
            header_end,
            format!("{head}{}", "\n".repeat(pad_head)),
        );
        self.replace(close, end, format!("{tail}{closing}"));
        if top && exported {
            self.kept_module_syntax = true;
        }
        self.values = before_values + 1;
        self.names.clear();
        self.names.push(first_name);
        Ok(())
    }
}

fn is_identifier_name(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(id_start) && chars.all(id_part)
}

impl Parser {
    fn module_string(&mut self) -> Step<()> {
        if self.tok.kind != Kind::Str {
            return fault(self.tok.start, "A module name was expected.");
        }
        self.advance()?;
        if (self.is_n("with") || self.is_n("assert")) && !self.tok.nl {
            self.advance()?;
            self.skip_balanced()?;
        }
        Ok(())
    }

    fn import_statement(&mut self, start: usize, exported: bool) -> Step<()> {
        self.saw_module_syntax = true;
        let import_start = self.tok.start;
        let import_end = self.tok.end;
        self.advance()?;
        let mut type_only = false;
        if self.is_n("type") {
            let next = self.peek()?;
            type_only = match next.kind {
                Kind::Punct => next.text == "{" || next.text == "*",
                Kind::Name if next.text != "from" => next.text != "=" || true,
                Kind::Name => {
                    let after = self.peek_after(&next)?;
                    after.kind == Kind::Name && after.text == "from"
                }
                _ => false,
            };
            if type_only && next.kind == Kind::Name && next.text != "from" {
                let after = self.peek_after(&next)?;
                if after.kind == Kind::Punct && after.text == "=" {
                    self.advance()?;
                    self.alias_import(start, import_start, import_end, exported, true)?;
                    return Ok(());
                }
            }
        }
        if type_only {
            self.advance()?;
        }
        if self.tok.kind == Kind::Str {
            self.module_string()?;
            self.semicolon()?;
            self.kept_module_syntax = true;
            return Ok(());
        }
        if self.tok.kind == Kind::Name {
            let next = self.peek()?;
            if next.kind == Kind::Punct && next.text == "=" {
                return self.alias_import(start, import_start, import_end, exported, false);
            }
        }
        let mut default = None;
        let mut namespace = None;
        let mut named: Vec<(String, String)> = Vec::new();
        let mut braces = false;
        let mut dropped_types = false;
        if self.tok.kind == Kind::Name && !self.is_n("from")
            || (self.is_n("from") && {
                let next = self.peek()?;
                next.kind == Kind::Name && next.text == "from"
            })
        {
            default = Some(self.tok.text.clone());
            self.advance()?;
            self.eat_p(",")?;
        }
        if self.eat_p("*")? {
            if !self.eat_n("as")? {
                return fault(self.tok.start, "`as` was expected.");
            }
            namespace = Some(self.expect_name()?);
        } else if self.is_p("{") {
            braces = true;
            self.advance()?;
            while !self.is_p("}") {
                if self.tok.kind == Kind::End {
                    return fault(self.tok.start, "An import list is not closed.");
                }
                let spec_start = self.tok.start;
                let mut is_type = false;
                if self.is_n("type") {
                    let next = self.peek()?;
                    if (next.kind == Kind::Name && next.text != "as") || next.kind == Kind::Str {
                        is_type = true;
                        self.advance()?;
                    } else if next.kind == Kind::Name && next.text == "as" {
                        let after = self.peek_after(&next)?;
                        if after.kind == Kind::Name && after.text == "as" {
                            is_type = true;
                            self.advance()?;
                        }
                    }
                }
                let imported = if self.tok.kind == Kind::Str {
                    let text = unquote(&self.tok.text);
                    self.advance()?;
                    text
                } else {
                    self.expect_name()?
                };
                let local = if self.eat_n("as")? {
                    self.expect_name()?
                } else {
                    imported
                };
                let spec_end = self.prev_end;
                if is_type {
                    dropped_types = true;
                } else {
                    named.push((local, self.text_between(spec_start, spec_end)));
                }
                if !self.eat_p(",")? {
                    break;
                }
            }
            self.expect_p("}")?;
        }
        let from_start = self.tok.start;
        if !self.eat_n("from")? {
            return fault(self.tok.start, "`from` was expected.");
        }
        self.module_string()?;
        self.semicolon()?;
        let end = self.prev_end;
        if type_only {
            self.erase(start, end);
            return Ok(());
        }
        self.imports.push(ImportStatement {
            start,
            end,
            clause: (import_end, from_start),
            default,
            namespace,
            named,
            braces,
            dropped_types,
        });
        Ok(())
    }

    fn alias_import(
        &mut self,
        start: usize,
        import_start: usize,
        import_end: usize,
        exported: bool,
        type_only: bool,
    ) -> Step<()> {
        let name = self.expect_name()?;
        self.expect_p("=")?;
        let mut is_require = false;
        if self.is_n("require") {
            let next = self.peek()?;
            if next.kind == Kind::Punct && next.text == "(" {
                is_require = true;
                self.advance()?;
                self.expect_p("(")?;
                if self.tok.kind != Kind::Str {
                    return fault(self.tok.start, "A module name was expected.");
                }
                self.advance()?;
                self.expect_p(")")?;
            }
        }
        if !is_require {
            let first = self.expect_name()?;
            self.use_name(&first);
            while self.eat_p(".")? {
                self.expect_name()?;
            }
        }
        self.semicolon()?;
        let end = self.prev_end;
        if type_only {
            self.erase(start, end);
            return Ok(());
        }
        self.aliases.push(AliasImport {
            start,
            import_start,
            import_end,
            end,
            name: name.clone(),
            is_require,
            exported,
        });
        self.declared.insert(name);
        Ok(())
    }

    fn declaration(&mut self, start: usize, exported: bool) -> Step<()> {
        let word = self.tok.text.clone();
        match word.as_str() {
            "var" | "let" | "const" => self.variable_statement(start, exported),
            "function" => self.function_statement(start, false, exported),
            "async" => {
                self.advance()?;
                self.function_statement(start, true, exported)
            }
            "class" => {
                self.values += 1;
                self.class(true)
            }
            "abstract" => {
                let (from, to) = (self.tok.start, self.tok.end);
                self.advance()?;
                self.erase(from, to);
                self.values += 1;
                self.class(true)
            }
            "enum" => self.enum_statement(start, exported, false),
            "namespace" | "module" => self.namespace_statement(start, exported, false),
            "interface" => self.interface_statement(start),
            "type" => self.type_alias_statement(start),
            "declare" => self.ambient_statement(start),
            "import" => self.import_statement(start, true),
            _ => fault(self.tok.start, "A declaration was expected."),
        }
    }

    fn export_statement(&mut self, start: usize) -> Step<()> {
        if self.namespace.is_none() {
            self.saw_module_syntax = true;
        }
        let export_start = self.tok.start;
        let export_end = self.tok.end;
        self.advance()?;
        if let Some(inside) = self.namespace.clone() {
            self.erase(export_start, export_end);
            let word = self.tok.text.clone();
            let before = self.values;
            self.names.clear();
            self.declaration(start, true)?;
            let own_assignment = matches!(word.as_str(), "enum" | "namespace" | "module");
            if !own_assignment && self.values > before {
                let names = std::mem::take(&mut self.names);
                let text: String = names
                    .iter()
                    .filter(|one| !one.is_empty())
                    .map(|one| format!(" {inside}.{one} = {one};"))
                    .collect();
                let at = self.prev_end;
                let ended = self.source.chars[..at]
                    .iter()
                    .rev()
                    .find(|c| !c.is_whitespace())
                    .is_some_and(|c| *c == ';' || *c == '}');
                self.insert(at, if ended { text } else { format!(";{text}") });
            }
            return Ok(());
        }
        self.export_statement_rest(start)
    }

    fn export_statement_rest(&mut self, start: usize) -> Step<()> {
        let before = self.values;
        match (self.tok.kind, self.tok.text.clone().as_str()) {
            (Kind::Name, "default") => {
                self.advance()?;
                self.kept_module_syntax = true;
                if self.is_n("interface") {
                    return self.interface_statement(start);
                }
                if self.is_n("abstract") {
                    let (from, to) = (self.tok.start, self.tok.end);
                    self.advance()?;
                    self.erase(from, to);
                    self.values += 1;
                    return self.class(true);
                }
                if self.is_n("class") {
                    self.values += 1;
                    return self.class(true);
                }
                if self.is_n("function") {
                    return self.function_statement(start, false, true);
                }
                if self.is_n("async") {
                    let next = self.peek()?;
                    if next.kind == Kind::Name && next.text == "function" && !next.nl {
                        self.advance()?;
                        return self.function_statement(start, true, true);
                    }
                }
                self.values += 1;
                self.assign()?;
                self.semicolon()
            }
            (Kind::Punct, "*") => {
                self.advance()?;
                if self.eat_n("as")? {
                    self.advance()?;
                }
                if !self.eat_n("from")? {
                    return fault(self.tok.start, "`from` was expected.");
                }
                self.module_string()?;
                self.semicolon()?;
                self.kept_module_syntax = true;
                Ok(())
            }
            (Kind::Punct, "{") => self.export_list(start, false),
            (Kind::Punct, "=") => {
                let export_start = start;
                self.advance()?;
                let end = self.prev_end;
                self.replace(export_start, end, "module.exports =".to_string());
                self.assign()?;
                self.semicolon()?;
                self.kept_module_syntax = true;
                Ok(())
            }
            (Kind::Name, "as") => {
                self.advance()?;
                if !self.eat_n("namespace")? {
                    return fault(self.tok.start, "`namespace` was expected.");
                }
                self.expect_name()?;
                self.semicolon()?;
                self.erase_statement(start);
                Ok(())
            }
            (Kind::Name, "type") => {
                let next = self.peek()?;
                if next.kind == Kind::Punct && (next.text == "{" || next.text == "*") {
                    self.advance()?;
                    if self.is_p("{") {
                        self.export_list(start, true)?;
                    } else {
                        self.advance()?;
                        if self.eat_n("as")? {
                            self.advance()?;
                        }
                        if !self.eat_n("from")? {
                            return fault(self.tok.start, "`from` was expected.");
                        }
                        self.module_string()?;
                        self.semicolon()?;
                        self.erase_statement(start);
                    }
                    return Ok(());
                }
                self.declaration(start, true)
            }
            _ => {
                self.declaration(start, true)?;
                if self.values > before {
                    self.kept_module_syntax = true;
                }
                Ok(())
            }
        }
    }

    fn export_list(&mut self, start: usize, type_only: bool) -> Step<()> {
        let open = self.tok.start;
        self.expect_p("{")?;
        let mut specifiers: Vec<ExportName> = Vec::new();
        let mut types_dropped = false;
        while !self.is_p("}") {
            if self.tok.kind == Kind::End {
                return fault(self.tok.start, "An export list is not closed.");
            }
            let spec_start = self.tok.start;
            let mut is_type = false;
            if self.is_n("type") {
                let next = self.peek()?;
                if (next.kind == Kind::Name && next.text != "as") || next.kind == Kind::Str {
                    is_type = true;
                    self.advance()?;
                }
            }
            let local = if self.tok.kind == Kind::Str {
                let text = unquote(&self.tok.text);
                self.advance()?;
                text
            } else {
                self.expect_name()?
            };
            if self.eat_n("as")? {
                if self.tok.kind == Kind::Str {
                    self.advance()?;
                } else {
                    self.expect_name()?;
                }
            }
            let spec_end = self.prev_end;
            if is_type {
                types_dropped = true;
            } else {
                specifiers.push(ExportName {
                    local,
                    text: self.text_between(spec_start, spec_end),
                });
            }
            if !self.eat_p(",")? {
                break;
            }
        }
        let close = self.tok.start;
        self.expect_p("}")?;
        let close_end = self.prev_end;
        let mut has_source = false;
        if self.is_n("from") {
            has_source = true;
            self.advance()?;
            self.module_string()?;
        }
        self.semicolon()?;
        let end = self.prev_end;
        if type_only {
            self.erase(start, end);
            return Ok(());
        }
        let _ = close;
        self.exports.push(ExportList {
            start,
            end,
            open,
            close: close_end,
            specifiers,
            types_dropped,
            has_source,
        });
        Ok(())
    }

    fn program(&mut self) -> Step<()> {
        while self.tok.kind != Kind::End {
            self.statement()?;
        }
        Ok(())
    }

    fn finish(mut self) -> String {
        let mut used: BTreeSet<String> = self.used.iter().cloned().collect();
        for name in &self.top_values {
            used.remove(name);
        }
        let exports = std::mem::take(&mut self.exports);
        for list in &exports {
            let kept: Vec<&ExportName> = list
                .specifiers
                .iter()
                .filter(|one| {
                    list.has_source
                        || !(self.type_names.contains(&one.local)
                            && !self.declared.contains(&one.local))
                })
                .collect();
            if list.has_source && kept.is_empty() {
                self.erase(list.start, list.end);
                continue;
            }
            self.kept_module_syntax = true;
            if !list.has_source {
                for one in &kept {
                    used.insert(one.local.clone());
                }
            }
            if kept.len() != list.specifiers.len() || list.types_dropped {
                let text = format!(
                    "{{ {} }}",
                    kept.iter()
                        .map(|one| one.text.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                let pad = self.newlines_between(list.open, list.close);
                self.replace(list.open, list.close, format!("{text}{}", "\n".repeat(pad)));
            }
        }
        let imports = std::mem::take(&mut self.imports);
        for import in &imports {
            let default = import.default.clone().filter(|one| used.contains(one));
            let namespace = import.namespace.clone().filter(|one| used.contains(one));
            let named: Vec<&(String, String)> = import
                .named
                .iter()
                .filter(|(local, _)| used.contains(local))
                .collect();
            if default.is_none() && namespace.is_none() && named.is_empty() {
                self.erase(import.start, import.end);
                continue;
            }
            self.kept_module_syntax = true;
            if default.as_ref() == import.default.as_ref()
                && namespace.as_ref() == import.namespace.as_ref()
                && named.len() == import.named.len()
                && !import.dropped_types
            {
                continue;
            }
            let mut clause = String::from(" ");
            let mut parts: Vec<String> = Vec::new();
            if let Some(name) = &default {
                parts.push(name.clone());
            }
            if let Some(name) = &namespace {
                parts.push(format!("* as {name}"));
            }
            if import.braces && !named.is_empty() {
                parts.push(format!(
                    "{{ {} }}",
                    named
                        .iter()
                        .map(|(_, text)| text.clone())
                        .collect::<Vec<_>>()
                        .join(", ")
                ));
            }
            clause.push_str(&parts.join(", "));
            clause.push(' ');
            let pad = self.newlines_between(import.clause.0, import.clause.1);
            self.replace(
                import.clause.0,
                import.clause.1,
                format!("{clause}{}", "\n".repeat(pad)),
            );
        }
        let aliases = std::mem::take(&mut self.aliases);
        for alias in &aliases {
            if !alias.exported && !used.contains(&alias.name) {
                self.erase(alias.start, alias.end);
                continue;
            }
            let word = if alias.is_require { "const" } else { "var" };
            self.replace(alias.import_start, alias.import_end, word.to_string());
            if alias.exported {
                self.kept_module_syntax = true;
            }
        }
        let mut text = apply_edits(&self.source.chars, &self.edits);
        if self.saw_module_syntax && !self.kept_module_syntax {
            if !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str("export {};\n");
        }
        text
    }
}

fn apply_edits(chars: &[char], edits: &[Edit]) -> String {
    let mut ordered: Vec<&Edit> = edits.iter().collect();
    ordered.sort_by_key(|one| (one.start, std::cmp::Reverse(one.end)));
    let mut out = String::with_capacity(chars.len());
    let mut cursor = 0usize;
    for edit in ordered {
        if edit.start < cursor {
            continue;
        }
        out.extend(&chars[cursor..edit.start]);
        let original = &chars[edit.start..edit.end];
        match &edit.text {
            None => {
                for character in original {
                    out.push(if is_line_break(*character) {
                        *character
                    } else {
                        ' '
                    });
                }
            }
            Some(text) => {
                out.push_str(text);
                let kept = text.chars().filter(|c| *c == '\n').count();
                let had = original.iter().filter(|c| **c == '\n').count();
                if edit.pad {
                    for _ in kept..had {
                        out.push('\n');
                    }
                }
            }
        }
        cursor = edit.end;
    }
    out.extend(&chars[cursor..]);
    out
}

fn position_of(source: &str, at: usize) -> (u32, u32) {
    let mut line = 1u32;
    let mut column = 1u32;
    for (index, character) in source.chars().enumerate() {
        if index >= at {
            break;
        }
        if character == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

pub fn transpile(source: &str, options: &Options) -> Result<Output, Diagnostic> {
    let outcome = Parser::new(source).and_then(|mut parser| {
        parser.jsx = options.jsx;
        parser.program()?;
        let erased = parser.edits.iter().filter(|one| one.text.is_none()).count();
        Ok((parser.finish(), erased))
    });
    match outcome {
        Ok((code, erased)) => Ok(Output { code, erased }),
        Err(held) => {
            let (line, column) = position_of(source, held.at);
            let base = if held.unsupported {
                error(
                    "EI751",
                    "The TypeScript transpiler cannot handle this construct yet.",
                )
            } else {
                error("EI750", "The TypeScript source has an error.")
            };
            Err(base
                .with_location(Location::at("source.ts", line, column))
                .with_context(format!("Reported: {}", held.message)))
        }
    }
}

pub fn check(source: &str) -> Result<(), Diagnostic> {
    transpile(source, &Options::default()).map(|_| ())
}

use crate::caps::Capability;
use crate::plugin::{
    unimplemented_diagnostic, Context as PluginContext, Contract, Outcome, Plugin, Version,
};

pub static CONTRACT: Contract = Contract {
    id: "plugin.typescript",
    display_name: "TypeScript",
    version: Version::new(0, 1, 0),
    produces_artifacts: false,
    summary:
        "Turns the TypeScript that a web package carries into JavaScript by taking its types out.",
    inputs: &["typescript.source"],
    outputs: &["web.script"],
    required_capabilities: &[Capability::FsRead, Capability::TempStorage],
    forbidden_capabilities: &[
        Capability::Network,
        Capability::Internet,
        Capability::KeyAccess,
        Capability::SensitiveOutput,
    ],
    non_responsibilities: &[
        "Checking types; a program is not refused for a type error.",
        "Fetching declaration files from a registry.",
    ],
};

pub struct TypescriptPlugin;

pub static PLUGIN: TypescriptPlugin = TypescriptPlugin;

impl Plugin for TypescriptPlugin {
    fn contract(&self) -> &'static Contract {
        &CONTRACT
    }

    fn execute(&self, _ctx: &mut PluginContext<'_>) -> Result<Outcome, Diagnostic> {
        Err(unimplemented_diagnostic(&CONTRACT))
    }
}
