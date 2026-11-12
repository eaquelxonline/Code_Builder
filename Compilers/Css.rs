use crate::diag::{Diagnostic, Location, Severity};
use crate::FailureClass;

pub const ORIGIN: &str = "web.css";

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
pub enum Token {
    Ident(String),
    Function(String),
    AtKeyword(String),
    Hash { value: String, id: bool },
    Str(String),
    BadString,
    Url(String),
    BadUrl,
    Delim(char),
    Number { text: String, integer: bool },
    Percentage { text: String },
    Dimension { text: String, unit: String },
    Whitespace,
    Cdo,
    Cdc,
    Colon,
    Semicolon,
    Comma,
    OpenBracket,
    CloseBracket,
    OpenParen,
    CloseParen,
    OpenBrace,
    CloseBrace,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Spanned {
    pub token: Token,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Code {
    Syntax,
    Selector,
    Value,
}

impl Code {
    pub fn name(self) -> String {
        format!("EI{}", 700 + self as u32)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Problem {
    pub line: u32,
    pub column: u32,
    pub code: Code,
    pub message: String,
}

fn is_name_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_' || !character.is_ascii()
}

fn is_name(character: char) -> bool {
    is_name_start(character) || character.is_ascii_digit() || character == '-'
}

fn is_non_printable(character: char) -> bool {
    matches!(character, '\u{0}'..='\u{8}' | '\u{b}' | '\u{e}'..='\u{1f}' | '\u{7f}')
}

fn is_whitespace(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\n')
}

struct Lexer {
    chars: Vec<char>,
    at: usize,
    line: u32,
    column: u32,
    tokens: Vec<Spanned>,
    problems: Vec<Problem>,
}

pub fn tokenize(source: &str) -> (Vec<Spanned>, Vec<Problem>) {
    let mut chars: Vec<char> = Vec::with_capacity(source.len());
    let mut held = source.chars().peekable();
    while let Some(character) = held.next() {
        match character {
            '\r' => {
                if held.peek() == Some(&'\n') {
                    held.next();
                }
                chars.push('\n');
            }
            '\u{c}' => chars.push('\n'),
            '\u{0}' => chars.push('\u{fffd}'),
            other => chars.push(other),
        }
    }
    let mut lexer = Lexer {
        chars,
        at: 0,
        line: 1,
        column: 1,
        tokens: Vec::new(),
        problems: Vec::new(),
    };
    lexer.run();
    (lexer.tokens, lexer.problems)
}

impl Lexer {
    fn peek(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.at + ahead).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let character = self.chars.get(self.at).copied()?;
        self.at += 1;
        if character == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(character)
    }

    fn problem(&mut self, line: u32, column: u32, code: Code, message: &str) {
        self.problems.push(Problem {
            line,
            column,
            code,
            message: message.to_string(),
        });
    }

    fn valid_escape(&self, ahead: usize) -> bool {
        self.peek(ahead) == Some('\\') && self.peek(ahead + 1).is_some_and(|next| next != '\n')
    }

    fn would_start_ident(&self, ahead: usize) -> bool {
        match self.peek(ahead) {
            Some('-') => match self.peek(ahead + 1) {
                Some(next) if is_name_start(next) || next == '-' => true,
                Some('\\') => self.valid_escape(ahead + 1),
                _ => false,
            },
            Some(first) if is_name_start(first) => true,
            Some('\\') => self.valid_escape(ahead),
            _ => false,
        }
    }

    fn would_start_number(&self, ahead: usize) -> bool {
        match self.peek(ahead) {
            Some('+' | '-') => match self.peek(ahead + 1) {
                Some(next) if next.is_ascii_digit() => true,
                Some('.') => self
                    .peek(ahead + 2)
                    .is_some_and(|next| next.is_ascii_digit()),
                _ => false,
            },
            Some('.') => self
                .peek(ahead + 1)
                .is_some_and(|next| next.is_ascii_digit()),
            Some(first) => first.is_ascii_digit(),
            None => false,
        }
    }

    fn consume_escape(&mut self) -> char {
        let mut value = 0u32;
        let mut digits = 0;
        while digits < 6 {
            match self.peek(0) {
                Some(next) if next.is_ascii_hexdigit() => {
                    value = value * 16 + next.to_digit(16).unwrap_or(0);
                    self.bump();
                    digits += 1;
                }
                _ => break,
            }
        }
        if digits == 0 {
            return self.bump().unwrap_or('\u{fffd}');
        }
        if self.peek(0).is_some_and(is_whitespace) {
            self.bump();
        }
        if value == 0 || (0xd800..=0xdfff).contains(&value) || value > 0x10ffff {
            '\u{fffd}'
        } else {
            char::from_u32(value).unwrap_or('\u{fffd}')
        }
    }

    fn consume_name(&mut self) -> String {
        let mut out = String::new();
        loop {
            match self.peek(0) {
                Some(next) if is_name(next) => {
                    out.push(next);
                    self.bump();
                }
                Some('\\') if self.valid_escape(0) => {
                    self.bump();
                    out.push(self.consume_escape());
                }
                _ => break,
            }
        }
        out
    }

    fn consume_number(&mut self) -> (String, bool) {
        let mut text = String::new();
        let mut integer = true;
        if matches!(self.peek(0), Some('+' | '-')) {
            if let Some(sign) = self.bump() {
                text.push(sign);
            }
        }
        while self.peek(0).is_some_and(|next| next.is_ascii_digit()) {
            text.extend(self.bump());
        }
        if self.peek(0) == Some('.') && self.peek(1).is_some_and(|next| next.is_ascii_digit()) {
            text.extend(self.bump());
            integer = false;
            while self.peek(0).is_some_and(|next| next.is_ascii_digit()) {
                text.extend(self.bump());
            }
        }
        if matches!(self.peek(0), Some('e' | 'E')) {
            let signed = matches!(self.peek(1), Some('+' | '-'))
                && self.peek(2).is_some_and(|next| next.is_ascii_digit());
            let plain = self.peek(1).is_some_and(|next| next.is_ascii_digit());
            if signed || plain {
                text.extend(self.bump());
                if signed {
                    text.extend(self.bump());
                }
                integer = false;
                while self.peek(0).is_some_and(|next| next.is_ascii_digit()) {
                    text.extend(self.bump());
                }
            }
        }
        (text, integer)
    }

    fn consume_numeric(&mut self) -> Token {
        let (text, integer) = self.consume_number();
        if self.would_start_ident(0) {
            let unit = self.consume_name();
            return Token::Dimension { text, unit };
        }
        if self.peek(0) == Some('%') {
            self.bump();
            return Token::Percentage { text };
        }
        Token::Number { text, integer }
    }

    fn consume_string(&mut self, ending: char, line: u32, column: u32) -> Token {
        let mut out = String::new();
        loop {
            match self.peek(0) {
                None => {
                    self.problem(line, column, Code::Syntax, "A string is not closed.");
                    return Token::Str(out);
                }
                Some(next) if next == ending => {
                    self.bump();
                    return Token::Str(out);
                }
                Some('\n') => {
                    self.problem(
                        line,
                        column,
                        Code::Value,
                        "A string breaks at the end of a line.",
                    );
                    return Token::BadString;
                }
                Some('\\') => {
                    self.bump();
                    match self.peek(0) {
                        None => {}
                        Some('\n') => {
                            self.bump();
                        }
                        Some(_) => out.push(self.consume_escape()),
                    }
                }
                Some(next) => {
                    out.push(next);
                    self.bump();
                }
            }
        }
    }

    fn consume_bad_url_remnants(&mut self) {
        loop {
            match self.peek(0) {
                None => return,
                Some(')') => {
                    self.bump();
                    return;
                }
                Some('\\') if self.valid_escape(0) => {
                    self.bump();
                    self.consume_escape();
                }
                Some(_) => {
                    self.bump();
                }
            }
        }
    }

    fn consume_url(&mut self, line: u32, column: u32) -> Token {
        while self.peek(0).is_some_and(is_whitespace) {
            self.bump();
        }
        let mut out = String::new();
        loop {
            match self.peek(0) {
                None => {
                    self.problem(line, column, Code::Syntax, "A url is not closed.");
                    return Token::Url(out);
                }
                Some(')') => {
                    self.bump();
                    return Token::Url(out);
                }
                Some(next) if is_whitespace(next) => {
                    while self.peek(0).is_some_and(is_whitespace) {
                        self.bump();
                    }
                    match self.peek(0) {
                        None => {
                            self.problem(line, column, Code::Syntax, "A url is not closed.");
                            return Token::Url(out);
                        }
                        Some(')') => {
                            self.bump();
                            return Token::Url(out);
                        }
                        Some(_) => {
                            self.consume_bad_url_remnants();
                            self.problem(
                                line,
                                column,
                                Code::Value,
                                "A url holds a stray character.",
                            );
                            return Token::BadUrl;
                        }
                    }
                }
                Some('"' | '\'' | '(') => {
                    self.consume_bad_url_remnants();
                    self.problem(line, column, Code::Value, "A url holds a stray character.");
                    return Token::BadUrl;
                }
                Some(next) if is_non_printable(next) => {
                    self.consume_bad_url_remnants();
                    self.problem(line, column, Code::Value, "A url holds a stray character.");
                    return Token::BadUrl;
                }
                Some('\\') => {
                    if self.valid_escape(0) {
                        self.bump();
                        out.push(self.consume_escape());
                    } else {
                        self.consume_bad_url_remnants();
                        self.problem(line, column, Code::Value, "A url holds a stray character.");
                        return Token::BadUrl;
                    }
                }
                Some(next) => {
                    out.push(next);
                    self.bump();
                }
            }
        }
    }

    fn consume_ident_like(&mut self, line: u32, column: u32) -> Token {
        let name = self.consume_name();
        if self.peek(0) == Some('(') {
            self.bump();
            if name.eq_ignore_ascii_case("url") {
                let mut look = 0;
                while self.peek(look).is_some_and(is_whitespace) {
                    look += 1;
                }
                if matches!(self.peek(look), Some('"' | '\'')) {
                    return Token::Function(name);
                }
                return self.consume_url(line, column);
            }
            return Token::Function(name);
        }
        Token::Ident(name)
    }

    fn run(&mut self) {
        loop {
            let line = self.line;
            let column = self.column;
            let Some(first) = self.peek(0) else {
                return;
            };
            let token = match first {
                character if is_whitespace(character) => {
                    while self.peek(0).is_some_and(is_whitespace) {
                        self.bump();
                    }
                    Token::Whitespace
                }
                '/' if self.peek(1) == Some('*') => {
                    self.bump();
                    self.bump();
                    loop {
                        match self.peek(0) {
                            None => {
                                self.problem(
                                    line,
                                    column,
                                    Code::Syntax,
                                    "A comment is not closed.",
                                );
                                break;
                            }
                            Some('*') if self.peek(1) == Some('/') => {
                                self.bump();
                                self.bump();
                                break;
                            }
                            Some(_) => {
                                self.bump();
                            }
                        }
                    }
                    continue;
                }
                '"' | '\'' => {
                    self.bump();
                    self.consume_string(first, line, column)
                }
                '#' => {
                    self.bump();
                    if self.peek(0).is_some_and(is_name) || self.valid_escape(0) {
                        let id = self.would_start_ident(0);
                        Token::Hash {
                            value: self.consume_name(),
                            id,
                        }
                    } else {
                        Token::Delim('#')
                    }
                }
                '(' => {
                    self.bump();
                    Token::OpenParen
                }
                ')' => {
                    self.bump();
                    Token::CloseParen
                }
                '[' => {
                    self.bump();
                    Token::OpenBracket
                }
                ']' => {
                    self.bump();
                    Token::CloseBracket
                }
                '{' => {
                    self.bump();
                    Token::OpenBrace
                }
                '}' => {
                    self.bump();
                    Token::CloseBrace
                }
                ',' => {
                    self.bump();
                    Token::Comma
                }
                ':' => {
                    self.bump();
                    Token::Colon
                }
                ';' => {
                    self.bump();
                    Token::Semicolon
                }
                '+' | '.' => {
                    if self.would_start_number(0) {
                        self.consume_numeric()
                    } else {
                        self.bump();
                        Token::Delim(first)
                    }
                }
                '-' => {
                    if self.would_start_number(0) {
                        self.consume_numeric()
                    } else if self.peek(1) == Some('-') && self.peek(2) == Some('>') {
                        self.bump();
                        self.bump();
                        self.bump();
                        Token::Cdc
                    } else if self.would_start_ident(0) {
                        self.consume_ident_like(line, column)
                    } else {
                        self.bump();
                        Token::Delim('-')
                    }
                }
                '<' => {
                    if self.peek(1) == Some('!')
                        && self.peek(2) == Some('-')
                        && self.peek(3) == Some('-')
                    {
                        for _ in 0..4 {
                            self.bump();
                        }
                        Token::Cdo
                    } else {
                        self.bump();
                        Token::Delim('<')
                    }
                }
                '@' => {
                    self.bump();
                    if self.would_start_ident(0) {
                        Token::AtKeyword(self.consume_name())
                    } else {
                        Token::Delim('@')
                    }
                }
                '\\' => {
                    if self.valid_escape(0) {
                        self.consume_ident_like(line, column)
                    } else {
                        self.bump();
                        self.problem(line, column, Code::Syntax, "A backslash escapes nothing.");
                        Token::Delim('\\')
                    }
                }
                character if character.is_ascii_digit() => self.consume_numeric(),
                character if is_name_start(character) => self.consume_ident_like(line, column),
                other => {
                    self.bump();
                    Token::Delim(other)
                }
            };
            self.tokens.push(Spanned {
                token,
                line,
                column,
            });
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Component {
    Token(Token),
    Function {
        name: String,
        arguments: Vec<Component>,
    },
    Parenthesis(Vec<Component>),
    Bracket(Vec<Component>),
    Brace(Vec<Component>),
}

impl Component {
    fn is_whitespace(&self) -> bool {
        matches!(self, Component::Token(Token::Whitespace))
    }

    fn delim(&self, wanted: char) -> bool {
        matches!(self, Component::Token(Token::Delim(found)) if *found == wanted)
    }

    fn ident(&self) -> Option<&str> {
        match self {
            Component::Token(Token::Ident(name)) => Some(name),
            _ => None,
        }
    }
}

struct Tree<'a> {
    tokens: &'a [Spanned],
    at: usize,
    problems: Vec<Problem>,
}

fn closing_of(token: &Token) -> Option<Token> {
    match token {
        Token::OpenParen | Token::Function(_) => Some(Token::CloseParen),
        Token::OpenBracket => Some(Token::CloseBracket),
        Token::OpenBrace => Some(Token::CloseBrace),
        _ => None,
    }
}

impl Tree<'_> {
    fn component_list(&mut self, ending: Option<&Token>) -> Vec<Component> {
        let mut out = Vec::new();
        while let Some(held) = self.tokens.get(self.at) {
            if Some(&held.token) == ending {
                self.at += 1;
                return out;
            }
            let line = held.line;
            let column = held.column;
            let token = held.token.clone();
            self.at += 1;
            match closing_of(&token) {
                Some(close) => {
                    let inner = self.component_list(Some(&close));
                    if !self.closed(&close) {
                        self.problems.push(Problem {
                            line,
                            column,
                            code: Code::Syntax,
                            message: "A bracket is not closed.".to_string(),
                        });
                    }
                    out.push(match token {
                        Token::Function(name) => Component::Function {
                            name,
                            arguments: inner,
                        },
                        Token::OpenParen => Component::Parenthesis(inner),
                        Token::OpenBracket => Component::Bracket(inner),
                        _ => Component::Brace(inner),
                    });
                }
                None => {
                    if matches!(
                        token,
                        Token::CloseParen | Token::CloseBracket | Token::CloseBrace
                    ) && ending.is_some()
                    {
                        self.at -= 1;
                        return out;
                    }
                    out.push(Component::Token(token));
                }
            }
        }
        out
    }

    fn closed(&mut self, close: &Token) -> bool {
        self.at
            .checked_sub(1)
            .and_then(|back| self.tokens.get(back))
            .is_some_and(|held| held.token == *close)
    }
}

fn components_of(tokens: &[Spanned]) -> (Vec<Component>, Vec<Problem>) {
    let mut tree = Tree {
        tokens,
        at: 0,
        problems: Vec::new(),
    };
    let mut out = Vec::new();
    while tree.at < tokens.len() {
        let before = tree.at;
        let mut part = tree.component_list(None);
        out.append(&mut part);
        if tree.at == before {
            tree.at += 1;
        }
    }
    (out, tree.problems)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    pub name: String,
    pub value: Vec<Component>,
    pub important: bool,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    Declaration(Declaration),
    Rule(Rule),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Block {
    Items(Vec<Item>),
    Raw(Vec<Component>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct AtRule {
    pub name: String,
    pub prelude: Vec<Component>,
    pub block: Option<Block>,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StyleRule {
    pub prelude: Vec<Component>,
    pub selectors: Option<Vec<Selector>>,
    pub items: Vec<Item>,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Rule {
    Style(StyleRule),
    At(AtRule),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stylesheet {
    pub rules: Vec<Rule>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Combinator {
    None,
    Descendant,
    Child,
    NextSibling,
    SubsequentSibling,
    Column,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttributeOperator {
    Equals,
    Includes,
    DashMatch,
    Prefix,
    Suffix,
    Substring,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PseudoArguments {
    None,
    Selectors(Vec<Selector>),
    Nth {
        a: i64,
        b: i64,
        of: Option<Vec<Selector>>,
    },
    Raw(Vec<Component>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Simple {
    Type {
        namespace: Option<String>,
        name: String,
    },
    Universal {
        namespace: Option<String>,
    },
    Id(String),
    Class(String),
    Attribute {
        namespace: Option<String>,
        name: String,
        matcher: Option<(AttributeOperator, String)>,
        modifier: Option<char>,
    },
    PseudoClass {
        name: String,
        arguments: PseudoArguments,
    },
    PseudoElement {
        name: String,
        arguments: PseudoArguments,
    },
    Nesting,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Selector {
    pub parts: Vec<(Combinator, Vec<Simple>)>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Specificity {
    pub ids: u32,
    pub classes: u32,
    pub types: u32,
}

impl Specificity {
    fn add(self, other: Specificity) -> Specificity {
        Specificity {
            ids: self.ids + other.ids,
            classes: self.classes + other.classes,
            types: self.types + other.types,
        }
    }
}

fn split_on_commas(components: &[Component]) -> Vec<&[Component]> {
    let mut out = Vec::new();
    let mut from = 0;
    for (at, held) in components.iter().enumerate() {
        if matches!(held, Component::Token(Token::Comma)) {
            out.push(&components[from..at]);
            from = at + 1;
        }
    }
    out.push(&components[from..]);
    out
}

fn trimmed(components: &[Component]) -> &[Component] {
    let mut from = 0;
    let mut to = components.len();
    while from < to && components[from].is_whitespace() {
        from += 1;
    }
    while to > from && components[to - 1].is_whitespace() {
        to -= 1;
    }
    &components[from..to]
}

pub fn parse_selector_list(components: &[Component]) -> Result<Vec<Selector>, String> {
    let mut out = Vec::new();
    for part in split_on_commas(components) {
        let part = trimmed(part);
        if part.is_empty() {
            return Err("A selector is empty.".to_string());
        }
        out.push(parse_complex(part)?);
    }
    Ok(out)
}

fn combinator_here(components: &[Component], at: usize) -> Option<(Combinator, usize)> {
    match components.get(at)? {
        component if component.delim('>') => Some((Combinator::Child, 1)),
        component if component.delim('+') => Some((Combinator::NextSibling, 1)),
        component if component.delim('~') => Some((Combinator::SubsequentSibling, 1)),
        component
            if component.delim('|')
                && components.get(at + 1).is_some_and(|next| next.delim('|')) =>
        {
            Some((Combinator::Column, 2))
        }
        _ => None,
    }
}

fn parse_complex(components: &[Component]) -> Result<Selector, String> {
    let mut parts: Vec<(Combinator, Vec<Simple>)> = Vec::new();
    let mut at = 0;
    let mut pending = Combinator::None;
    while at < components.len() {
        if components[at].is_whitespace() {
            at += 1;
            continue;
        }
        if let Some((combinator, width)) = combinator_here(components, at) {
            if pending != Combinator::None && pending != Combinator::Descendant {
                return Err("Two combinators follow each other.".to_string());
            }
            pending = combinator;
            at += width;
            continue;
        }
        let (compound, next) = parse_compound(components, at)?;
        if compound.is_empty() {
            return Err("A selector holds something that is not a selector.".to_string());
        }
        let how = if parts.is_empty() || pending != Combinator::None {
            pending
        } else {
            Combinator::Descendant
        };
        parts.push((how, compound));
        pending = Combinator::None;
        at = next;
    }
    if pending != Combinator::None {
        return Err("A selector ends in a combinator.".to_string());
    }
    if parts.is_empty() {
        return Err("A selector is empty.".to_string());
    }
    Ok(Selector { parts })
}

fn namespaced(components: &[Component], at: usize) -> Option<(Option<String>, usize)> {
    let first = components.get(at)?;
    let named = match first {
        Component::Token(Token::Ident(name)) => Some(name.clone()),
        component if component.delim('*') => Some("*".to_string()),
        _ => None,
    };
    if let Some(prefix) = named {
        if components.get(at + 1).is_some_and(|next| next.delim('|'))
            && !components
                .get(at + 2)
                .is_some_and(|next| next.delim('|') || next.delim('='))
        {
            return Some((Some(prefix), at + 2));
        }
        return Some((None, at));
    }
    if first.delim('|') && !components.get(at + 1).is_some_and(|next| next.delim('|')) {
        return Some((Some(String::new()), at + 1));
    }
    None
}

fn parse_compound(components: &[Component], start: usize) -> Result<(Vec<Simple>, usize), String> {
    let mut out = Vec::new();
    let mut at = start;
    if let Some((namespace, next)) = namespaced(components, at) {
        match components.get(next) {
            Some(Component::Token(Token::Ident(name))) => {
                out.push(Simple::Type {
                    namespace,
                    name: name.clone(),
                });
                at = next + 1;
            }
            Some(component) if component.delim('*') => {
                out.push(Simple::Universal { namespace });
                at = next + 1;
            }
            _ => return Err("A namespace prefix names nothing.".to_string()),
        }
    }
    while let Some(held) = components.get(at) {
        match held {
            Component::Token(Token::Hash { value, id: true }) => {
                out.push(Simple::Id(value.clone()));
                at += 1;
            }
            Component::Token(Token::Hash { .. }) => {
                return Err("An identifier selector is not a valid name.".to_string());
            }
            component if component.delim('.') => match components.get(at + 1) {
                Some(Component::Token(Token::Ident(name))) => {
                    out.push(Simple::Class(name.clone()));
                    at += 2;
                }
                _ => return Err("A class selector has no name.".to_string()),
            },
            component if component.delim('&') => {
                out.push(Simple::Nesting);
                at += 1;
            }
            Component::Bracket(inner) => {
                out.push(parse_attribute(inner)?);
                at += 1;
            }
            Component::Token(Token::Colon) => {
                let element =
                    matches!(components.get(at + 1), Some(Component::Token(Token::Colon)));
                let from = if element { at + 2 } else { at + 1 };
                match components.get(from) {
                    Some(Component::Token(Token::Ident(name))) => {
                        let name = name.to_ascii_lowercase();
                        let legacy = matches!(
                            name.as_str(),
                            "before" | "after" | "first-line" | "first-letter"
                        );
                        if element || legacy {
                            out.push(Simple::PseudoElement {
                                name,
                                arguments: PseudoArguments::None,
                            });
                        } else {
                            out.push(Simple::PseudoClass {
                                name,
                                arguments: PseudoArguments::None,
                            });
                        }
                        at = from + 1;
                    }
                    Some(Component::Function { name, arguments }) => {
                        let name = name.to_ascii_lowercase();
                        let parsed = parse_pseudo_arguments(&name, arguments)?;
                        if element {
                            out.push(Simple::PseudoElement {
                                name,
                                arguments: parsed,
                            });
                        } else {
                            out.push(Simple::PseudoClass {
                                name,
                                arguments: parsed,
                            });
                        }
                        at = from + 1;
                    }
                    _ => return Err("A pseudo selector has no name.".to_string()),
                }
            }
            _ => break,
        }
    }
    Ok((out, at))
}

fn parse_attribute(inner: &[Component]) -> Result<Simple, String> {
    let inner = trimmed(inner);
    let (namespace, at) = match namespaced(inner, 0) {
        Some((namespace, next)) => (namespace, next),
        None => (None, 0),
    };
    let name = match inner.get(at) {
        Some(Component::Token(Token::Ident(name))) => name.clone(),
        _ => return Err("An attribute selector has no name.".to_string()),
    };
    let mut at = at + 1;
    while inner.get(at).is_some_and(Component::is_whitespace) {
        at += 1;
    }
    if at >= inner.len() {
        return Ok(Simple::Attribute {
            namespace,
            name,
            matcher: None,
            modifier: None,
        });
    }
    let (operator, width) = match inner.get(at) {
        Some(component) if component.delim('=') => (AttributeOperator::Equals, 1),
        Some(component) => {
            let which = ['~', '|', '^', '$', '*']
                .into_iter()
                .find(|mark| component.delim(*mark))
                .ok_or_else(|| "An attribute selector has an unknown operator.".to_string())?;
            if !inner.get(at + 1).is_some_and(|next| next.delim('=')) {
                return Err("An attribute operator is not followed by `=`.".to_string());
            }
            (
                match which {
                    '~' => AttributeOperator::Includes,
                    '|' => AttributeOperator::DashMatch,
                    '^' => AttributeOperator::Prefix,
                    '$' => AttributeOperator::Suffix,
                    _ => AttributeOperator::Substring,
                },
                2,
            )
        }
        None => return Err("An attribute selector ends early.".to_string()),
    };
    at += width;
    while inner.get(at).is_some_and(Component::is_whitespace) {
        at += 1;
    }
    let value = match inner.get(at) {
        Some(Component::Token(Token::Ident(text) | Token::Str(text))) => text.clone(),
        _ => return Err("An attribute selector has no value.".to_string()),
    };
    at += 1;
    while inner.get(at).is_some_and(Component::is_whitespace) {
        at += 1;
    }
    let modifier = match inner.get(at) {
        Some(Component::Token(Token::Ident(text))) if text.len() == 1 => text.chars().next(),
        None => None,
        _ => return Err("An attribute selector holds something after its value.".to_string()),
    };
    Ok(Simple::Attribute {
        namespace,
        name,
        matcher: Some((operator, value)),
        modifier,
    })
}

fn parse_nth(text: &str) -> Option<(i64, i64)> {
    let cleaned: String = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase();
    match cleaned.as_str() {
        "even" => return Some((2, 0)),
        "odd" => return Some((2, 1)),
        _ => {}
    }
    let Some(n_at) = cleaned.find('n') else {
        return cleaned.parse::<i64>().ok().map(|b| (0, b));
    };
    let before = &cleaned[..n_at];
    let after = &cleaned[n_at + 1..];
    let a = match before {
        "" | "+" => 1,
        "-" => -1,
        other => other.parse::<i64>().ok()?,
    };
    let b = if after.is_empty() {
        0
    } else {
        let signed = after.strip_prefix('+').unwrap_or(after);
        if !(after.starts_with('+') || after.starts_with('-')) {
            return None;
        }
        signed.parse::<i64>().ok()?
    };
    Some((a, b))
}

fn parse_pseudo_arguments(name: &str, arguments: &[Component]) -> Result<PseudoArguments, String> {
    match name {
        "is" | "where" | "not" | "has" | "matches" | "-webkit-any" | "-moz-any" | "host"
        | "host-context" | "slotted" | "cue" | "cue-region" | "current" | "past" | "future" => {
            match parse_selector_list(arguments) {
                Ok(list) => Ok(PseudoArguments::Selectors(list)),
                Err(_)
                    if matches!(
                        name,
                        "host"
                            | "host-context"
                            | "slotted"
                            | "cue"
                            | "cue-region"
                            | "current"
                            | "past"
                            | "future"
                    ) =>
                {
                    Ok(PseudoArguments::Raw(arguments.to_vec()))
                }
                Err(why) => Err(why),
            }
        }
        "nth-child" | "nth-last-child" | "nth-of-type" | "nth-last-of-type" | "nth-col"
        | "nth-last-col" => {
            let mut of_at = None;
            for (at, held) in arguments.iter().enumerate() {
                if held
                    .ident()
                    .is_some_and(|word| word.eq_ignore_ascii_case("of"))
                {
                    of_at = Some(at);
                    break;
                }
            }
            let (head, tail) = match of_at {
                Some(at) => (&arguments[..at], Some(&arguments[at + 1..])),
                None => (arguments, None),
            };
            let text = serialize_components(head, false);
            let Some((a, b)) = parse_nth(&text) else {
                return Err(
                    "An `an+b` expression is not written the way CSS writes it.".to_string()
                );
            };
            let of = match tail {
                Some(rest) => Some(parse_selector_list(rest)?),
                None => None,
            };
            Ok(PseudoArguments::Nth { a, b, of })
        }
        _ => Ok(PseudoArguments::Raw(arguments.to_vec())),
    }
}

impl Selector {
    pub fn specificity(&self) -> Specificity {
        self.specificity_within(Specificity::default())
    }

    pub fn specificity_within(&self, parent: Specificity) -> Specificity {
        let mut total = Specificity::default();
        for (_, compound) in &self.parts {
            for simple in compound {
                total = total.add(simple_specificity(simple, parent));
            }
        }
        total
    }

    pub fn has_nesting(&self) -> bool {
        self.parts.iter().any(|(_, compound)| {
            compound
                .iter()
                .any(|simple| matches!(simple, Simple::Nesting))
        })
    }
}

fn most_specific(list: &[Selector], parent: Specificity) -> Specificity {
    list.iter()
        .map(|one| one.specificity_within(parent))
        .max()
        .unwrap_or_default()
}

fn simple_specificity(simple: &Simple, parent: Specificity) -> Specificity {
    let one_class = Specificity {
        ids: 0,
        classes: 1,
        types: 0,
    };
    match simple {
        Simple::Id(_) => Specificity {
            ids: 1,
            classes: 0,
            types: 0,
        },
        Simple::Class(_) | Simple::Attribute { .. } => one_class,
        Simple::Type { .. } => Specificity {
            ids: 0,
            classes: 0,
            types: 1,
        },
        Simple::Universal { .. } => Specificity::default(),
        Simple::Nesting => parent,
        Simple::PseudoElement { .. } => Specificity {
            ids: 0,
            classes: 0,
            types: 1,
        },
        Simple::PseudoClass { name, arguments } => match (name.as_str(), arguments) {
            ("where", _) => Specificity::default(),
            (
                "is" | "not" | "has" | "matches" | "-webkit-any" | "-moz-any",
                PseudoArguments::Selectors(list),
            ) => most_specific(list, parent),
            ("nth-child" | "nth-last-child", PseudoArguments::Nth { of: Some(list), .. }) => {
                one_class.add(most_specific(list, parent))
            }
            ("host", PseudoArguments::Selectors(list)) => {
                one_class.add(most_specific(list, parent))
            }
            _ => one_class,
        },
    }
}

const BLOCK_OF_ITEMS: &[&str] = &[
    "media",
    "supports",
    "layer",
    "container",
    "scope",
    "document",
    "-moz-document",
    "starting-style",
    "font-face",
    "page",
    "property",
    "counter-style",
    "font-feature-values",
    "font-palette-values",
    "keyframes",
    "-webkit-keyframes",
    "-moz-keyframes",
    "-o-keyframes",
    "viewport",
    "-ms-viewport",
    "position-try",
    "view-transition",
    "top-left",
    "top-center",
    "top-right",
    "bottom-left",
    "bottom-center",
    "bottom-right",
    "left-top",
    "left-middle",
    "left-bottom",
    "right-top",
    "right-middle",
    "right-bottom",
    "stylistic",
    "styleset",
    "character-variant",
    "swash",
    "ornaments",
    "annotation",
    "nest",
    "custom-media-block",
];

fn is_keyframes(name: &str) -> bool {
    matches!(
        name,
        "keyframes" | "-webkit-keyframes" | "-moz-keyframes" | "-o-keyframes"
    )
}

struct Parser {
    problems: Vec<Problem>,
}

pub fn parse_stylesheet_forgiving(source: &str) -> (Stylesheet, Vec<Problem>) {
    let (tokens, mut problems) = tokenize(source);
    let (components, mut tree_problems) = components_of(&tokens);
    problems.append(&mut tree_problems);
    let mut parser = Parser { problems };
    let positions = Positions::of(&tokens);
    let mut rules = Vec::new();
    parser.rules(&components, &positions, 0, false, &mut rules);
    let mut problems = parser.problems;
    problems.sort_by_key(|one| (one.line, one.column));
    problems.dedup();
    (Stylesheet { rules }, problems)
}

struct Positions {
    at: Vec<(u32, u32)>,
}

impl Positions {
    fn of(tokens: &[Spanned]) -> Positions {
        Positions {
            at: tokens.iter().map(|held| (held.line, held.column)).collect(),
        }
    }

    fn start(&self, ordinal: usize) -> (u32, u32) {
        self.at
            .get(ordinal.min(self.at.len().saturating_sub(1)))
            .copied()
            .unwrap_or((1, 1))
    }
}

fn weight(components: &[Component]) -> usize {
    components
        .iter()
        .map(|held| match held {
            Component::Token(_) => 1,
            Component::Function { arguments, .. } => 2 + weight(arguments),
            Component::Parenthesis(inner) | Component::Bracket(inner) | Component::Brace(inner) => {
                2 + weight(inner)
            }
        })
        .sum()
}

impl Parser {
    fn note(&mut self, line: u32, column: u32, code: Code, message: &str) {
        self.problems.push(Problem {
            line,
            column,
            code,
            message: message.to_string(),
        });
    }

    fn rules(
        &mut self,
        components: &[Component],
        positions: &Positions,
        base: usize,
        nested: bool,
        out: &mut Vec<Rule>,
    ) {
        let mut items = Vec::new();
        self.items(components, positions, base, false, nested, &mut items);
        for one in items {
            match one {
                Item::Rule(rule) => out.push(rule),
                Item::Declaration(held) => {
                    self.note(
                        held.line,
                        held.column,
                        Code::Syntax,
                        "A declaration stands outside any rule.",
                    );
                }
            }
        }
    }

    fn items(
        &mut self,
        components: &[Component],
        positions: &Positions,
        base: usize,
        keyframes: bool,
        nested: bool,
        out: &mut Vec<Item>,
    ) {
        let mut at = 0usize;
        let mut ordinal = base;
        while at < components.len() {
            let held = &components[at];
            if held.is_whitespace()
                || matches!(
                    held,
                    Component::Token(Token::Semicolon | Token::Cdo | Token::Cdc)
                )
            {
                ordinal += 1;
                at += 1;
                continue;
            }
            let (line, column) = positions.start(ordinal);
            if let Component::Token(Token::AtKeyword(name)) = held {
                let name = name.to_ascii_lowercase();
                let mut end = at + 1;
                while end < components.len()
                    && !matches!(
                        components[end],
                        Component::Token(Token::Semicolon) | Component::Brace(_)
                    )
                {
                    end += 1;
                }
                let prelude = trimmed(&components[at + 1..end]).to_vec();
                let before_block = ordinal + weight(&components[at..end]);
                let mut block = None;
                if let Some(Component::Brace(inner)) = components.get(end) {
                    block = Some(self.block_of(&name, inner, positions, before_block + 1, nested));
                }
                out.push(Item::Rule(Rule::At(AtRule {
                    name,
                    prelude,
                    block,
                    line,
                    column,
                })));
                let next = (end + 1).min(components.len());
                ordinal += weight(&components[at..next]);
                at = next;
                continue;
            }
            let mut end = at;
            let mut custom = false;
            if let Some(name) = held.ident() {
                if name.starts_with("--") {
                    let mut look = at + 1;
                    while components.get(look).is_some_and(Component::is_whitespace) {
                        look += 1;
                    }
                    custom = matches!(components.get(look), Some(Component::Token(Token::Colon)));
                }
            }
            while end < components.len() {
                match &components[end] {
                    Component::Token(Token::Semicolon) => break,
                    Component::Brace(_) if !custom => break,
                    _ => end += 1,
                }
            }
            if let Some(Component::Brace(inner)) = components.get(end).filter(|_| !custom) {
                let prelude = trimmed(&components[at..end]).to_vec();
                let before_block = ordinal + weight(&components[at..end]);
                let next = end + 1;
                let span = weight(&components[at..next]);
                let selectors = if keyframes {
                    None
                } else {
                    match parse_selector_list(&prelude) {
                        Ok(list) => Some(list),
                        Err(why) => {
                            self.note(line, column, Code::Selector, &why);
                            ordinal += span;
                            at = next;
                            continue;
                        }
                    }
                };
                let mut inner_items = Vec::new();
                self.items(
                    inner,
                    positions,
                    before_block + 1,
                    false,
                    true,
                    &mut inner_items,
                );
                out.push(Item::Rule(Rule::Style(StyleRule {
                    prelude,
                    selectors,
                    items: inner_items,
                    line,
                    column,
                })));
                ordinal += span;
                at = next;
                continue;
            }
            match parse_declaration(&components[at..end], line, column) {
                Ok(declaration) => out.push(Item::Declaration(declaration)),
                Err(why) => self.note(line, column, Code::Syntax, &why),
            }
            let next = end.max(at + 1);
            ordinal += weight(&components[at..next]);
            at = next;
        }
        let _ = nested;
    }

    fn block_of(
        &mut self,
        name: &str,
        inner: &[Component],
        positions: &Positions,
        base: usize,
        nested: bool,
    ) -> Block {
        if BLOCK_OF_ITEMS.contains(&name) {
            let mut items = Vec::new();
            self.items(
                inner,
                positions,
                base,
                is_keyframes(name),
                nested,
                &mut items,
            );
            Block::Items(items)
        } else {
            Block::Raw(inner.to_vec())
        }
    }
}

fn parse_declaration(slice: &[Component], line: u32, column: u32) -> Result<Declaration, String> {
    let slice = trimmed(slice);
    let Some(first) = slice.first() else {
        return Err("A declaration is empty.".to_string());
    };
    let Some(name) = first.ident() else {
        return Err("A declaration does not start with a property name.".to_string());
    };
    let mut at = 1;
    while slice.get(at).is_some_and(Component::is_whitespace) {
        at += 1;
    }
    if !matches!(slice.get(at), Some(Component::Token(Token::Colon))) {
        return Err("A declaration has no colon after its property name.".to_string());
    }
    let mut value: Vec<Component> = trimmed(&slice[at + 1..]).to_vec();
    let mut important = false;
    let mut end = value.len();
    while end > 0 && value[end - 1].is_whitespace() {
        end -= 1;
    }
    if end > 0
        && value[end - 1]
            .ident()
            .is_some_and(|word| word.eq_ignore_ascii_case("important"))
    {
        let mut before = end - 1;
        while before > 0 && value[before - 1].is_whitespace() {
            before -= 1;
        }
        if before > 0 && value[before - 1].delim('!') {
            important = true;
            value.truncate(before - 1);
            value = trimmed(&value).to_vec();
        }
    }
    let name = if name.starts_with("--") {
        name.to_string()
    } else {
        name.to_ascii_lowercase()
    };
    if value.is_empty() && !name.starts_with("--") {
        return Err("A declaration has no value.".to_string());
    }
    Ok(Declaration {
        name,
        value,
        important,
        line,
        column,
    })
}

pub fn parse_declarations_forgiving(source: &str) -> (Vec<Declaration>, Vec<Problem>) {
    let (tokens, mut problems) = tokenize(source);
    let (components, mut tree_problems) = components_of(&tokens);
    problems.append(&mut tree_problems);
    let mut parser = Parser { problems };
    let positions = Positions::of(&tokens);
    let mut items = Vec::new();
    parser.items(&components, &positions, 0, false, false, &mut items);
    let mut out = Vec::new();
    for one in items {
        match one {
            Item::Declaration(held) => out.push(held),
            Item::Rule(_) => {}
        }
    }
    let mut problems = parser.problems;
    problems.sort_by_key(|one| (one.line, one.column));
    problems.dedup();
    (out, problems)
}

fn message_of(problem: &Problem) -> Diagnostic {
    let held = match problem.code {
        Code::Selector => error("EI701", "A CSS selector is not valid."),
        Code::Value => error("EI702", "A CSS value is not written the way CSS writes it."),
        Code::Syntax => error("EI700", "The stylesheet has an error."),
    };
    held.with_location(Location::at("stylesheet", problem.line, problem.column))
        .with_context(format!("Reported: {}", problem.message))
}

pub fn diagnostics_of(problems: &[Problem]) -> Vec<Diagnostic> {
    problems.iter().map(message_of).collect()
}

pub fn parse_stylesheet(source: &str) -> Result<Stylesheet, Diagnostic> {
    let (sheet, problems) = parse_stylesheet_forgiving(source);
    match problems.first() {
        Some(first) => Err(message_of(first)),
        None => Ok(sheet),
    }
}

pub fn parse_declarations(source: &str) -> Result<Vec<Declaration>, Diagnostic> {
    let (list, problems) = parse_declarations_forgiving(source);
    match problems.first() {
        Some(first) => Err(message_of(first)),
        None => Ok(list),
    }
}

pub fn parse_selectors(source: &str) -> Result<Vec<Selector>, Diagnostic> {
    let (tokens, _) = tokenize(source);
    let (components, _) = components_of(&tokens);
    parse_selector_list(trimmed(&components)).map_err(|why| {
        error("EI701", "A CSS selector is not valid.").with_context(format!("Reported: {why}"))
    })
}

fn write_string(out: &mut String, text: &str) {
    let quote = if text.contains('"') && !text.contains('\'') {
        '\''
    } else {
        '"'
    };
    out.push(quote);
    for character in text.chars() {
        match character {
            '\n' => out.push_str("\\a "),
            '\\' => out.push_str("\\\\"),
            other if other == quote => {
                out.push('\\');
                out.push(other);
            }
            other => out.push(other),
        }
    }
    out.push(quote);
}

fn write_ident(out: &mut String, text: &str) {
    let chars: Vec<char> = text.chars().collect();
    for (at, character) in chars.iter().copied().enumerate() {
        let blocked_digit = character.is_ascii_digit() && (at == 0 || (at == 1 && chars[0] == '-'));
        if character == '-' && chars.len() == 1 {
            out.push_str("\\-");
        } else if is_name(character) && !blocked_digit {
            out.push(character);
        } else if (character as u32) < 0x20 || character as u32 == 0x7f || blocked_digit {
            out.push_str(&format!("\\{:x} ", character as u32));
        } else {
            out.push('\\');
            out.push(character);
        }
    }
}

fn number_text(text: &str, minify: bool) -> String {
    if !minify {
        return text.to_string();
    }
    let lower = text.to_ascii_lowercase();
    if lower.contains('e') {
        return lower;
    }
    let (sign, digits) = match lower.strip_prefix('-') {
        Some(rest) => ("-", rest.to_string()),
        None => ("", lower.trim_start_matches('+').to_string()),
    };
    let (whole, fraction) = match digits.split_once('.') {
        Some((whole, fraction)) => (
            whole.to_string(),
            fraction.trim_end_matches('0').to_string(),
        ),
        None => (digits, String::new()),
    };
    let whole = whole.trim_start_matches('0').to_string();
    if whole.is_empty() && fraction.is_empty() {
        return "0".to_string();
    }
    if fraction.is_empty() {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{fraction}")
    }
}

fn shorten_hex(value: &str) -> String {
    let lower = value.to_ascii_lowercase();
    if !lower.chars().all(|character| character.is_ascii_hexdigit()) {
        return value.to_string();
    }
    let bytes: Vec<char> = lower.chars().collect();
    let pairs =
        |step: usize| (0..bytes.len() / 2).all(|at| bytes[at * 2] == bytes[at * 2 + 1] && step > 0);
    match bytes.len() {
        6 | 8 if pairs(1) => bytes.iter().step_by(2).collect(),
        _ => lower,
    }
}

fn write_token(out: &mut String, token: &Token, minify: bool) {
    match token {
        Token::Ident(name) => write_ident(out, name),
        Token::Function(name) => {
            write_ident(out, name);
            out.push('(');
        }
        Token::AtKeyword(name) => {
            out.push('@');
            write_ident(out, name);
        }
        Token::Hash { value, id } => {
            out.push('#');
            if minify && *id {
                out.push_str(&shorten_hex(value));
            } else {
                for character in value.chars() {
                    if is_name(character) {
                        out.push(character);
                    } else {
                        out.push('\\');
                        out.push(character);
                    }
                }
            }
        }
        Token::Str(text) => write_string(out, text),
        Token::BadString => out.push_str("\"\""),
        Token::Url(text) => {
            out.push_str("url(");
            let safe = text.chars().all(|character| {
                !is_whitespace(character)
                    && !matches!(character, '"' | '\'' | '(' | ')' | '\\')
                    && !is_non_printable(character)
            });
            if safe {
                out.push_str(text);
            } else {
                write_string(out, text);
            }
            out.push(')');
        }
        Token::BadUrl => out.push_str("url()"),
        Token::Delim(character) => out.push(*character),
        Token::Number { text, .. } => out.push_str(&number_text(text, minify)),
        Token::Percentage { text } => {
            out.push_str(&number_text(text, minify));
            out.push('%');
        }
        Token::Dimension { text, unit } => {
            out.push_str(&number_text(text, minify));
            write_ident(out, unit);
        }
        Token::Whitespace => out.push(' '),
        Token::Cdo => out.push_str("<!--"),
        Token::Cdc => out.push_str("-->"),
        Token::Colon => out.push(':'),
        Token::Semicolon => out.push(';'),
        Token::Comma => out.push(','),
        Token::OpenBracket => out.push('['),
        Token::CloseBracket => out.push(']'),
        Token::OpenParen => out.push('('),
        Token::CloseParen => out.push(')'),
        Token::OpenBrace => out.push('{'),
        Token::CloseBrace => out.push('}'),
    }
}

pub fn serialize_components(components: &[Component], minify: bool) -> String {
    let mut out = String::new();
    write_components(&mut out, components, minify, false);
    out
}

fn write_components(
    out: &mut String,
    components: &[Component],
    minify: bool,
    inside_parenthesis: bool,
) {
    let mut previous: Option<&Component> = None;
    let mut index = 0;
    while index < components.len() {
        let held = &components[index];
        if held.is_whitespace() {
            if minify {
                let next = components[index + 1..]
                    .iter()
                    .find(|one| !one.is_whitespace());
                let after_comma = matches!(previous, Some(Component::Token(Token::Comma)));
                let before_comma = matches!(next, Some(Component::Token(Token::Comma)));
                let tight = |one: &Component| one.delim('/') || one.delim('*');
                let slash = previous.is_some_and(tight) || next.is_some_and(tight);
                let before_bang = next.is_some_and(|one| one.delim('!'));
                let before_colon =
                    inside_parenthesis && matches!(next, Some(Component::Token(Token::Colon)));
                let keep = match (previous, next) {
                    (Some(left), Some(_)) => {
                        !after_comma
                            && !before_comma
                            && !slash
                            && !before_bang
                            && !before_colon
                            && !matches!(left, Component::Token(Token::Colon))
                    }
                    _ => false,
                };
                if keep {
                    out.push(' ');
                }
                while index + 1 < components.len() && components[index + 1].is_whitespace() {
                    index += 1;
                }
            } else {
                out.push(' ');
                while index + 1 < components.len() && components[index + 1].is_whitespace() {
                    index += 1;
                }
            }
            index += 1;
            continue;
        }
        match held {
            Component::Token(token) => write_token(out, token, minify),
            Component::Function { name, arguments } => {
                write_ident(out, name);
                out.push('(');
                write_components(out, arguments, minify, true);
                out.push(')');
            }
            Component::Parenthesis(inner) => {
                out.push('(');
                write_components(out, inner, minify, true);
                out.push(')');
            }
            Component::Bracket(inner) => {
                out.push('[');
                write_components(out, inner, minify, false);
                out.push(']');
            }
            Component::Brace(inner) => {
                out.push('{');
                write_components(out, inner, minify, false);
                out.push('}');
            }
        }
        previous = Some(held);
        index += 1;
    }
}

pub fn serialize_selector(selector: &Selector) -> String {
    let mut out = String::new();
    for (index, (combinator, compound)) in selector.parts.iter().enumerate() {
        match (index, combinator) {
            (0, Combinator::None) => {}
            (0, other) => {
                out.push_str(combinator_text(*other, false));
            }
            (_, other) => out.push_str(combinator_text(*other, true)),
        }
        for simple in compound {
            write_simple(&mut out, simple);
        }
    }
    out
}

fn combinator_text(combinator: Combinator, between: bool) -> &'static str {
    match (combinator, between) {
        (Combinator::None, _) => "",
        (Combinator::Descendant, _) => " ",
        (Combinator::Child, true) => " > ",
        (Combinator::NextSibling, true) => " + ",
        (Combinator::SubsequentSibling, true) => " ~ ",
        (Combinator::Column, true) => " || ",
        (Combinator::Child, false) => "> ",
        (Combinator::NextSibling, false) => "+ ",
        (Combinator::SubsequentSibling, false) => "~ ",
        (Combinator::Column, false) => "|| ",
    }
}

fn write_namespace(out: &mut String, namespace: &Option<String>) {
    if let Some(prefix) = namespace {
        if prefix != "*" {
            write_ident(out, prefix);
        } else {
            out.push('*');
        }
        out.push('|');
    }
}

fn write_simple(out: &mut String, simple: &Simple) {
    match simple {
        Simple::Type { namespace, name } => {
            write_namespace(out, namespace);
            write_ident(out, name);
        }
        Simple::Universal { namespace } => {
            write_namespace(out, namespace);
            out.push('*');
        }
        Simple::Id(name) => {
            out.push('#');
            write_ident(out, name);
        }
        Simple::Class(name) => {
            out.push('.');
            write_ident(out, name);
        }
        Simple::Nesting => out.push('&'),
        Simple::Attribute {
            namespace,
            name,
            matcher,
            modifier,
        } => {
            out.push('[');
            write_namespace(out, namespace);
            write_ident(out, name);
            if let Some((operator, value)) = matcher {
                out.push_str(match operator {
                    AttributeOperator::Equals => "=",
                    AttributeOperator::Includes => "~=",
                    AttributeOperator::DashMatch => "|=",
                    AttributeOperator::Prefix => "^=",
                    AttributeOperator::Suffix => "$=",
                    AttributeOperator::Substring => "*=",
                });
                write_string(out, value);
                if let Some(held) = modifier {
                    out.push(' ');
                    out.push(*held);
                }
            }
            out.push(']');
        }
        Simple::PseudoClass { name, arguments } => {
            out.push(':');
            write_pseudo(out, name, arguments);
        }
        Simple::PseudoElement { name, arguments } => {
            out.push_str("::");
            write_pseudo(out, name, arguments);
        }
    }
}

fn write_pseudo(out: &mut String, name: &str, arguments: &PseudoArguments) {
    write_ident(out, name);
    match arguments {
        PseudoArguments::None => {}
        PseudoArguments::Selectors(list) => {
            out.push('(');
            out.push_str(&serialize_selector_list(list));
            out.push(')');
        }
        PseudoArguments::Nth { a, b, of } => {
            out.push('(');
            out.push_str(&nth_text(*a, *b));
            if let Some(list) = of {
                out.push_str(" of ");
                out.push_str(&serialize_selector_list(list));
            }
            out.push(')');
        }
        PseudoArguments::Raw(components) => {
            out.push('(');
            write_components(out, components, false, true);
            out.push(')');
        }
    }
}

fn nth_text(a: i64, b: i64) -> String {
    match (a, b) {
        (0, b) => b.to_string(),
        (2, 0) => "2n".to_string(),
        (a, 0) => format!("{}n", step_text(a)),
        (a, b) if b > 0 => format!("{}n+{b}", step_text(a)),
        (a, b) => format!("{}n{b}", step_text(a)),
    }
}

fn step_text(a: i64) -> String {
    match a {
        1 => String::new(),
        -1 => "-".to_string(),
        other => other.to_string(),
    }
}

pub fn serialize_selector_list(list: &[Selector]) -> String {
    list.iter()
        .map(serialize_selector)
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    Pretty,
    Minified,
}

impl Stylesheet {
    pub fn to_css(&self, style: Style) -> String {
        let mut out = String::new();
        for (index, rule) in self.rules.iter().enumerate() {
            if style == Style::Pretty && index > 0 {
                out.push('\n');
            }
            write_rule(&mut out, rule, style, 0);
        }
        out
    }

    pub fn minified(&self) -> String {
        self.to_css(Style::Minified)
    }

    pub fn rule_count(&self) -> usize {
        fn count(rules: &[Item]) -> usize {
            rules
                .iter()
                .map(|one| match one {
                    Item::Rule(Rule::Style(style)) => 1 + count(&style.items),
                    Item::Rule(Rule::At(at)) => match &at.block {
                        Some(Block::Items(inner)) => 1 + count(inner),
                        _ => 1,
                    },
                    Item::Declaration(_) => 0,
                })
                .sum()
        }
        self.rules
            .iter()
            .map(|one| match one {
                Rule::Style(style) => 1 + count(&style.items),
                Rule::At(at) => match &at.block {
                    Some(Block::Items(inner)) => 1 + count(inner),
                    _ => 1,
                },
            })
            .sum()
    }
}

fn indent(out: &mut String, style: Style, depth: usize) {
    if style == Style::Pretty {
        for _ in 0..depth {
            out.push_str("  ");
        }
    }
}

fn write_rule(out: &mut String, rule: &Rule, style: Style, depth: usize) {
    let minify = style == Style::Minified;
    match rule {
        Rule::Style(held) => {
            indent(out, style, depth);
            match &held.selectors {
                Some(list) => {
                    let joined = if minify {
                        list.iter()
                            .map(serialize_selector)
                            .collect::<Vec<_>>()
                            .join(",")
                    } else {
                        serialize_selector_list(list)
                    };
                    out.push_str(&joined);
                }
                None => out.push_str(&serialize_components(&held.prelude, minify)),
            }
            write_items(out, &held.items, style, depth);
        }
        Rule::At(held) => {
            indent(out, style, depth);
            out.push('@');
            write_ident(out, &held.name);
            if !held.prelude.is_empty() {
                let text = serialize_components(&held.prelude, minify);
                if minify
                    && (text.starts_with('(') || text.starts_with('"') || text.starts_with('\''))
                {
                    out.push_str(&text);
                } else {
                    out.push(' ');
                    out.push_str(&text);
                }
            }
            match &held.block {
                None => {
                    out.push(';');
                    if !minify {
                        out.push('\n');
                    }
                }
                Some(Block::Items(items)) => write_items(out, items, style, depth),
                Some(Block::Raw(components)) => {
                    out.push('{');
                    out.push_str(&serialize_components(components, minify));
                    out.push('}');
                    if !minify {
                        out.push('\n');
                    }
                }
            }
        }
    }
}

fn write_items(out: &mut String, items: &[Item], style: Style, depth: usize) {
    let minify = style == Style::Minified;
    let live: Vec<&Item> = items
        .iter()
        .filter(|one| match one {
            Item::Rule(Rule::Style(held)) => !(minify && held.items.is_empty()),
            _ => true,
        })
        .collect();
    if minify {
        out.push('{');
    } else {
        out.push_str(" {\n");
    }
    for (index, item) in live.iter().enumerate() {
        match item {
            Item::Declaration(held) => {
                indent(out, style, depth + 1);
                write_ident(out, &held.name);
                out.push(':');
                if !minify {
                    out.push(' ');
                }
                let value = if held.name.starts_with("--") {
                    serialize_components(&held.value, false)
                } else {
                    serialize_components(&held.value, minify)
                };
                out.push_str(value.trim());
                if held.important {
                    out.push_str(if minify { "!important" } else { " !important" });
                }
                if !(minify && index + 1 == live.len()) {
                    out.push(';');
                }
                if !minify {
                    out.push('\n');
                }
            }
            Item::Rule(rule) => write_rule(out, rule, style, depth + 1),
        }
    }
    indent(out, style, depth);
    out.push('}');
    if !minify {
        out.push('\n');
    }
}

pub fn minify(source: &str) -> Result<String, Diagnostic> {
    parse_stylesheet(source).map(|sheet| sheet.minified())
}

pub fn pretty(source: &str) -> Result<String, Diagnostic> {
    parse_stylesheet(source).map(|sheet| sheet.to_css(Style::Pretty))
}

pub fn validate(source: &str) -> Vec<Diagnostic> {
    let (_, problems) = parse_stylesheet_forgiving(source);
    diagnostics_of(&problems)
}

use crate::caps::Capability;
use crate::plugin::{unimplemented_diagnostic, Context, Contract, Outcome, Plugin, Version};

pub static CONTRACT: Contract = Contract {
    id: "plugin.css",
    display_name: "CSS",
    version: Version::new(0, 1, 0),
    produces_artifacts: false,
    summary: "Reads, checks and minifies CSS style sheets that a web package carries.",
    inputs: &["css.source"],
    outputs: &["web.stylesheet"],
    required_capabilities: &[Capability::FsRead, Capability::TempStorage],
    forbidden_capabilities: &[
        Capability::Network,
        Capability::Internet,
        Capability::KeyAccess,
        Capability::SensitiveOutput,
    ],
    non_responsibilities: &[
        "Applying a style sheet to a page; the web view does that.",
        "Fetching style sheets from a network.",
    ],
};

pub struct CssPlugin;

pub static PLUGIN: CssPlugin = CssPlugin;

impl Plugin for CssPlugin {
    fn contract(&self) -> &'static Contract {
        &CONTRACT
    }

    fn execute(&self, _ctx: &mut Context<'_>) -> Result<Outcome, Diagnostic> {
        Err(unimplemented_diagnostic(&CONTRACT))
    }
}
