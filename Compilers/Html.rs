use crate::diag::{Diagnostic, Location, Severity};
use crate::FailureClass;
use std::collections::VecDeque;

pub const ORIGIN: &str = "web.html";

fn error(code: &str, sentence: &str) -> Diagnostic {
    Diagnostic::new(
        code,
        Severity::Error,
        FailureClass::UserError,
        ORIGIN,
        sentence,
    )
}

pub type NodeId = usize;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Namespace {
    Html,
    Svg,
    MathMl,
}

impl Namespace {
    pub fn uri(self) -> &'static str {
        match self {
            Namespace::Html => "http://www.w3.org/1999/xhtml",
            Namespace::Svg => "http://www.w3.org/2000/svg",
            Namespace::MathMl => "http://www.w3.org/1998/Math/MathML",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attribute {
    pub prefix: Option<&'static str>,
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    Document,
    Doctype {
        name: String,
        public_id: String,
        system_id: String,
    },
    Element {
        namespace: Namespace,
        name: String,
        attributes: Vec<Attribute>,
        contents: Option<NodeId>,
    },
    Text(String),
    Comment(String),
    Fragment,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Node {
    pub kind: NodeKind,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Quirks {
    No,
    Limited,
    Full,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseError {
    pub code: &'static str,
    pub offset: usize,
}

#[derive(Clone, Debug)]
pub struct Document {
    pub nodes: Vec<Node>,
    pub quirks: Quirks,
    pub errors: Vec<ParseError>,
}

impl Document {
    fn new() -> Document {
        Document {
            nodes: vec![Node {
                kind: NodeKind::Document,
                parent: None,
                children: Vec::new(),
            }],
            quirks: Quirks::No,
            errors: Vec::new(),
        }
    }

    pub fn root(&self) -> NodeId {
        0
    }

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id]
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        &self.nodes[id].children
    }

    pub fn name(&self, id: NodeId) -> Option<&str> {
        match &self.nodes[id].kind {
            NodeKind::Element { name, .. } => Some(name.as_str()),
            _ => None,
        }
    }

    pub fn attribute(&self, id: NodeId, wanted: &str) -> Option<&str> {
        match &self.nodes[id].kind {
            NodeKind::Element { attributes, .. } => attributes
                .iter()
                .find(|attribute| attribute.name == wanted)
                .map(|attribute| attribute.value.as_str()),
            _ => None,
        }
    }

    pub fn text_content(&self, id: NodeId) -> String {
        let mut out = String::new();
        self.collect_text(id, &mut out);
        out
    }

    fn collect_text(&self, id: NodeId, out: &mut String) {
        match &self.nodes[id].kind {
            NodeKind::Text(text) => out.push_str(text),
            NodeKind::Element { contents, .. } => {
                if let Some(inner) = contents {
                    self.collect_text(*inner, out);
                }
                for child in &self.nodes[id].children {
                    self.collect_text(*child, out);
                }
            }
            _ => {
                for child in &self.nodes[id].children {
                    self.collect_text(*child, out);
                }
            }
        }
    }

    pub fn descendants(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack: Vec<NodeId> = self.nodes[id].children.iter().rev().copied().collect();
        while let Some(next) = stack.pop() {
            out.push(next);
            if let NodeKind::Element {
                contents: Some(inner),
                ..
            } = &self.nodes[next].kind
            {
                for child in self.nodes[*inner].children.iter().rev() {
                    stack.push(*child);
                }
            }
            for child in self.nodes[next].children.iter().rev() {
                stack.push(*child);
            }
        }
        out
    }

    pub fn elements_named(&self, wanted: &str) -> Vec<NodeId> {
        self.descendants(0)
            .into_iter()
            .filter(|id| self.name(*id) == Some(wanted))
            .collect()
    }

    fn create(&mut self, kind: NodeKind) -> NodeId {
        self.nodes.push(Node {
            kind,
            parent: None,
            children: Vec::new(),
        });
        self.nodes.len() - 1
    }

    fn append(&mut self, parent: NodeId, child: NodeId) {
        self.nodes[child].parent = Some(parent);
        self.nodes[parent].children.push(child);
    }

    fn insert_before(&mut self, parent: NodeId, child: NodeId, before: NodeId) {
        let at = self.nodes[parent]
            .children
            .iter()
            .position(|held| *held == before)
            .unwrap_or(self.nodes[parent].children.len());
        self.nodes[child].parent = Some(parent);
        self.nodes[parent].children.insert(at, child);
    }

    fn detach(&mut self, child: NodeId) {
        if let Some(parent) = self.nodes[child].parent.take() {
            self.nodes[parent].children.retain(|held| *held != child);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Doctype {
        name: Option<String>,
        public_id: Option<String>,
        system_id: Option<String>,
        force_quirks: bool,
    },
    Start {
        name: String,
        attributes: Vec<(String, String)>,
        self_closing: bool,
    },
    End {
        name: String,
    },
    Comment(String),
    Char(char),
    Eof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Data,
    Rcdata,
    Rawtext,
    ScriptData,
    Plaintext,
    TagOpen,
    EndTagOpen,
    TagName,
    RcdataLessThan,
    RcdataEndTagOpen,
    RcdataEndTagName,
    RawtextLessThan,
    RawtextEndTagOpen,
    RawtextEndTagName,
    ScriptLessThan,
    ScriptEndTagOpen,
    ScriptEndTagName,
    ScriptEscapeStart,
    ScriptEscapeStartDash,
    ScriptEscaped,
    ScriptEscapedDash,
    ScriptEscapedDashDash,
    ScriptEscapedLessThan,
    ScriptEscapedEndTagOpen,
    ScriptEscapedEndTagName,
    ScriptDoubleEscapeStart,
    ScriptDoubleEscaped,
    ScriptDoubleEscapedDash,
    ScriptDoubleEscapedDashDash,
    ScriptDoubleEscapedLessThan,
    ScriptDoubleEscapeEnd,
    BeforeAttributeName,
    AttributeName,
    AfterAttributeName,
    BeforeAttributeValue,
    AttributeValueDouble,
    AttributeValueSingle,
    AttributeValueUnquoted,
    AfterAttributeValueQuoted,
    SelfClosingStartTag,
    BogusComment,
    MarkupDeclarationOpen,
    CommentStart,
    CommentStartDash,
    Comment,
    CommentLessThan,
    CommentLessThanBang,
    CommentLessThanBangDash,
    CommentLessThanBangDashDash,
    CommentEndDash,
    CommentEnd,
    CommentEndBang,
    Doctype,
    BeforeDoctypeName,
    DoctypeName,
    AfterDoctypeName,
    AfterDoctypePublicKeyword,
    BeforeDoctypePublicId,
    DoctypePublicIdDouble,
    DoctypePublicIdSingle,
    AfterDoctypePublicId,
    BetweenDoctypeIds,
    AfterDoctypeSystemKeyword,
    BeforeDoctypeSystemId,
    DoctypeSystemIdDouble,
    DoctypeSystemIdSingle,
    AfterDoctypeSystemId,
    BogusDoctype,
    CdataSection,
    CdataSectionBracket,
    CdataSectionEnd,
}

const ENTITY_LONGEST: usize = 32;

fn find_entity(name: &str) -> Option<&'static str> {
    ENTITIES
        .binary_search_by(|(held, _)| (*held).cmp(name))
        .ok()
        .map(|at| ENTITIES[at].1)
}

fn numeric_replacement(code: u32) -> char {
    const TABLE: [(u32, char); 27] = [
        (0x80, '\u{20ac}'),
        (0x82, '\u{201a}'),
        (0x83, '\u{0192}'),
        (0x84, '\u{201e}'),
        (0x85, '\u{2026}'),
        (0x86, '\u{2020}'),
        (0x87, '\u{2021}'),
        (0x88, '\u{02c6}'),
        (0x89, '\u{2030}'),
        (0x8a, '\u{0160}'),
        (0x8b, '\u{2039}'),
        (0x8c, '\u{0152}'),
        (0x8e, '\u{017d}'),
        (0x91, '\u{2018}'),
        (0x92, '\u{2019}'),
        (0x93, '\u{201c}'),
        (0x94, '\u{201d}'),
        (0x95, '\u{2022}'),
        (0x96, '\u{2013}'),
        (0x97, '\u{2014}'),
        (0x98, '\u{02dc}'),
        (0x99, '\u{2122}'),
        (0x9a, '\u{0161}'),
        (0x9b, '\u{203a}'),
        (0x9c, '\u{0153}'),
        (0x9e, '\u{017e}'),
        (0x9f, '\u{0178}'),
    ];
    if code == 0 || code > 0x10ffff || (0xd800..=0xdfff).contains(&code) {
        return '\u{fffd}';
    }
    for (from, to) in TABLE {
        if from == code {
            return to;
        }
    }
    char::from_u32(code).unwrap_or('\u{fffd}')
}

struct Tokenizer {
    chars: Vec<char>,
    pos: usize,
    state: State,
    return_state: State,
    pending: VecDeque<Token>,
    errors: Vec<ParseError>,
    buffer: String,
    tag_name: String,
    tag_end: bool,
    tag_self_closing: bool,
    attributes: Vec<(String, String)>,
    attribute_name: String,
    attribute_value: String,
    comment: String,
    doctype_name: Option<String>,
    doctype_public: Option<String>,
    doctype_system: Option<String>,
    doctype_quirks: bool,
    last_start: String,
    cdata_allowed: bool,
    finished: bool,
}

fn is_ascii_space(character: char) -> bool {
    matches!(character, '\t' | '\n' | '\u{c}' | ' ')
}

impl Tokenizer {
    fn new(source: &str) -> Tokenizer {
        let mut chars: Vec<char> = Vec::with_capacity(source.len());
        let mut previous_cr = false;
        for character in source.chars() {
            if character == '\r' {
                chars.push('\n');
                previous_cr = true;
                continue;
            }
            if character == '\n' && previous_cr {
                previous_cr = false;
                continue;
            }
            previous_cr = false;
            chars.push(character);
        }
        let mut errors = Vec::new();
        for (at, character) in chars.iter().enumerate() {
            let code = *character as u32;
            if (0xfdd0..=0xfdef).contains(&code) || (code & 0xfffe) == 0xfffe {
                errors.push(ParseError {
                    code: "noncharacter-in-input-stream",
                    offset: at + 1,
                });
            } else if (code < 0x20 && !matches!(code, 0x00 | 0x09 | 0x0a | 0x0c))
                || (0x7f..=0x9f).contains(&code)
            {
                errors.push(ParseError {
                    code: "control-character-in-input-stream",
                    offset: at + 1,
                });
            }
        }
        Tokenizer {
            chars,
            pos: 0,
            state: State::Data,
            return_state: State::Data,
            pending: VecDeque::new(),
            errors,
            buffer: String::new(),
            tag_name: String::new(),
            tag_end: false,
            tag_self_closing: false,
            attributes: Vec::new(),
            attribute_name: String::new(),
            attribute_value: String::new(),
            comment: String::new(),
            doctype_name: None,
            doctype_public: None,
            doctype_system: None,
            doctype_quirks: false,
            last_start: String::new(),
            cdata_allowed: false,
            finished: false,
        }
    }

    fn fault(&mut self, code: &'static str) {
        self.errors.push(ParseError {
            code,
            offset: self.pos,
        });
    }

    fn consume(&mut self) -> Option<char> {
        let character = self.chars.get(self.pos).copied();
        if character.is_some() {
            self.pos += 1;
        }
        character
    }

    fn unconsume(&mut self, character: Option<char>) {
        if character.is_some() {
            self.pos -= 1;
        }
    }

    fn starts_with(&self, text: &str, ignore_case: bool) -> bool {
        text.chars().enumerate().all(|(offset, expected)| {
            self.chars.get(self.pos + offset).is_some_and(|held| {
                if ignore_case {
                    held.eq_ignore_ascii_case(&expected)
                } else {
                    *held == expected
                }
            })
        })
    }

    fn emit(&mut self, token: Token) {
        self.pending.push_back(token);
    }

    fn emit_str(&mut self, text: &str) {
        for character in text.chars() {
            self.pending.push_back(Token::Char(character));
        }
    }

    fn start_tag(&mut self, end: bool) {
        self.tag_name.clear();
        self.tag_end = end;
        self.tag_self_closing = false;
        self.attributes.clear();
        self.attribute_name.clear();
        self.attribute_value.clear();
    }

    fn finish_attribute(&mut self) {
        if self.attribute_name.is_empty() {
            return;
        }
        let name = std::mem::take(&mut self.attribute_name);
        let value = std::mem::take(&mut self.attribute_value);
        if self.attributes.iter().any(|(held, _)| *held == name) {
            self.fault("duplicate-attribute");
        } else {
            self.attributes.push((name, value));
        }
    }

    fn begin_attribute(&mut self) {
        self.finish_attribute();
        self.attribute_name.clear();
        self.attribute_value.clear();
    }

    fn emit_tag(&mut self) {
        self.finish_attribute();
        let name = std::mem::take(&mut self.tag_name);
        if self.tag_end {
            if !self.attributes.is_empty() {
                self.fault("end-tag-with-attributes");
            }
            if self.tag_self_closing {
                self.fault("end-tag-with-trailing-solidus");
            }
            self.emit(Token::End { name });
        } else {
            self.last_start = name.clone();
            let attributes = std::mem::take(&mut self.attributes);
            self.emit(Token::Start {
                name,
                attributes,
                self_closing: self.tag_self_closing,
            });
        }
    }

    fn appropriate_end(&self) -> bool {
        self.tag_end && !self.last_start.is_empty() && self.tag_name == self.last_start
    }

    fn in_attribute(&self) -> bool {
        matches!(
            self.return_state,
            State::AttributeValueDouble
                | State::AttributeValueSingle
                | State::AttributeValueUnquoted
        )
    }

    fn flush_text(&mut self, text: &str) {
        if self.in_attribute() {
            self.attribute_value.push_str(text);
        } else {
            self.emit_str(text);
        }
    }

    fn character_reference(&mut self) {
        let next = self.chars.get(self.pos).copied();
        match next {
            Some(character) if character.is_ascii_alphanumeric() => {
                let mut longest: Option<(usize, &'static str)> = None;
                let mut candidate = String::new();
                let mut length = 0;
                while length < ENTITY_LONGEST {
                    let Some(held) = self.chars.get(self.pos + length).copied() else {
                        break;
                    };
                    if !(held.is_ascii_alphanumeric() || held == ';') {
                        break;
                    }
                    candidate.push(held);
                    length += 1;
                    if let Some(value) = find_entity(&candidate) {
                        longest = Some((length, value));
                    }
                    if held == ';' {
                        break;
                    }
                }
                match longest {
                    Some((used, value)) => {
                        let terminated = self.chars[self.pos + used - 1] == ';';
                        let after = self.chars.get(self.pos + used).copied();
                        if self.in_attribute()
                            && !terminated
                            && after.is_some_and(|held| held == '=' || held.is_ascii_alphanumeric())
                        {
                            self.flush_text("&");
                            return;
                        }
                        if !terminated {
                            self.fault("missing-semicolon-after-character-reference");
                        }
                        self.pos += used;
                        self.flush_text(value);
                    }
                    None => {
                        self.flush_text("&");
                        let mut look = 0;
                        while self
                            .chars
                            .get(self.pos + look)
                            .is_some_and(|held| held.is_ascii_alphanumeric())
                        {
                            look += 1;
                        }
                        if self.chars.get(self.pos + look) == Some(&';') {
                            self.fault("unknown-named-character-reference");
                        }
                    }
                }
            }
            Some('#') => {
                self.pos += 1;
                let hex = matches!(self.chars.get(self.pos), Some('x') | Some('X'));
                if hex {
                    self.pos += 1;
                }
                let radix = if hex { 16 } else { 10 };
                let begin = self.pos;
                let mut code: u32 = 0;
                while let Some(digit) = self
                    .chars
                    .get(self.pos)
                    .and_then(|held| held.to_digit(radix))
                {
                    code = code.saturating_mul(radix).saturating_add(digit);
                    if code > 0x10ffff {
                        code = 0x110000;
                    }
                    self.pos += 1;
                }
                if self.pos == begin {
                    self.fault("absence-of-digits-in-numeric-character-reference");
                    self.pos = begin - 1 - usize::from(hex);
                    self.flush_text("&");
                    return;
                }
                if self.chars.get(self.pos) == Some(&';') {
                    self.pos += 1;
                } else {
                    self.fault("missing-semicolon-after-character-reference");
                }
                if code == 0 {
                    self.fault("null-character-reference");
                } else if code > 0x10ffff {
                    self.fault("character-reference-outside-unicode-range");
                } else if (0xd800..=0xdfff).contains(&code) {
                    self.fault("surrogate-character-reference");
                } else if (0xfdd0..=0xfdef).contains(&code) || (code & 0xfffe) == 0xfffe {
                    self.fault("noncharacter-character-reference");
                } else if code == 0x0d
                    || (code < 0x20 && !matches!(code, 0x09 | 0x0a | 0x0c))
                    || (0x7f..=0x9f).contains(&code)
                {
                    self.fault("control-character-reference");
                }
                let replaced = numeric_replacement(code);
                self.flush_text(&replaced.to_string());
            }
            _ => self.flush_text("&"),
        }
    }

    fn step(&mut self) {
        match self.state {
            State::Data => match self.consume() {
                Some('&') => {
                    self.return_state = State::Data;
                    self.character_reference();
                }
                Some('<') => self.state = State::TagOpen,
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.emit(Token::Char('\0'));
                }
                Some(character) => self.emit(Token::Char(character)),
                None => self.finish(),
            },
            State::Rcdata => match self.consume() {
                Some('&') => {
                    self.return_state = State::Rcdata;
                    self.character_reference();
                }
                Some('<') => self.state = State::RcdataLessThan,
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.emit(Token::Char('\u{fffd}'));
                }
                Some(character) => self.emit(Token::Char(character)),
                None => self.finish(),
            },
            State::Rawtext => match self.consume() {
                Some('<') => self.state = State::RawtextLessThan,
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.emit(Token::Char('\u{fffd}'));
                }
                Some(character) => self.emit(Token::Char(character)),
                None => self.finish(),
            },
            State::ScriptData => match self.consume() {
                Some('<') => self.state = State::ScriptLessThan,
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.emit(Token::Char('\u{fffd}'));
                }
                Some(character) => self.emit(Token::Char(character)),
                None => self.finish(),
            },
            State::Plaintext => match self.consume() {
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.emit(Token::Char('\u{fffd}'));
                }
                Some(character) => self.emit(Token::Char(character)),
                None => self.finish(),
            },
            State::TagOpen => match self.consume() {
                Some('!') => self.state = State::MarkupDeclarationOpen,
                Some('/') => self.state = State::EndTagOpen,
                Some(character) if character.is_ascii_alphabetic() => {
                    self.start_tag(false);
                    self.unconsume(Some(character));
                    self.state = State::TagName;
                }
                Some('?') => {
                    self.fault("unexpected-question-mark-instead-of-tag-name");
                    self.comment.clear();
                    self.pos -= 1;
                    self.state = State::BogusComment;
                }
                other => {
                    self.fault(if other.is_none() {
                        "eof-before-tag-name"
                    } else {
                        "invalid-first-character-of-tag-name"
                    });
                    self.emit(Token::Char('<'));
                    self.unconsume(other);
                    self.state = State::Data;
                }
            },
            State::EndTagOpen => match self.consume() {
                Some(character) if character.is_ascii_alphabetic() => {
                    self.start_tag(true);
                    self.unconsume(Some(character));
                    self.state = State::TagName;
                }
                Some('>') => {
                    self.fault("missing-end-tag-name");
                    self.state = State::Data;
                }
                None => {
                    self.fault("eof-before-tag-name");
                    self.emit_str("</");
                    self.finish();
                }
                Some(character) => {
                    self.fault("invalid-first-character-of-tag-name");
                    self.comment.clear();
                    self.unconsume(Some(character));
                    self.state = State::BogusComment;
                }
            },
            State::TagName => match self.consume() {
                Some(character) if is_ascii_space(character) => {
                    self.state = State::BeforeAttributeName
                }
                Some('/') => self.state = State::SelfClosingStartTag,
                Some('>') => {
                    self.emit_tag();
                    self.state = State::Data;
                }
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.tag_name.push('\u{fffd}');
                }
                Some(character) => self.tag_name.push(character.to_ascii_lowercase()),
                None => {
                    self.fault("eof-in-tag");
                    self.finish();
                }
            },
            State::RcdataLessThan => match self.consume() {
                Some('/') => {
                    self.buffer.clear();
                    self.state = State::RcdataEndTagOpen;
                }
                other => {
                    self.emit(Token::Char('<'));
                    self.unconsume(other);
                    self.state = State::Rcdata;
                }
            },
            State::RcdataEndTagOpen => match self.consume() {
                Some(character) if character.is_ascii_alphabetic() => {
                    self.start_tag(true);
                    self.unconsume(Some(character));
                    self.state = State::RcdataEndTagName;
                }
                other => {
                    self.emit_str("</");
                    self.unconsume(other);
                    self.state = State::Rcdata;
                }
            },
            State::RcdataEndTagName => self.raw_end_tag_name(State::Rcdata),
            State::RawtextLessThan => match self.consume() {
                Some('/') => {
                    self.buffer.clear();
                    self.state = State::RawtextEndTagOpen;
                }
                other => {
                    self.emit(Token::Char('<'));
                    self.unconsume(other);
                    self.state = State::Rawtext;
                }
            },
            State::RawtextEndTagOpen => match self.consume() {
                Some(character) if character.is_ascii_alphabetic() => {
                    self.start_tag(true);
                    self.unconsume(Some(character));
                    self.state = State::RawtextEndTagName;
                }
                other => {
                    self.emit_str("</");
                    self.unconsume(other);
                    self.state = State::Rawtext;
                }
            },
            State::RawtextEndTagName => self.raw_end_tag_name(State::Rawtext),
            State::ScriptLessThan => match self.consume() {
                Some('/') => {
                    self.buffer.clear();
                    self.state = State::ScriptEndTagOpen;
                }
                Some('!') => {
                    self.state = State::ScriptEscapeStart;
                    self.emit_str("<!");
                }
                other => {
                    self.emit(Token::Char('<'));
                    self.unconsume(other);
                    self.state = State::ScriptData;
                }
            },
            State::ScriptEndTagOpen => match self.consume() {
                Some(character) if character.is_ascii_alphabetic() => {
                    self.start_tag(true);
                    self.unconsume(Some(character));
                    self.state = State::ScriptEndTagName;
                }
                other => {
                    self.emit_str("</");
                    self.unconsume(other);
                    self.state = State::ScriptData;
                }
            },
            State::ScriptEndTagName => self.raw_end_tag_name(State::ScriptData),
            State::ScriptEscapeStart => match self.consume() {
                Some('-') => {
                    self.state = State::ScriptEscapeStartDash;
                    self.emit(Token::Char('-'));
                }
                other => {
                    self.unconsume(other);
                    self.state = State::ScriptData;
                }
            },
            State::ScriptEscapeStartDash => match self.consume() {
                Some('-') => {
                    self.state = State::ScriptEscapedDashDash;
                    self.emit(Token::Char('-'));
                }
                other => {
                    self.unconsume(other);
                    self.state = State::ScriptData;
                }
            },
            State::ScriptEscaped => match self.consume() {
                Some('-') => {
                    self.state = State::ScriptEscapedDash;
                    self.emit(Token::Char('-'));
                }
                Some('<') => self.state = State::ScriptEscapedLessThan,
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.emit(Token::Char('\u{fffd}'));
                }
                Some(character) => self.emit(Token::Char(character)),
                None => {
                    self.fault("eof-in-script-html-comment-like-text");
                    self.finish();
                }
            },
            State::ScriptEscapedDash => match self.consume() {
                Some('-') => {
                    self.state = State::ScriptEscapedDashDash;
                    self.emit(Token::Char('-'));
                }
                Some('<') => self.state = State::ScriptEscapedLessThan,
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.state = State::ScriptEscaped;
                    self.emit(Token::Char('\u{fffd}'));
                }
                Some(character) => {
                    self.state = State::ScriptEscaped;
                    self.emit(Token::Char(character));
                }
                None => {
                    self.fault("eof-in-script-html-comment-like-text");
                    self.finish();
                }
            },
            State::ScriptEscapedDashDash => match self.consume() {
                Some('-') => self.emit(Token::Char('-')),
                Some('<') => self.state = State::ScriptEscapedLessThan,
                Some('>') => {
                    self.state = State::ScriptData;
                    self.emit(Token::Char('>'));
                }
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.state = State::ScriptEscaped;
                    self.emit(Token::Char('\u{fffd}'));
                }
                Some(character) => {
                    self.state = State::ScriptEscaped;
                    self.emit(Token::Char(character));
                }
                None => {
                    self.fault("eof-in-script-html-comment-like-text");
                    self.finish();
                }
            },
            State::ScriptEscapedLessThan => match self.consume() {
                Some('/') => {
                    self.buffer.clear();
                    self.state = State::ScriptEscapedEndTagOpen;
                }
                Some(character) if character.is_ascii_alphabetic() => {
                    self.buffer.clear();
                    self.emit(Token::Char('<'));
                    self.unconsume(Some(character));
                    self.state = State::ScriptDoubleEscapeStart;
                }
                other => {
                    self.emit(Token::Char('<'));
                    self.unconsume(other);
                    self.state = State::ScriptEscaped;
                }
            },
            State::ScriptEscapedEndTagOpen => match self.consume() {
                Some(character) if character.is_ascii_alphabetic() => {
                    self.start_tag(true);
                    self.unconsume(Some(character));
                    self.state = State::ScriptEscapedEndTagName;
                }
                other => {
                    self.emit_str("</");
                    self.unconsume(other);
                    self.state = State::ScriptEscaped;
                }
            },
            State::ScriptEscapedEndTagName => self.raw_end_tag_name(State::ScriptEscaped),
            State::ScriptDoubleEscapeStart => match self.consume() {
                Some(character)
                    if is_ascii_space(character) || character == '/' || character == '>' =>
                {
                    self.state = if self.buffer == "script" {
                        State::ScriptDoubleEscaped
                    } else {
                        State::ScriptEscaped
                    };
                    self.emit(Token::Char(character));
                }
                Some(character) if character.is_ascii_alphabetic() => {
                    self.buffer.push(character.to_ascii_lowercase());
                    self.emit(Token::Char(character));
                }
                other => {
                    self.unconsume(other);
                    self.state = State::ScriptEscaped;
                }
            },
            State::ScriptDoubleEscaped => match self.consume() {
                Some('-') => {
                    self.state = State::ScriptDoubleEscapedDash;
                    self.emit(Token::Char('-'));
                }
                Some('<') => {
                    self.state = State::ScriptDoubleEscapedLessThan;
                    self.emit(Token::Char('<'));
                }
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.emit(Token::Char('\u{fffd}'));
                }
                Some(character) => self.emit(Token::Char(character)),
                None => {
                    self.fault("eof-in-script-html-comment-like-text");
                    self.finish();
                }
            },
            State::ScriptDoubleEscapedDash => match self.consume() {
                Some('-') => {
                    self.state = State::ScriptDoubleEscapedDashDash;
                    self.emit(Token::Char('-'));
                }
                Some('<') => {
                    self.state = State::ScriptDoubleEscapedLessThan;
                    self.emit(Token::Char('<'));
                }
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.state = State::ScriptDoubleEscaped;
                    self.emit(Token::Char('\u{fffd}'));
                }
                Some(character) => {
                    self.state = State::ScriptDoubleEscaped;
                    self.emit(Token::Char(character));
                }
                None => {
                    self.fault("eof-in-script-html-comment-like-text");
                    self.finish();
                }
            },
            State::ScriptDoubleEscapedDashDash => match self.consume() {
                Some('-') => self.emit(Token::Char('-')),
                Some('<') => {
                    self.state = State::ScriptDoubleEscapedLessThan;
                    self.emit(Token::Char('<'));
                }
                Some('>') => {
                    self.state = State::ScriptData;
                    self.emit(Token::Char('>'));
                }
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.state = State::ScriptDoubleEscaped;
                    self.emit(Token::Char('\u{fffd}'));
                }
                Some(character) => {
                    self.state = State::ScriptDoubleEscaped;
                    self.emit(Token::Char(character));
                }
                None => {
                    self.fault("eof-in-script-html-comment-like-text");
                    self.finish();
                }
            },
            State::ScriptDoubleEscapedLessThan => match self.consume() {
                Some('/') => {
                    self.buffer.clear();
                    self.state = State::ScriptDoubleEscapeEnd;
                    self.emit(Token::Char('/'));
                }
                other => {
                    self.unconsume(other);
                    self.state = State::ScriptDoubleEscaped;
                }
            },
            State::ScriptDoubleEscapeEnd => match self.consume() {
                Some(character)
                    if is_ascii_space(character) || character == '/' || character == '>' =>
                {
                    self.state = if self.buffer == "script" {
                        State::ScriptEscaped
                    } else {
                        State::ScriptDoubleEscaped
                    };
                    self.emit(Token::Char(character));
                }
                Some(character) if character.is_ascii_alphabetic() => {
                    self.buffer.push(character.to_ascii_lowercase());
                    self.emit(Token::Char(character));
                }
                other => {
                    self.unconsume(other);
                    self.state = State::ScriptDoubleEscaped;
                }
            },
            State::BeforeAttributeName => match self.consume() {
                Some(character) if is_ascii_space(character) => {}
                Some('/') | Some('>') | None => {
                    self.pos = self.pos.saturating_sub(usize::from(
                        self.pos > 0
                            && self.chars.get(self.pos - 1).is_some()
                            && self.pos <= self.chars.len(),
                    ));
                    self.state = State::AfterAttributeName;
                }
                Some('=') => {
                    self.fault("unexpected-equals-sign-before-attribute-name");
                    self.begin_attribute();
                    self.attribute_name.push('=');
                    self.state = State::AttributeName;
                }
                Some(character) => {
                    self.begin_attribute();
                    self.unconsume(Some(character));
                    self.state = State::AttributeName;
                }
            },
            State::AttributeName => match self.consume() {
                Some(character)
                    if is_ascii_space(character) || character == '/' || character == '>' =>
                {
                    self.unconsume(Some(character));
                    self.state = State::AfterAttributeName;
                }
                None => self.state = State::AfterAttributeName,
                Some('=') => self.state = State::BeforeAttributeValue,
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.attribute_name.push('\u{fffd}');
                }
                Some(character) => {
                    if matches!(character, '"' | '\'' | '<') {
                        self.fault("unexpected-character-in-attribute-name");
                    }
                    self.attribute_name.push(character.to_ascii_lowercase());
                }
            },
            State::AfterAttributeName => match self.consume() {
                Some(character) if is_ascii_space(character) => {}
                Some('/') => self.state = State::SelfClosingStartTag,
                Some('=') => self.state = State::BeforeAttributeValue,
                Some('>') => {
                    self.emit_tag();
                    self.state = State::Data;
                }
                None => {
                    self.fault("eof-in-tag");
                    self.finish();
                }
                Some(character) => {
                    self.begin_attribute();
                    self.unconsume(Some(character));
                    self.state = State::AttributeName;
                }
            },
            State::BeforeAttributeValue => match self.consume() {
                Some(character) if is_ascii_space(character) => {}
                Some('"') => self.state = State::AttributeValueDouble,
                Some('\'') => self.state = State::AttributeValueSingle,
                Some('>') => {
                    self.fault("missing-attribute-value");
                    self.emit_tag();
                    self.state = State::Data;
                }
                other => {
                    self.unconsume(other);
                    self.state = State::AttributeValueUnquoted;
                }
            },
            State::AttributeValueDouble => match self.consume() {
                Some('"') => self.state = State::AfterAttributeValueQuoted,
                Some('&') => {
                    self.return_state = State::AttributeValueDouble;
                    self.character_reference();
                }
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.attribute_value.push('\u{fffd}');
                }
                Some(character) => self.attribute_value.push(character),
                None => {
                    self.fault("eof-in-tag");
                    self.finish();
                }
            },
            State::AttributeValueSingle => match self.consume() {
                Some('\'') => self.state = State::AfterAttributeValueQuoted,
                Some('&') => {
                    self.return_state = State::AttributeValueSingle;
                    self.character_reference();
                }
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.attribute_value.push('\u{fffd}');
                }
                Some(character) => self.attribute_value.push(character),
                None => {
                    self.fault("eof-in-tag");
                    self.finish();
                }
            },
            State::AttributeValueUnquoted => match self.consume() {
                Some(character) if is_ascii_space(character) => {
                    self.state = State::BeforeAttributeName
                }
                Some('&') => {
                    self.return_state = State::AttributeValueUnquoted;
                    self.character_reference();
                }
                Some('>') => {
                    self.emit_tag();
                    self.state = State::Data;
                }
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.attribute_value.push('\u{fffd}');
                }
                Some(character) => {
                    if matches!(character, '"' | '\'' | '<' | '=' | '`') {
                        self.fault("unexpected-character-in-unquoted-attribute-value");
                    }
                    self.attribute_value.push(character);
                }
                None => {
                    self.fault("eof-in-tag");
                    self.finish();
                }
            },
            State::AfterAttributeValueQuoted => match self.consume() {
                Some(character) if is_ascii_space(character) => {
                    self.state = State::BeforeAttributeName
                }
                Some('/') => self.state = State::SelfClosingStartTag,
                Some('>') => {
                    self.emit_tag();
                    self.state = State::Data;
                }
                None => {
                    self.fault("eof-in-tag");
                    self.finish();
                }
                other => {
                    self.fault("missing-whitespace-between-attributes");
                    self.unconsume(other);
                    self.state = State::BeforeAttributeName;
                }
            },
            State::SelfClosingStartTag => match self.consume() {
                Some('>') => {
                    self.tag_self_closing = true;
                    self.emit_tag();
                    self.state = State::Data;
                }
                None => {
                    self.fault("eof-in-tag");
                    self.finish();
                }
                other => {
                    self.fault("unexpected-solidus-in-tag");
                    self.unconsume(other);
                    self.state = State::BeforeAttributeName;
                }
            },
            State::BogusComment => match self.consume() {
                Some('>') => {
                    let text = std::mem::take(&mut self.comment);
                    self.emit(Token::Comment(text));
                    self.state = State::Data;
                }
                None => {
                    let text = std::mem::take(&mut self.comment);
                    self.emit(Token::Comment(text));
                    self.finish();
                }
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.comment.push('\u{fffd}');
                }
                Some(character) => self.comment.push(character),
            },
            State::MarkupDeclarationOpen => {
                if self.starts_with("--", false) {
                    self.pos += 2;
                    self.comment.clear();
                    self.state = State::CommentStart;
                } else if self.starts_with("doctype", true) {
                    self.pos += 7;
                    self.state = State::Doctype;
                } else if self.starts_with("[CDATA[", false) {
                    self.pos += 7;
                    if self.cdata_allowed {
                        self.state = State::CdataSection;
                    } else {
                        self.fault("cdata-in-html-content");
                        self.comment = "[CDATA[".to_string();
                        self.state = State::BogusComment;
                    }
                } else {
                    self.fault("incorrectly-opened-comment");
                    self.comment.clear();
                    self.state = State::BogusComment;
                }
            }
            State::CommentStart => match self.consume() {
                Some('-') => self.state = State::CommentStartDash,
                Some('>') => {
                    self.fault("abrupt-closing-of-empty-comment");
                    let text = std::mem::take(&mut self.comment);
                    self.emit(Token::Comment(text));
                    self.state = State::Data;
                }
                other => {
                    self.unconsume(other);
                    self.state = State::Comment;
                }
            },
            State::CommentStartDash => match self.consume() {
                Some('-') => self.state = State::CommentEnd,
                Some('>') => {
                    self.fault("abrupt-closing-of-empty-comment");
                    let text = std::mem::take(&mut self.comment);
                    self.emit(Token::Comment(text));
                    self.state = State::Data;
                }
                None => {
                    self.fault("eof-in-comment");
                    let text = std::mem::take(&mut self.comment);
                    self.emit(Token::Comment(text));
                    self.finish();
                }
                other => {
                    self.comment.push('-');
                    self.unconsume(other);
                    self.state = State::Comment;
                }
            },
            State::Comment => match self.consume() {
                Some('<') => {
                    self.comment.push('<');
                    self.state = State::CommentLessThan;
                }
                Some('-') => self.state = State::CommentEndDash,
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.comment.push('\u{fffd}');
                }
                Some(character) => self.comment.push(character),
                None => {
                    self.fault("eof-in-comment");
                    let text = std::mem::take(&mut self.comment);
                    self.emit(Token::Comment(text));
                    self.finish();
                }
            },
            State::CommentLessThan => match self.consume() {
                Some('!') => {
                    self.comment.push('!');
                    self.state = State::CommentLessThanBang;
                }
                Some('<') => self.comment.push('<'),
                other => {
                    self.unconsume(other);
                    self.state = State::Comment;
                }
            },
            State::CommentLessThanBang => match self.consume() {
                Some('-') => self.state = State::CommentLessThanBangDash,
                other => {
                    self.unconsume(other);
                    self.state = State::Comment;
                }
            },
            State::CommentLessThanBangDash => match self.consume() {
                Some('-') => self.state = State::CommentLessThanBangDashDash,
                other => {
                    self.unconsume(other);
                    self.state = State::CommentEndDash;
                }
            },
            State::CommentLessThanBangDashDash => match self.consume() {
                Some('>') | None => {
                    self.pos = self.pos.saturating_sub(usize::from(
                        self.pos > 0 && self.chars.get(self.pos - 1) == Some(&'>'),
                    ));
                    self.state = State::CommentEnd;
                }
                other => {
                    self.fault("nested-comment");
                    self.unconsume(other);
                    self.state = State::CommentEnd;
                }
            },
            State::CommentEndDash => match self.consume() {
                Some('-') => self.state = State::CommentEnd,
                None => {
                    self.fault("eof-in-comment");
                    let text = std::mem::take(&mut self.comment);
                    self.emit(Token::Comment(text));
                    self.finish();
                }
                other => {
                    self.comment.push('-');
                    self.unconsume(other);
                    self.state = State::Comment;
                }
            },
            State::CommentEnd => match self.consume() {
                Some('>') => {
                    let text = std::mem::take(&mut self.comment);
                    self.emit(Token::Comment(text));
                    self.state = State::Data;
                }
                Some('!') => self.state = State::CommentEndBang,
                Some('-') => self.comment.push('-'),
                None => {
                    self.fault("eof-in-comment");
                    let text = std::mem::take(&mut self.comment);
                    self.emit(Token::Comment(text));
                    self.finish();
                }
                other => {
                    self.comment.push_str("--");
                    self.unconsume(other);
                    self.state = State::Comment;
                }
            },
            State::CommentEndBang => match self.consume() {
                Some('-') => {
                    self.comment.push_str("--!");
                    self.state = State::CommentEndDash;
                }
                Some('>') => {
                    self.fault("incorrectly-closed-comment");
                    let text = std::mem::take(&mut self.comment);
                    self.emit(Token::Comment(text));
                    self.state = State::Data;
                }
                None => {
                    self.fault("eof-in-comment");
                    let text = std::mem::take(&mut self.comment);
                    self.emit(Token::Comment(text));
                    self.finish();
                }
                other => {
                    self.comment.push_str("--!");
                    self.unconsume(other);
                    self.state = State::Comment;
                }
            },
            State::Doctype => match self.consume() {
                Some(character) if is_ascii_space(character) => {
                    self.state = State::BeforeDoctypeName
                }
                Some('>') => {
                    self.unconsume(Some('>'));
                    self.state = State::BeforeDoctypeName;
                }
                None => {
                    self.fault("eof-in-doctype");
                    self.emit_doctype(true);
                    self.finish();
                }
                other => {
                    self.fault("missing-whitespace-before-doctype-name");
                    self.unconsume(other);
                    self.state = State::BeforeDoctypeName;
                }
            },
            State::BeforeDoctypeName => {
                self.doctype_name = None;
                self.doctype_public = None;
                self.doctype_system = None;
                self.doctype_quirks = false;
                match self.consume() {
                    Some(character) if is_ascii_space(character) => {}
                    Some('>') => {
                        self.fault("missing-doctype-name");
                        self.emit_doctype(true);
                        self.state = State::Data;
                    }
                    Some('\0') => {
                        self.fault("unexpected-null-character");
                        self.doctype_name = Some("\u{fffd}".to_string());
                        self.state = State::DoctypeName;
                    }
                    None => {
                        self.fault("eof-in-doctype");
                        self.emit_doctype(true);
                        self.finish();
                    }
                    Some(character) => {
                        self.doctype_name = Some(character.to_ascii_lowercase().to_string());
                        self.state = State::DoctypeName;
                    }
                }
            }
            State::DoctypeName => match self.consume() {
                Some(character) if is_ascii_space(character) => {
                    self.state = State::AfterDoctypeName
                }
                Some('>') => {
                    self.emit_doctype(false);
                    self.state = State::Data;
                }
                Some('\0') => {
                    self.fault("unexpected-null-character");
                    self.doctype_name
                        .get_or_insert_with(String::new)
                        .push('\u{fffd}');
                }
                None => {
                    self.fault("eof-in-doctype");
                    self.emit_doctype(true);
                    self.finish();
                }
                Some(character) => self
                    .doctype_name
                    .get_or_insert_with(String::new)
                    .push(character.to_ascii_lowercase()),
            },
            State::AfterDoctypeName => match self.consume() {
                Some(character) if is_ascii_space(character) => {}
                Some('>') => {
                    self.emit_doctype(false);
                    self.state = State::Data;
                }
                None => {
                    self.fault("eof-in-doctype");
                    self.emit_doctype(true);
                    self.finish();
                }
                Some(character) => {
                    self.unconsume(Some(character));
                    if self.starts_with("public", true) {
                        self.pos += 6;
                        self.state = State::AfterDoctypePublicKeyword;
                    } else if self.starts_with("system", true) {
                        self.pos += 6;
                        self.state = State::AfterDoctypeSystemKeyword;
                    } else {
                        self.fault("invalid-character-sequence-after-doctype-name");
                        self.doctype_quirks = true;
                        self.state = State::BogusDoctype;
                    }
                }
            },
            State::AfterDoctypePublicKeyword => match self.consume() {
                Some(character) if is_ascii_space(character) => {
                    self.state = State::BeforeDoctypePublicId
                }
                Some('"') => {
                    self.fault("missing-whitespace-after-doctype-public-keyword");
                    self.doctype_public = Some(String::new());
                    self.state = State::DoctypePublicIdDouble;
                }
                Some('\'') => {
                    self.fault("missing-whitespace-after-doctype-public-keyword");
                    self.doctype_public = Some(String::new());
                    self.state = State::DoctypePublicIdSingle;
                }
                Some('>') => {
                    self.fault("missing-doctype-public-identifier");
                    self.emit_doctype(true);
                    self.state = State::Data;
                }
                None => {
                    self.fault("eof-in-doctype");
                    self.emit_doctype(true);
                    self.finish();
                }
                other => {
                    self.fault("missing-quote-before-doctype-public-identifier");
                    self.doctype_quirks = true;
                    self.unconsume(other);
                    self.state = State::BogusDoctype;
                }
            },
            State::BeforeDoctypePublicId => match self.consume() {
                Some(character) if is_ascii_space(character) => {}
                Some('"') => {
                    self.doctype_public = Some(String::new());
                    self.state = State::DoctypePublicIdDouble;
                }
                Some('\'') => {
                    self.doctype_public = Some(String::new());
                    self.state = State::DoctypePublicIdSingle;
                }
                Some('>') => {
                    self.fault("missing-doctype-public-identifier");
                    self.emit_doctype(true);
                    self.state = State::Data;
                }
                None => {
                    self.fault("eof-in-doctype");
                    self.emit_doctype(true);
                    self.finish();
                }
                other => {
                    self.fault("missing-quote-before-doctype-public-identifier");
                    self.doctype_quirks = true;
                    self.unconsume(other);
                    self.state = State::BogusDoctype;
                }
            },
            State::DoctypePublicIdDouble | State::DoctypePublicIdSingle => {
                let quote = if self.state == State::DoctypePublicIdDouble {
                    '"'
                } else {
                    '\''
                };
                match self.consume() {
                    Some(character) if character == quote => {
                        self.state = State::AfterDoctypePublicId
                    }
                    Some('\0') => {
                        self.fault("unexpected-null-character");
                        self.doctype_public
                            .get_or_insert_with(String::new)
                            .push('\u{fffd}');
                    }
                    Some('>') => {
                        self.fault("abrupt-doctype-public-identifier");
                        self.emit_doctype(true);
                        self.state = State::Data;
                    }
                    None => {
                        self.fault("eof-in-doctype");
                        self.emit_doctype(true);
                        self.finish();
                    }
                    Some(character) => self
                        .doctype_public
                        .get_or_insert_with(String::new)
                        .push(character),
                }
            }
            State::AfterDoctypePublicId => match self.consume() {
                Some(character) if is_ascii_space(character) => {
                    self.state = State::BetweenDoctypeIds
                }
                Some('>') => {
                    self.emit_doctype(false);
                    self.state = State::Data;
                }
                Some('"') => {
                    self.fault("missing-whitespace-between-doctype-public-and-system-identifiers");
                    self.doctype_system = Some(String::new());
                    self.state = State::DoctypeSystemIdDouble;
                }
                Some('\'') => {
                    self.fault("missing-whitespace-between-doctype-public-and-system-identifiers");
                    self.doctype_system = Some(String::new());
                    self.state = State::DoctypeSystemIdSingle;
                }
                None => {
                    self.fault("eof-in-doctype");
                    self.emit_doctype(true);
                    self.finish();
                }
                other => {
                    self.fault("missing-quote-before-doctype-system-identifier");
                    self.doctype_quirks = true;
                    self.unconsume(other);
                    self.state = State::BogusDoctype;
                }
            },
            State::BetweenDoctypeIds => match self.consume() {
                Some(character) if is_ascii_space(character) => {}
                Some('>') => {
                    self.emit_doctype(false);
                    self.state = State::Data;
                }
                Some('"') => {
                    self.doctype_system = Some(String::new());
                    self.state = State::DoctypeSystemIdDouble;
                }
                Some('\'') => {
                    self.doctype_system = Some(String::new());
                    self.state = State::DoctypeSystemIdSingle;
                }
                None => {
                    self.fault("eof-in-doctype");
                    self.emit_doctype(true);
                    self.finish();
                }
                other => {
                    self.fault("missing-quote-before-doctype-system-identifier");
                    self.doctype_quirks = true;
                    self.unconsume(other);
                    self.state = State::BogusDoctype;
                }
            },
            State::AfterDoctypeSystemKeyword => match self.consume() {
                Some(character) if is_ascii_space(character) => {
                    self.state = State::BeforeDoctypeSystemId
                }
                Some('"') => {
                    self.fault("missing-whitespace-after-doctype-system-keyword");
                    self.doctype_system = Some(String::new());
                    self.state = State::DoctypeSystemIdDouble;
                }
                Some('\'') => {
                    self.fault("missing-whitespace-after-doctype-system-keyword");
                    self.doctype_system = Some(String::new());
                    self.state = State::DoctypeSystemIdSingle;
                }
                Some('>') => {
                    self.fault("missing-doctype-system-identifier");
                    self.emit_doctype(true);
                    self.state = State::Data;
                }
                None => {
                    self.fault("eof-in-doctype");
                    self.emit_doctype(true);
                    self.finish();
                }
                other => {
                    self.fault("missing-quote-before-doctype-system-identifier");
                    self.doctype_quirks = true;
                    self.unconsume(other);
                    self.state = State::BogusDoctype;
                }
            },
            State::BeforeDoctypeSystemId => match self.consume() {
                Some(character) if is_ascii_space(character) => {}
                Some('"') => {
                    self.doctype_system = Some(String::new());
                    self.state = State::DoctypeSystemIdDouble;
                }
                Some('\'') => {
                    self.doctype_system = Some(String::new());
                    self.state = State::DoctypeSystemIdSingle;
                }
                Some('>') => {
                    self.fault("missing-doctype-system-identifier");
                    self.emit_doctype(true);
                    self.state = State::Data;
                }
                None => {
                    self.fault("eof-in-doctype");
                    self.emit_doctype(true);
                    self.finish();
                }
                other => {
                    self.fault("missing-quote-before-doctype-system-identifier");
                    self.doctype_quirks = true;
                    self.unconsume(other);
                    self.state = State::BogusDoctype;
                }
            },
            State::DoctypeSystemIdDouble | State::DoctypeSystemIdSingle => {
                let quote = if self.state == State::DoctypeSystemIdDouble {
                    '"'
                } else {
                    '\''
                };
                match self.consume() {
                    Some(character) if character == quote => {
                        self.state = State::AfterDoctypeSystemId
                    }
                    Some('\0') => {
                        self.fault("unexpected-null-character");
                        self.doctype_system
                            .get_or_insert_with(String::new)
                            .push('\u{fffd}');
                    }
                    Some('>') => {
                        self.fault("abrupt-doctype-system-identifier");
                        self.emit_doctype(true);
                        self.state = State::Data;
                    }
                    None => {
                        self.fault("eof-in-doctype");
                        self.emit_doctype(true);
                        self.finish();
                    }
                    Some(character) => self
                        .doctype_system
                        .get_or_insert_with(String::new)
                        .push(character),
                }
            }
            State::AfterDoctypeSystemId => match self.consume() {
                Some(character) if is_ascii_space(character) => {}
                Some('>') => {
                    self.emit_doctype(false);
                    self.state = State::Data;
                }
                None => {
                    self.fault("eof-in-doctype");
                    self.emit_doctype(true);
                    self.finish();
                }
                other => {
                    self.fault("unexpected-character-after-doctype-system-identifier");
                    self.unconsume(other);
                    self.state = State::BogusDoctype;
                }
            },
            State::BogusDoctype => match self.consume() {
                Some('>') => {
                    self.emit_doctype(false);
                    self.state = State::Data;
                }
                Some('\0') => self.fault("unexpected-null-character"),
                None => {
                    self.emit_doctype(false);
                    self.finish();
                }
                Some(_) => {}
            },
            State::CdataSection => match self.consume() {
                Some(']') => self.state = State::CdataSectionBracket,
                None => {
                    self.fault("eof-in-cdata");
                    self.finish();
                }
                Some(character) => self.emit(Token::Char(character)),
            },
            State::CdataSectionBracket => match self.consume() {
                Some(']') => self.state = State::CdataSectionEnd,
                other => {
                    self.emit(Token::Char(']'));
                    self.unconsume(other);
                    self.state = State::CdataSection;
                }
            },
            State::CdataSectionEnd => match self.consume() {
                Some(']') => self.emit(Token::Char(']')),
                Some('>') => self.state = State::Data,
                other => {
                    self.emit_str("]]");
                    self.unconsume(other);
                    self.state = State::CdataSection;
                }
            },
        }
    }

    fn raw_end_tag_name(&mut self, fallback: State) {
        match self.consume() {
            Some(character) if is_ascii_space(character) && self.appropriate_end() => {
                self.state = State::BeforeAttributeName
            }
            Some('/') if self.appropriate_end() => self.state = State::SelfClosingStartTag,
            Some('>') if self.appropriate_end() => {
                self.emit_tag();
                self.state = State::Data;
            }
            Some(character) if character.is_ascii_alphabetic() => {
                self.tag_name.push(character.to_ascii_lowercase());
                self.buffer.push(character);
            }
            other => {
                self.emit_str("</");
                let held = std::mem::take(&mut self.buffer);
                self.emit_str(&held);
                self.unconsume(other);
                self.state = fallback;
            }
        }
    }

    fn emit_doctype(&mut self, force_quirks: bool) {
        let token = Token::Doctype {
            name: self.doctype_name.take(),
            public_id: self.doctype_public.take(),
            system_id: self.doctype_system.take(),
            force_quirks: force_quirks || self.doctype_quirks,
        };
        self.emit(token);
        self.doctype_quirks = false;
    }

    fn finish(&mut self) {
        self.finished = true;
        self.emit(Token::Eof);
    }

    fn next_token(&mut self) -> Token {
        loop {
            if let Some(token) = self.pending.pop_front() {
                return token;
            }
            if self.finished {
                return Token::Eof;
            }
            self.step();
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Initial,
    BeforeHtml,
    BeforeHead,
    InHead,
    InHeadNoscript,
    AfterHead,
    InBody,
    Text,
    InTable,
    InTableText,
    InCaption,
    InColumnGroup,
    InTableBody,
    InRow,
    InCell,
    InSelect,
    InSelectInTable,
    InTemplate,
    AfterBody,
    InFrameset,
    AfterFrameset,
    AfterAfterBody,
    AfterAfterFrameset,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Default,
    ListItem,
    Button,
    Table,
    Select,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Listed {
    Marker,
    Element(NodeId),
}

const SPECIAL_HTML: &[&str] = &[
    "address",
    "applet",
    "area",
    "article",
    "aside",
    "base",
    "basefont",
    "bgsound",
    "blockquote",
    "body",
    "br",
    "button",
    "caption",
    "center",
    "col",
    "colgroup",
    "dd",
    "details",
    "dir",
    "div",
    "dl",
    "dt",
    "embed",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "form",
    "frame",
    "frameset",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "head",
    "header",
    "hgroup",
    "hr",
    "html",
    "iframe",
    "img",
    "input",
    "keygen",
    "li",
    "link",
    "listing",
    "main",
    "marquee",
    "menu",
    "meta",
    "nav",
    "noembed",
    "noframes",
    "noscript",
    "object",
    "ol",
    "p",
    "param",
    "plaintext",
    "pre",
    "script",
    "search",
    "section",
    "select",
    "source",
    "style",
    "summary",
    "table",
    "tbody",
    "td",
    "template",
    "textarea",
    "tfoot",
    "th",
    "thead",
    "title",
    "tr",
    "track",
    "ul",
    "wbr",
    "xmp",
];

const FORMATTING: &[&str] = &[
    "a", "b", "big", "code", "em", "font", "i", "nobr", "s", "small", "strike", "strong", "tt", "u",
];

const SVG_TAGS: &[(&str, &str)] = &[
    ("altglyph", "altGlyph"),
    ("altglyphdef", "altGlyphDef"),
    ("altglyphitem", "altGlyphItem"),
    ("animatecolor", "animateColor"),
    ("animatemotion", "animateMotion"),
    ("animatetransform", "animateTransform"),
    ("clippath", "clipPath"),
    ("feblend", "feBlend"),
    ("fecolormatrix", "feColorMatrix"),
    ("fecomponenttransfer", "feComponentTransfer"),
    ("fecomposite", "feComposite"),
    ("feconvolvematrix", "feConvolveMatrix"),
    ("fediffuselighting", "feDiffuseLighting"),
    ("fedisplacementmap", "feDisplacementMap"),
    ("fedistantlight", "feDistantLight"),
    ("fedropshadow", "feDropShadow"),
    ("feflood", "feFlood"),
    ("fefunca", "feFuncA"),
    ("fefuncb", "feFuncB"),
    ("fefuncg", "feFuncG"),
    ("fefuncr", "feFuncR"),
    ("fegaussianblur", "feGaussianBlur"),
    ("feimage", "feImage"),
    ("femerge", "feMerge"),
    ("femergenode", "feMergeNode"),
    ("femorphology", "feMorphology"),
    ("feoffset", "feOffset"),
    ("fepointlight", "fePointLight"),
    ("fespecularlighting", "feSpecularLighting"),
    ("fespotlight", "feSpotLight"),
    ("fetile", "feTile"),
    ("feturbulence", "feTurbulence"),
    ("foreignobject", "foreignObject"),
    ("glyphref", "glyphRef"),
    ("lineargradient", "linearGradient"),
    ("radialgradient", "radialGradient"),
    ("textpath", "textPath"),
];

const SVG_ATTRIBUTES: &[(&str, &str)] = &[
    ("attributename", "attributeName"),
    ("attributetype", "attributeType"),
    ("basefrequency", "baseFrequency"),
    ("baseprofile", "baseProfile"),
    ("calcmode", "calcMode"),
    ("clippathunits", "clipPathUnits"),
    ("diffuseconstant", "diffuseConstant"),
    ("edgemode", "edgeMode"),
    ("filterunits", "filterUnits"),
    ("glyphref", "glyphRef"),
    ("gradienttransform", "gradientTransform"),
    ("gradientunits", "gradientUnits"),
    ("kernelmatrix", "kernelMatrix"),
    ("kernelunitlength", "kernelUnitLength"),
    ("keypoints", "keyPoints"),
    ("keysplines", "keySplines"),
    ("keytimes", "keyTimes"),
    ("lengthadjust", "lengthAdjust"),
    ("limitingconeangle", "limitingConeAngle"),
    ("markerheight", "markerHeight"),
    ("markerunits", "markerUnits"),
    ("markerwidth", "markerWidth"),
    ("maskcontentunits", "maskContentUnits"),
    ("maskunits", "maskUnits"),
    ("numoctaves", "numOctaves"),
    ("pathlength", "pathLength"),
    ("patterncontentunits", "patternContentUnits"),
    ("patterntransform", "patternTransform"),
    ("patternunits", "patternUnits"),
    ("pointsatx", "pointsAtX"),
    ("pointsaty", "pointsAtY"),
    ("pointsatz", "pointsAtZ"),
    ("preservealpha", "preserveAlpha"),
    ("preserveaspectratio", "preserveAspectRatio"),
    ("primitiveunits", "primitiveUnits"),
    ("refx", "refX"),
    ("refy", "refY"),
    ("repeatcount", "repeatCount"),
    ("repeatdur", "repeatDur"),
    ("requiredextensions", "requiredExtensions"),
    ("requiredfeatures", "requiredFeatures"),
    ("specularconstant", "specularConstant"),
    ("specularexponent", "specularExponent"),
    ("spreadmethod", "spreadMethod"),
    ("startoffset", "startOffset"),
    ("stddeviation", "stdDeviation"),
    ("stitchtiles", "stitchTiles"),
    ("surfacescale", "surfaceScale"),
    ("systemlanguage", "systemLanguage"),
    ("tablevalues", "tableValues"),
    ("targetx", "targetX"),
    ("targety", "targetY"),
    ("textlength", "textLength"),
    ("viewbox", "viewBox"),
    ("viewtarget", "viewTarget"),
    ("xchannelselector", "xChannelSelector"),
    ("ychannelselector", "yChannelSelector"),
    ("zoomandpan", "zoomAndPan"),
];

const QUIRKY_PUBLIC_PREFIXES: &[&str] = &[
    "+//silmaril//dtd html pro v0r11 19970101//",
    "-//as//dtd html 3.0 aswedit + extensions//",
    "-//advasoft ltd//dtd html 3.0 aswedit + extensions//",
    "-//ietf//dtd html 2.0 level 1//",
    "-//ietf//dtd html 2.0 level 2//",
    "-//ietf//dtd html 2.0 strict level 1//",
    "-//ietf//dtd html 2.0 strict level 2//",
    "-//ietf//dtd html 2.0 strict//",
    "-//ietf//dtd html 2.0//",
    "-//ietf//dtd html 2.1e//",
    "-//ietf//dtd html 3.0//",
    "-//ietf//dtd html 3.2 final//",
    "-//ietf//dtd html 3.2//",
    "-//ietf//dtd html 3//",
    "-//ietf//dtd html level 0//",
    "-//ietf//dtd html level 1//",
    "-//ietf//dtd html level 2//",
    "-//ietf//dtd html level 3//",
    "-//ietf//dtd html strict level 0//",
    "-//ietf//dtd html strict level 1//",
    "-//ietf//dtd html strict level 2//",
    "-//ietf//dtd html strict level 3//",
    "-//ietf//dtd html strict//",
    "-//ietf//dtd html//",
    "-//metrius//dtd metrius presentational//",
    "-//microsoft//dtd internet explorer 2.0 html strict//",
    "-//microsoft//dtd internet explorer 2.0 html//",
    "-//microsoft//dtd internet explorer 2.0 tables//",
    "-//microsoft//dtd internet explorer 3.0 html strict//",
    "-//microsoft//dtd internet explorer 3.0 html//",
    "-//microsoft//dtd internet explorer 3.0 tables//",
    "-//netscape comm. corp.//dtd html//",
    "-//netscape comm. corp.//dtd strict html//",
    "-//o'reilly and associates//dtd html 2.0//",
    "-//o'reilly and associates//dtd html extended 1.0//",
    "-//o'reilly and associates//dtd html extended relaxed 1.0//",
    "-//sq//dtd html 2.0 hotmetal + extensions//",
    "-//softquad software//dtd hotmetal pro 6.0::19990601::extensions to html 4.0//",
    "-//softquad//dtd hotmetal pro 4.0::19971010::extensions to html 4.0//",
    "-//spyglass//dtd html 2.0 extended//",
    "-//sun microsystems corp.//dtd hotjava html//",
    "-//sun microsystems corp.//dtd hotjava strict html//",
    "-//w3c//dtd html 3 1995-03-24//",
    "-//w3c//dtd html 3.2 draft//",
    "-//w3c//dtd html 3.2 final//",
    "-//w3c//dtd html 3.2//",
    "-//w3c//dtd html 3.2s draft//",
    "-//w3c//dtd html 4.0 frameset//",
    "-//w3c//dtd html 4.0 transitional//",
    "-//w3c//dtd html experimental 19960712//",
    "-//w3c//dtd html experimental 970421//",
    "-//w3c//dtd w3 html//",
    "-//w3o//dtd w3 html 3.0//",
    "-//webtechs//dtd mozilla html 2.0//",
    "-//webtechs//dtd mozilla html//",
];

fn plain(attributes: &[(String, String)]) -> Vec<Attribute> {
    attributes
        .iter()
        .map(|(name, value)| Attribute {
            prefix: None,
            name: name.clone(),
            value: value.clone(),
        })
        .collect()
}

fn adjust_mathml(attributes: &mut [Attribute]) {
    for attribute in attributes {
        if attribute.name == "definitionurl" {
            attribute.name = "definitionURL".to_string();
        }
    }
}

fn adjust_svg_attributes(attributes: &mut [Attribute]) {
    for attribute in attributes {
        if let Some((_, fixed)) = SVG_ATTRIBUTES
            .iter()
            .find(|(held, _)| *held == attribute.name)
        {
            attribute.name = (*fixed).to_string();
        }
    }
}

fn adjust_foreign(attributes: &mut [Attribute]) {
    for attribute in attributes {
        let prefix: Option<&'static str> = if attribute.prefix.is_some() {
            None
        } else if let Some(rest) = attribute.name.strip_prefix("xlink:") {
            if matches!(
                rest,
                "actuate" | "arcrole" | "href" | "role" | "show" | "title" | "type"
            ) {
                Some("xlink")
            } else {
                None
            }
        } else if let Some(rest) = attribute.name.strip_prefix("xml:") {
            if matches!(rest, "lang" | "space") {
                Some("xml")
            } else {
                None
            }
        } else if attribute.name == "xmlns:xlink" {
            Some("xmlns")
        } else {
            None
        };
        if let Some(prefix) = prefix {
            let local = attribute.name[prefix.len() + 1..].to_string();
            attribute.prefix = Some(prefix);
            attribute.name = local;
        }
    }
}

fn quirks_for(
    name: &Option<String>,
    public_id: &Option<String>,
    system_id: &Option<String>,
    force: bool,
) -> Quirks {
    let public = public_id.as_deref().unwrap_or("").to_ascii_lowercase();
    let has_public = public_id.is_some();
    let has_system = system_id.is_some();
    let system = system_id.as_deref().unwrap_or("").to_ascii_lowercase();
    if force
        || name.as_deref() != Some("html")
        || (has_public
            && (public == "-//w3o//dtd w3 html strict 3.0//en//"
                || public == "-/w3c/dtd html 4.0 transitional/en"
                || public == "html"
                || QUIRKY_PUBLIC_PREFIXES
                    .iter()
                    .any(|prefix| public.starts_with(prefix))))
        || system == "http://www.ibm.com/data/dtd/v11/ibmxhtml1-transitional.dtd"
        || (!has_system
            && has_public
            && (public.starts_with("-//w3c//dtd html 4.01 frameset//")
                || public.starts_with("-//w3c//dtd html 4.01 transitional//")))
    {
        return Quirks::Full;
    }
    if has_public
        && (public.starts_with("-//w3c//dtd xhtml 1.0 frameset//")
            || public.starts_with("-//w3c//dtd xhtml 1.0 transitional//")
            || (has_system
                && (public.starts_with("-//w3c//dtd html 4.01 frameset//")
                    || public.starts_with("-//w3c//dtd html 4.01 transitional//"))))
    {
        return Quirks::Limited;
    }
    Quirks::No
}

struct TreeBuilder {
    tok: Tokenizer,
    doc: Document,
    mode: Mode,
    original: Mode,
    open: Vec<NodeId>,
    listed: Vec<Listed>,
    templates: Vec<Mode>,
    head: Option<NodeId>,
    form: Option<NodeId>,
    frameset_ok: bool,
    scripting: bool,
    foster: bool,
    table_text: Vec<char>,
    context: Option<NodeId>,
    skip_lf: bool,
    stopped: bool,
    acknowledged: bool,
    errors: Vec<ParseError>,
}

impl TreeBuilder {
    fn new(source: &str, scripting: bool) -> TreeBuilder {
        TreeBuilder {
            tok: Tokenizer::new(source.strip_prefix('\u{feff}').unwrap_or(source)),
            doc: Document::new(),
            mode: Mode::Initial,
            original: Mode::Initial,
            open: Vec::new(),
            listed: Vec::new(),
            templates: Vec::new(),
            head: None,
            form: None,
            frameset_ok: true,
            scripting,
            foster: false,
            table_text: Vec::new(),
            context: None,
            skip_lf: false,
            stopped: false,
            acknowledged: false,
            errors: Vec::new(),
        }
    }

    fn fault(&mut self, code: &'static str) {
        self.errors.push(ParseError {
            code,
            offset: self.tok.pos,
        });
    }

    fn run(&mut self) {
        while !self.stopped {
            self.tok.cdata_allowed = self
                .adjusted_current()
                .is_some_and(|id| self.namespace_of(id) != Namespace::Html);
            let token = self.tok.next_token();
            if self.skip_lf {
                self.skip_lf = false;
                if token == Token::Char('\n') {
                    continue;
                }
            }
            self.dispatch(token);
        }
    }

    fn namespace_of(&self, id: NodeId) -> Namespace {
        match &self.doc.nodes[id].kind {
            NodeKind::Element { namespace, .. } => *namespace,
            _ => Namespace::Html,
        }
    }

    fn name_of(&self, id: NodeId) -> &str {
        match &self.doc.nodes[id].kind {
            NodeKind::Element { name, .. } => name.as_str(),
            _ => "",
        }
    }

    fn is(&self, id: NodeId, wanted: &str) -> bool {
        match &self.doc.nodes[id].kind {
            NodeKind::Element {
                namespace: Namespace::Html,
                name,
                ..
            } => name == wanted,
            _ => false,
        }
    }

    fn is_one_of(&self, id: NodeId, wanted: &[&str]) -> bool {
        match &self.doc.nodes[id].kind {
            NodeKind::Element {
                namespace: Namespace::Html,
                name,
                ..
            } => wanted.contains(&name.as_str()),
            _ => false,
        }
    }

    fn current(&self) -> Option<NodeId> {
        self.open.last().copied()
    }

    fn current_is(&self, wanted: &str) -> bool {
        self.current().is_some_and(|id| self.is(id, wanted))
    }

    fn current_is_one_of(&self, wanted: &[&str]) -> bool {
        self.current().is_some_and(|id| self.is_one_of(id, wanted))
    }

    fn adjusted_current(&self) -> Option<NodeId> {
        if self.open.len() == 1 {
            if let Some(context) = self.context {
                return Some(context);
            }
        }
        self.current()
    }

    fn is_special(&self, id: NodeId) -> bool {
        match &self.doc.nodes[id].kind {
            NodeKind::Element {
                namespace, name, ..
            } => match namespace {
                Namespace::Html => SPECIAL_HTML.contains(&name.as_str()),
                Namespace::MathMl => {
                    matches!(
                        name.as_str(),
                        "mi" | "mo" | "mn" | "ms" | "mtext" | "annotation-xml"
                    )
                }
                Namespace::Svg => matches!(name.as_str(), "foreignObject" | "desc" | "title"),
            },
            _ => false,
        }
    }

    fn math_text_point(&self, id: NodeId) -> bool {
        self.namespace_of(id) == Namespace::MathMl
            && matches!(self.name_of(id), "mi" | "mo" | "mn" | "ms" | "mtext")
    }

    fn html_point(&self, id: NodeId) -> bool {
        match self.namespace_of(id) {
            Namespace::MathMl => {
                self.name_of(id) == "annotation-xml"
                    && self.doc.attribute(id, "encoding").is_some_and(|value| {
                        value.eq_ignore_ascii_case("text/html")
                            || value.eq_ignore_ascii_case("application/xhtml+xml")
                    })
            }
            Namespace::Svg => matches!(self.name_of(id), "foreignObject" | "desc" | "title"),
            Namespace::Html => false,
        }
    }

    fn boundary(&self, id: NodeId, scope: Scope) -> bool {
        let namespace = self.namespace_of(id);
        let name = self.name_of(id);
        match namespace {
            Namespace::Html => {
                let base = matches!(
                    name,
                    "applet"
                        | "caption"
                        | "html"
                        | "table"
                        | "td"
                        | "th"
                        | "marquee"
                        | "object"
                        | "template"
                );
                match scope {
                    Scope::Default => base,
                    Scope::ListItem => base || matches!(name, "ol" | "ul"),
                    Scope::Button => base || name == "button",
                    Scope::Table => matches!(name, "html" | "table" | "template"),
                    Scope::Select => !matches!(name, "optgroup" | "option"),
                }
            }
            Namespace::MathMl => {
                matches!(scope, Scope::Default | Scope::ListItem | Scope::Button)
                    && matches!(name, "mi" | "mo" | "mn" | "ms" | "mtext" | "annotation-xml")
            }
            Namespace::Svg => {
                scope == Scope::Select
                    || (matches!(scope, Scope::Default | Scope::ListItem | Scope::Button)
                        && matches!(name, "foreignObject" | "desc" | "title"))
            }
        }
    }

    fn in_scope(&self, wanted: &str, scope: Scope) -> bool {
        for &id in self.open.iter().rev() {
            if self.is(id, wanted) {
                return true;
            }
            if self.boundary(id, scope) {
                return false;
            }
        }
        false
    }

    fn in_scope_one_of(&self, wanted: &[&str], scope: Scope) -> bool {
        for &id in self.open.iter().rev() {
            if self.is_one_of(id, wanted) {
                return true;
            }
            if self.boundary(id, scope) {
                return false;
            }
        }
        false
    }

    fn node_in_scope(&self, target: NodeId, scope: Scope) -> bool {
        for &id in self.open.iter().rev() {
            if id == target {
                return true;
            }
            if self.boundary(id, scope) {
                return false;
            }
        }
        false
    }

    fn has_template(&self) -> bool {
        self.open.iter().any(|id| self.is(*id, "template"))
    }

    fn make(&mut self, namespace: Namespace, name: &str, attributes: Vec<Attribute>) -> NodeId {
        let contents = if namespace == Namespace::Html && name == "template" {
            Some(self.doc.create(NodeKind::Fragment))
        } else {
            None
        };
        self.doc.create(NodeKind::Element {
            namespace,
            name: name.to_string(),
            attributes,
            contents,
        })
    }

    fn place(&self, target: Option<NodeId>) -> (NodeId, Option<NodeId>) {
        let target = target.or_else(|| self.current()).unwrap_or(0);
        if self.foster && self.is_one_of(target, &["table", "tbody", "tfoot", "thead", "tr"]) {
            let template = self.open.iter().rposition(|id| self.is(*id, "template"));
            let table = self.open.iter().rposition(|id| self.is(*id, "table"));
            if let Some(at) = template {
                if table.is_none_or(|held| at > held) {
                    return (self.contents_of(self.open[at]), None);
                }
            }
            let Some(at) = table else {
                return (self.open[0], None);
            };
            let table = self.open[at];
            if let Some(parent) = self.doc.nodes[table].parent {
                return (parent, Some(table));
            }
            return (self.open[at.saturating_sub(1)], None);
        }
        if self.is(target, "template") {
            return (self.contents_of(target), None);
        }
        (target, None)
    }

    fn contents_of(&self, id: NodeId) -> NodeId {
        match &self.doc.nodes[id].kind {
            NodeKind::Element {
                contents: Some(inner),
                ..
            } => *inner,
            _ => id,
        }
    }

    fn put(&mut self, at: (NodeId, Option<NodeId>), child: NodeId) {
        match at.1 {
            Some(before) => self.doc.insert_before(at.0, child, before),
            None => self.doc.append(at.0, child),
        }
    }

    fn insert_element(
        &mut self,
        namespace: Namespace,
        name: &str,
        attributes: Vec<Attribute>,
    ) -> NodeId {
        let id = self.make(namespace, name, attributes);
        let at = self.place(None);
        self.put(at, id);
        self.open.push(id);
        id
    }

    fn insert_html(&mut self, name: &str, attributes: &[(String, String)]) -> NodeId {
        self.insert_element(Namespace::Html, name, plain(attributes))
    }

    fn insert_bare(&mut self, name: &str) -> NodeId {
        self.insert_element(Namespace::Html, name, Vec::new())
    }

    fn insert_leaf(&mut self, name: &str, attributes: &[(String, String)]) {
        self.insert_html(name, attributes);
        self.open.pop();
        self.acknowledged = true;
    }

    fn insert_char(&mut self, character: char) {
        let (parent, before) = self.place(None);
        if matches!(self.doc.nodes[parent].kind, NodeKind::Document) {
            return;
        }
        let previous = match before {
            Some(held) => {
                let at = self.doc.nodes[parent]
                    .children
                    .iter()
                    .position(|child| *child == held)
                    .unwrap_or(0);
                if at == 0 {
                    None
                } else {
                    Some(self.doc.nodes[parent].children[at - 1])
                }
            }
            None => self.doc.nodes[parent].children.last().copied(),
        };
        if let Some(previous) = previous {
            if let NodeKind::Text(text) = &mut self.doc.nodes[previous].kind {
                text.push(character);
                return;
            }
        }
        let id = self.doc.create(NodeKind::Text(character.to_string()));
        self.put((parent, before), id);
    }

    fn insert_comment(&mut self, data: &str) {
        let at = self.place(None);
        let id = self.doc.create(NodeKind::Comment(data.to_string()));
        self.put(at, id);
    }

    fn append_comment(&mut self, parent: NodeId, data: &str) {
        let id = self.doc.create(NodeKind::Comment(data.to_string()));
        self.doc.append(parent, id);
    }

    fn pop_until(&mut self, wanted: &str) {
        while let Some(id) = self.open.pop() {
            if self.is(id, wanted) {
                break;
            }
        }
    }

    fn pop_until_one_of(&mut self, wanted: &[&str]) {
        while let Some(id) = self.open.pop() {
            if self.is_one_of(id, wanted) {
                break;
            }
        }
    }

    fn pop_through(&mut self, target: NodeId) {
        while let Some(id) = self.open.pop() {
            if id == target {
                break;
            }
        }
    }

    fn remove_from_stack(&mut self, target: NodeId) {
        self.open.retain(|id| *id != target);
    }

    fn generate_implied_end_tags(&mut self, except: Option<&str>) {
        while let Some(id) = self.current() {
            if self.is_one_of(
                id,
                &[
                    "dd", "dt", "li", "optgroup", "option", "p", "rb", "rp", "rt", "rtc",
                ],
            ) && except.is_none_or(|name| !self.is(id, name))
            {
                self.open.pop();
            } else {
                break;
            }
        }
    }

    fn generate_all_implied_end_tags(&mut self) {
        while let Some(id) = self.current() {
            if self.is_one_of(
                id,
                &[
                    "caption", "colgroup", "dd", "dt", "li", "optgroup", "option", "p", "rb", "rp",
                    "rt", "rtc", "tbody", "td", "tfoot", "th", "thead", "tr",
                ],
            ) {
                self.open.pop();
            } else {
                break;
            }
        }
    }

    fn close_p(&mut self) {
        self.generate_implied_end_tags(Some("p"));
        if !self.current_is("p") {
            self.fault("unexpected-open-element");
        }
        self.pop_until("p");
    }

    fn close_p_in_button_scope(&mut self) {
        if self.in_scope("p", Scope::Button) {
            self.close_p();
        }
    }

    fn generic_text(&mut self, name: &str, attributes: &[(String, String)], state: State) {
        self.insert_html(name, attributes);
        self.tok.state = state;
        self.original = self.mode;
        self.mode = Mode::Text;
    }

    fn clear_to_marker(&mut self) {
        while let Some(entry) = self.listed.pop() {
            if entry == Listed::Marker {
                break;
            }
        }
    }

    fn clone_attributes(&self, id: NodeId) -> Vec<Attribute> {
        match &self.doc.nodes[id].kind {
            NodeKind::Element { attributes, .. } => attributes.clone(),
            _ => Vec::new(),
        }
    }

    fn same_element(&self, left: NodeId, right: NodeId) -> bool {
        let (
            NodeKind::Element {
                namespace: first_ns,
                name: first_name,
                attributes: first,
                ..
            },
            NodeKind::Element {
                namespace: second_ns,
                name: second_name,
                attributes: second,
                ..
            },
        ) = (&self.doc.nodes[left].kind, &self.doc.nodes[right].kind)
        else {
            return false;
        };
        first_ns == second_ns
            && first_name == second_name
            && first.len() == second.len()
            && first.iter().all(|attribute| second.contains(attribute))
    }

    fn push_listed(&mut self, id: NodeId) {
        let mut matches: Vec<usize> = Vec::new();
        for (at, entry) in self.listed.iter().enumerate().rev() {
            match entry {
                Listed::Marker => break,
                Listed::Element(held) => {
                    if self.same_element(*held, id) {
                        matches.push(at);
                    }
                }
            }
        }
        if matches.len() >= 3 {
            let earliest = *matches.last().unwrap_or(&0);
            self.listed.remove(earliest);
        }
        self.listed.push(Listed::Element(id));
    }

    fn reconstruct(&mut self) {
        let Some(&last) = self.listed.last() else {
            return;
        };
        let Listed::Element(held) = last else {
            return;
        };
        if self.open.contains(&held) {
            return;
        }
        let mut at = self.listed.len() - 1;
        while at > 0 {
            match self.listed[at - 1] {
                Listed::Marker => break,
                Listed::Element(previous) => {
                    if self.open.contains(&previous) {
                        break;
                    }
                    at -= 1;
                }
            }
        }
        while at < self.listed.len() {
            let Listed::Element(old) = self.listed[at] else {
                break;
            };
            let name = self.name_of(old).to_string();
            let attributes = self.clone_attributes(old);
            let fresh = self.insert_element(Namespace::Html, &name, attributes);
            self.listed[at] = Listed::Element(fresh);
            at += 1;
        }
    }

    fn reset_mode(&mut self) {
        let mut last = false;
        let mut at = self.open.len();
        while at > 0 {
            at -= 1;
            let mut node = self.open[at];
            if at == 0 {
                last = true;
                if let Some(context) = self.context {
                    node = context;
                }
            }
            if self.namespace_of(node) == Namespace::Html {
                match self.name_of(node) {
                    "select" => {
                        if !last {
                            let mut above = at;
                            while above > 0 {
                                above -= 1;
                                let ancestor = self.open[above];
                                if self.is(ancestor, "template") {
                                    break;
                                }
                                if self.is(ancestor, "table") {
                                    self.mode = Mode::InSelectInTable;
                                    return;
                                }
                            }
                        }
                        self.mode = Mode::InSelect;
                        return;
                    }
                    "td" | "th" if !last => {
                        self.mode = Mode::InCell;
                        return;
                    }
                    "tr" => {
                        self.mode = Mode::InRow;
                        return;
                    }
                    "tbody" | "thead" | "tfoot" => {
                        self.mode = Mode::InTableBody;
                        return;
                    }
                    "caption" => {
                        self.mode = Mode::InCaption;
                        return;
                    }
                    "colgroup" => {
                        self.mode = Mode::InColumnGroup;
                        return;
                    }
                    "table" => {
                        self.mode = Mode::InTable;
                        return;
                    }
                    "template" => {
                        self.mode = self.templates.last().copied().unwrap_or(Mode::InTemplate);
                        return;
                    }
                    "head" if !last => {
                        self.mode = Mode::InHead;
                        return;
                    }
                    "body" => {
                        self.mode = Mode::InBody;
                        return;
                    }
                    "frameset" => {
                        self.mode = Mode::InFrameset;
                        return;
                    }
                    "html" => {
                        self.mode = if self.head.is_none() {
                            Mode::BeforeHead
                        } else {
                            Mode::AfterHead
                        };
                        return;
                    }
                    _ => {}
                }
            }
            if last {
                self.mode = Mode::InBody;
                return;
            }
        }
        self.mode = Mode::InBody;
    }

    fn run_adoption_agency(&mut self, subject: &str) -> bool {
        if let Some(current) = self.current() {
            if self.is(current, subject) && !self.listed.contains(&Listed::Element(current)) {
                self.open.pop();
                return true;
            }
        }
        for _ in 0..8 {
            let mut found: Option<(usize, NodeId)> = None;
            for (at, entry) in self.listed.iter().enumerate().rev() {
                match entry {
                    Listed::Marker => break,
                    Listed::Element(id) => {
                        if self.is(*id, subject) {
                            found = Some((at, *id));
                            break;
                        }
                    }
                }
            }
            let Some((list_at, element)) = found else {
                return false;
            };
            let Some(stack_at) = self.open.iter().position(|id| *id == element) else {
                self.fault("adoption-agency-missing");
                self.listed.remove(list_at);
                return true;
            };
            if !self.node_in_scope(element, Scope::Default) {
                self.fault("adoption-agency-scope");
                return true;
            }
            if self.current() != Some(element) {
                self.fault("adoption-agency-not-current");
            }
            let mut furthest: Option<usize> = None;
            for at in stack_at + 1..self.open.len() {
                if self.is_special(self.open[at]) {
                    furthest = Some(at);
                    break;
                }
            }
            let Some(furthest_at) = furthest else {
                self.open.truncate(stack_at);
                self.listed.remove(list_at);
                return true;
            };
            let furthest_block = self.open[furthest_at];
            let common = self.open[stack_at - 1];
            let mut bookmark = list_at;
            let mut node_at = furthest_at;
            let mut last_node = furthest_block;
            let mut inner = 0;
            loop {
                inner += 1;
                node_at -= 1;
                let node = self.open[node_at];
                if node == element {
                    break;
                }
                let mut in_list = self
                    .listed
                    .iter()
                    .position(|entry| *entry == Listed::Element(node));
                if inner > 3 {
                    if let Some(held) = in_list {
                        self.listed.remove(held);
                        if held < bookmark {
                            bookmark -= 1;
                        }
                        in_list = None;
                    }
                }
                let Some(held) = in_list else {
                    self.open.remove(node_at);
                    continue;
                };
                let name = self.name_of(node).to_string();
                let attributes = self.clone_attributes(node);
                let fresh = self.make(Namespace::Html, &name, attributes);
                self.listed[held] = Listed::Element(fresh);
                self.open[node_at] = fresh;
                if last_node == furthest_block {
                    bookmark = held + 1;
                }
                self.doc.detach(last_node);
                self.doc.append(fresh, last_node);
                last_node = fresh;
            }
            self.doc.detach(last_node);
            let saved = self.foster;
            self.foster = true;
            let at = self.place(Some(common));
            self.foster = saved;
            self.put(at, last_node);
            let name = self.name_of(element).to_string();
            let attributes = self.clone_attributes(element);
            let fresh = self.make(Namespace::Html, &name, attributes);
            let moved: Vec<NodeId> = std::mem::take(&mut self.doc.nodes[furthest_block].children);
            for child in moved {
                self.doc.nodes[child].parent = None;
                self.doc.append(fresh, child);
            }
            self.doc.append(furthest_block, fresh);
            if let Some(held) = self
                .listed
                .iter()
                .position(|entry| *entry == Listed::Element(element))
            {
                self.listed.remove(held);
                if held < bookmark {
                    bookmark -= 1;
                }
            }
            let bookmark = bookmark.min(self.listed.len());
            self.listed.insert(bookmark, Listed::Element(fresh));
            self.remove_from_stack(element);
            let below = self
                .open
                .iter()
                .position(|id| *id == furthest_block)
                .map_or(self.open.len(), |held| held + 1);
            self.open.insert(below, fresh);
        }
        true
    }
}

fn is_space_char(token: &Token) -> bool {
    matches!(token, Token::Char(character) if is_ascii_space(*character))
}

impl TreeBuilder {
    fn dispatch(&mut self, token: Token) {
        let was_self_closing = matches!(
            &token,
            Token::Start {
                self_closing: true,
                ..
            }
        );
        self.acknowledged = false;
        self.dispatch_again(token);
        if was_self_closing && !self.acknowledged {
            self.fault("non-void-html-element-start-tag-with-trailing-solidus");
        }
    }

    fn html_rules(&self, token: &Token) -> bool {
        match self.adjusted_current() {
            None => true,
            Some(node) => {
                self.namespace_of(node) == Namespace::Html || mode_is_html(self, node, token)
            }
        }
    }

    fn process(&mut self, mode: Mode, token: &Token) {
        match mode {
            Mode::Initial => self.initial(token),
            Mode::BeforeHtml => self.before_html(token),
            Mode::BeforeHead => self.before_head(token),
            Mode::InHead => self.in_head(token),
            Mode::InHeadNoscript => self.in_head_noscript(token),
            Mode::AfterHead => self.after_head(token),
            Mode::InBody => self.in_body(token),
            Mode::Text => self.text(token),
            Mode::InTable => self.in_table(token),
            Mode::InTableText => self.in_table_text(token),
            Mode::InCaption => self.in_caption(token),
            Mode::InColumnGroup => self.in_column_group(token),
            Mode::InTableBody => self.in_table_body(token),
            Mode::InRow => self.in_row(token),
            Mode::InCell => self.in_cell(token),
            Mode::InSelect => self.in_select(token),
            Mode::InSelectInTable => self.in_select_in_table(token),
            Mode::InTemplate => self.in_template(token),
            Mode::AfterBody => self.after_body(token),
            Mode::InFrameset => self.in_frameset(token),
            Mode::AfterFrameset => self.after_frameset(token),
            Mode::AfterAfterBody => self.after_after_body(token),
            Mode::AfterAfterFrameset => self.after_after_frameset(token),
        }
    }

    fn reprocess(&mut self, token: &Token) {
        self.dispatch_again(token.clone());
    }

    fn dispatch_again(&mut self, token: Token) {
        if self.html_rules(&token) {
            self.process(self.mode, &token);
        } else {
            self.foreign(&token);
        }
    }

    fn initial(&mut self, token: &Token) {
        match token {
            Token::Char(_) if is_space_char(token) => {}
            Token::Comment(data) => self.append_comment(0, data),
            Token::Doctype {
                name,
                public_id,
                system_id,
                force_quirks,
            } => {
                if name.as_deref() != Some("html")
                    || public_id.is_some()
                    || system_id
                        .as_deref()
                        .is_some_and(|held| held != "about:legacy-compat")
                {
                    self.fault("unexpected-doctype");
                }
                let id = self.doc.create(NodeKind::Doctype {
                    name: name.clone().unwrap_or_default(),
                    public_id: public_id.clone().unwrap_or_default(),
                    system_id: system_id.clone().unwrap_or_default(),
                });
                self.doc.append(0, id);
                self.doc.quirks = quirks_for(name, public_id, system_id, *force_quirks);
                self.mode = Mode::BeforeHtml;
            }
            _ => {
                self.fault("missing-doctype");
                self.doc.quirks = Quirks::Full;
                self.mode = Mode::BeforeHtml;
                self.reprocess(token);
            }
        }
    }

    fn before_html(&mut self, token: &Token) {
        match token {
            Token::Doctype { .. } => self.fault("unexpected-doctype"),
            Token::Comment(data) => self.append_comment(0, data),
            Token::Char(_) if is_space_char(token) => {}
            Token::Start {
                name, attributes, ..
            } if name == "html" => {
                let id = self.make(Namespace::Html, "html", plain(attributes));
                self.doc.append(0, id);
                self.open.push(id);
                self.mode = Mode::BeforeHead;
            }
            Token::End { name } if !matches!(name.as_str(), "head" | "body" | "html" | "br") => {
                self.fault("unexpected-end-tag");
            }
            _ => {
                let id = self.make(Namespace::Html, "html", Vec::new());
                self.doc.append(0, id);
                self.open.push(id);
                self.mode = Mode::BeforeHead;
                self.reprocess(token);
            }
        }
    }

    fn before_head(&mut self, token: &Token) {
        match token {
            Token::Char(_) if is_space_char(token) => {}
            Token::Comment(data) => self.insert_comment(data),
            Token::Doctype { .. } => self.fault("unexpected-doctype"),
            Token::Start { name, .. } if name == "html" => self.in_body(token),
            Token::Start {
                name, attributes, ..
            } if name == "head" => {
                let id = self.insert_html("head", attributes);
                self.head = Some(id);
                self.mode = Mode::InHead;
            }
            Token::End { name } if !matches!(name.as_str(), "head" | "body" | "html" | "br") => {
                self.fault("unexpected-end-tag");
            }
            _ => {
                let id = self.insert_bare("head");
                self.head = Some(id);
                self.mode = Mode::InHead;
                self.reprocess(token);
            }
        }
    }

    fn in_head(&mut self, token: &Token) {
        match token {
            Token::Char(character) if is_ascii_space(*character) => self.insert_char(*character),
            Token::Comment(data) => self.insert_comment(data),
            Token::Doctype { .. } => self.fault("unexpected-doctype"),
            Token::Start {
                name, attributes, ..
            } => match name.as_str() {
                "html" => self.in_body(token),
                "base" | "basefont" | "bgsound" | "link" | "meta" => {
                    self.insert_leaf(name, attributes)
                }
                "title" => self.generic_text(name, attributes, State::Rcdata),
                "noscript" if self.scripting => self.generic_text(name, attributes, State::Rawtext),
                "noscript" => {
                    self.insert_html(name, attributes);
                    self.mode = Mode::InHeadNoscript;
                }
                "noframes" | "style" => self.generic_text(name, attributes, State::Rawtext),
                "script" => self.generic_text(name, attributes, State::ScriptData),
                "template" => {
                    self.insert_html(name, attributes);
                    self.listed.push(Listed::Marker);
                    self.frameset_ok = false;
                    self.mode = Mode::InTemplate;
                    self.templates.push(Mode::InTemplate);
                }
                "head" => self.fault("unexpected-start-tag"),
                _ => self.in_head_other(token),
            },
            Token::End { name } => match name.as_str() {
                "head" => {
                    self.open.pop();
                    self.mode = Mode::AfterHead;
                }
                "body" | "html" | "br" => self.in_head_other(token),
                "template" => self.end_template(),
                _ => self.fault("unexpected-end-tag"),
            },
            _ => self.in_head_other(token),
        }
    }

    fn in_head_other(&mut self, token: &Token) {
        self.open.pop();
        self.mode = Mode::AfterHead;
        self.reprocess(token);
    }

    fn end_template(&mut self) {
        if !self.has_template() {
            self.fault("unexpected-end-tag");
            return;
        }
        self.generate_all_implied_end_tags();
        if !self.current_is("template") {
            self.fault("unexpected-open-element");
        }
        self.pop_until("template");
        self.clear_to_marker();
        self.templates.pop();
        self.reset_mode();
    }

    fn in_head_noscript(&mut self, token: &Token) {
        match token {
            Token::Doctype { .. } => self.fault("unexpected-doctype"),
            Token::Start { name, .. } if name == "html" => self.in_body(token),
            Token::End { name } if name == "noscript" => {
                self.open.pop();
                self.mode = Mode::InHead;
            }
            Token::Char(_) if is_space_char(token) => self.in_head(token),
            Token::Comment(_) => self.in_head(token),
            Token::Start { name, .. }
                if matches!(
                    name.as_str(),
                    "basefont" | "bgsound" | "link" | "meta" | "noframes" | "style"
                ) =>
            {
                self.in_head(token)
            }
            Token::Start { name, .. } if matches!(name.as_str(), "head" | "noscript") => {
                self.fault("unexpected-start-tag")
            }
            Token::End { name } if name != "br" => self.fault("unexpected-end-tag"),
            _ => {
                self.fault("unexpected-token");
                self.open.pop();
                self.mode = Mode::InHead;
                self.reprocess(token);
            }
        }
    }

    fn after_head(&mut self, token: &Token) {
        match token {
            Token::Char(character) if is_ascii_space(*character) => self.insert_char(*character),
            Token::Comment(data) => self.insert_comment(data),
            Token::Doctype { .. } => self.fault("unexpected-doctype"),
            Token::Start {
                name, attributes, ..
            } => match name.as_str() {
                "html" => self.in_body(token),
                "body" => {
                    self.insert_html(name, attributes);
                    self.frameset_ok = false;
                    self.mode = Mode::InBody;
                }
                "frameset" => {
                    self.insert_html(name, attributes);
                    self.mode = Mode::InFrameset;
                }
                "base" | "basefont" | "bgsound" | "link" | "meta" | "noframes" | "script"
                | "style" | "template" | "title" => {
                    self.fault("unexpected-start-tag");
                    if let Some(head) = self.head {
                        self.open.push(head);
                        self.in_head(token);
                        self.remove_from_stack(head);
                    }
                }
                "head" => self.fault("unexpected-start-tag"),
                _ => self.after_head_other(token),
            },
            Token::End { name } => match name.as_str() {
                "template" => self.in_head(token),
                "body" | "html" | "br" => self.after_head_other(token),
                _ => self.fault("unexpected-end-tag"),
            },
            _ => self.after_head_other(token),
        }
    }

    fn after_head_other(&mut self, token: &Token) {
        self.insert_bare("body");
        self.mode = Mode::InBody;
        self.reprocess(token);
    }

    fn text(&mut self, token: &Token) {
        match token {
            Token::Char(character) => self.insert_char(*character),
            Token::Eof => {
                self.fault("eof-in-text");
                self.open.pop();
                self.mode = self.original;
                self.reprocess(token);
            }
            Token::End { .. } => {
                self.open.pop();
                self.mode = self.original;
            }
            _ => {}
        }
    }
}

fn mode_is_html(builder: &TreeBuilder, node: NodeId, token: &Token) -> bool {
    let namespace = builder.namespace_of(node);
    let start_name = match token {
        Token::Start { name, .. } => Some(name.as_str()),
        _ => None,
    };
    let is_char = matches!(token, Token::Char(_));
    if *token == Token::Eof {
        return true;
    }
    if builder.math_text_point(node)
        && ((start_name.is_some() && !matches!(start_name, Some("mglyph" | "malignmark")))
            || is_char)
    {
        return true;
    }
    if namespace == Namespace::MathMl
        && builder.name_of(node) == "annotation-xml"
        && start_name == Some("svg")
    {
        return true;
    }
    builder.html_point(node) && (start_name.is_some() || is_char)
}

const HEADINGS: &[&str] = &["h1", "h2", "h3", "h4", "h5", "h6"];

const BLOCK_START: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "center",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "header",
    "hgroup",
    "main",
    "menu",
    "nav",
    "ol",
    "p",
    "search",
    "section",
    "summary",
    "ul",
];

const BLOCK_END: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "button",
    "center",
    "details",
    "dialog",
    "dir",
    "div",
    "dl",
    "fieldset",
    "figcaption",
    "figure",
    "footer",
    "header",
    "hgroup",
    "listing",
    "main",
    "menu",
    "nav",
    "ol",
    "pre",
    "search",
    "section",
    "summary",
    "ul",
];

impl TreeBuilder {
    fn merge_attributes(&mut self, target: NodeId, attributes: &[(String, String)]) {
        if let NodeKind::Element {
            attributes: held, ..
        } = &mut self.doc.nodes[target].kind
        {
            for (name, value) in attributes {
                if !held
                    .iter()
                    .any(|attribute| attribute.prefix.is_none() && attribute.name == *name)
                {
                    held.push(Attribute {
                        prefix: None,
                        name: name.clone(),
                        value: value.clone(),
                    });
                }
            }
        }
    }

    fn in_body(&mut self, token: &Token) {
        match token {
            Token::Char('\0') => self.fault("unexpected-null-character"),
            Token::Char(character) if is_ascii_space(*character) => {
                self.reconstruct();
                self.insert_char(*character);
            }
            Token::Char(character) => {
                self.reconstruct();
                self.insert_char(*character);
                self.frameset_ok = false;
            }
            Token::Comment(data) => self.insert_comment(data),
            Token::Doctype { .. } => self.fault("unexpected-doctype"),
            Token::Start {
                name,
                attributes,
                self_closing,
            } => self.body_start(token, name, attributes, *self_closing),
            Token::End { name } => self.body_end(token, name),
            Token::Eof => {
                if self.templates.is_empty() {
                    self.stopped = true;
                } else {
                    self.in_template(token);
                }
            }
        }
    }

    fn body_start(
        &mut self,
        token: &Token,
        name: &str,
        attributes: &[(String, String)],
        self_closing: bool,
    ) {
        match name {
            "html" => {
                self.fault("unexpected-start-tag");
                if !self.has_template() {
                    if let Some(&root) = self.open.first() {
                        self.merge_attributes(root, attributes);
                    }
                }
            }
            "base" | "basefont" | "bgsound" | "link" | "meta" | "noframes" | "script" | "style"
            | "template" | "title" => self.in_head(token),
            "body" => {
                self.fault("unexpected-start-tag");
                if self.open.len() >= 2 && self.is(self.open[1], "body") && !self.has_template() {
                    self.frameset_ok = false;
                    let body = self.open[1];
                    self.merge_attributes(body, attributes);
                }
            }
            "frameset" => {
                self.fault("unexpected-start-tag");
                if self.open.len() >= 2 && self.is(self.open[1], "body") && self.frameset_ok {
                    let body = self.open[1];
                    self.doc.detach(body);
                    self.open.truncate(1);
                    self.insert_html(name, attributes);
                    self.mode = Mode::InFrameset;
                }
            }
            _ if BLOCK_START.contains(&name) => {
                self.close_p_in_button_scope();
                self.insert_html(name, attributes);
            }
            _ if HEADINGS.contains(&name) => {
                self.close_p_in_button_scope();
                if self.current_is_one_of(HEADINGS) {
                    self.fault("unexpected-heading");
                    self.open.pop();
                }
                self.insert_html(name, attributes);
            }
            "pre" | "listing" => {
                self.close_p_in_button_scope();
                self.insert_html(name, attributes);
                self.skip_lf = true;
                self.frameset_ok = false;
            }
            "form" => {
                if self.form.is_some() && !self.has_template() {
                    self.fault("nested-form");
                } else {
                    self.close_p_in_button_scope();
                    let id = self.insert_html(name, attributes);
                    if !self.has_template() {
                        self.form = Some(id);
                    }
                }
            }
            "li" => {
                self.frameset_ok = false;
                let mut at = self.open.len();
                while at > 0 {
                    at -= 1;
                    let node = self.open[at];
                    if self.is(node, "li") {
                        self.generate_implied_end_tags(Some("li"));
                        if !self.current_is("li") {
                            self.fault("unexpected-open-element");
                        }
                        self.pop_until("li");
                        break;
                    }
                    if self.is_special(node) && !self.is_one_of(node, &["address", "div", "p"]) {
                        break;
                    }
                }
                self.close_p_in_button_scope();
                self.insert_html(name, attributes);
            }
            "dd" | "dt" => {
                self.frameset_ok = false;
                let mut at = self.open.len();
                while at > 0 {
                    at -= 1;
                    let node = self.open[at];
                    if self.is_one_of(node, &["dd", "dt"]) {
                        let held = self.name_of(node).to_string();
                        self.generate_implied_end_tags(Some(&held));
                        if !self.is(self.open[self.open.len() - 1], &held) {
                            self.fault("unexpected-open-element");
                        }
                        self.pop_until(&held);
                        break;
                    }
                    if self.is_special(node) && !self.is_one_of(node, &["address", "div", "p"]) {
                        break;
                    }
                }
                self.close_p_in_button_scope();
                self.insert_html(name, attributes);
            }
            "plaintext" => {
                self.close_p_in_button_scope();
                self.insert_html(name, attributes);
                self.tok.state = State::Plaintext;
            }
            "button" => {
                if self.in_scope("button", Scope::Default) {
                    self.fault("nested-button");
                    self.generate_implied_end_tags(None);
                    self.pop_until("button");
                }
                self.reconstruct();
                self.insert_html(name, attributes);
                self.frameset_ok = false;
            }
            "a" => {
                let mut found = None;
                for entry in self.listed.iter().rev() {
                    match entry {
                        Listed::Marker => break,
                        Listed::Element(id) => {
                            if self.is(*id, "a") {
                                found = Some(*id);
                                break;
                            }
                        }
                    }
                }
                if let Some(held) = found {
                    self.fault("nested-anchor");
                    self.run_adoption_agency("a");
                    self.listed.retain(|entry| *entry != Listed::Element(held));
                    self.remove_from_stack(held);
                }
                self.reconstruct();
                let id = self.insert_html(name, attributes);
                self.push_listed(id);
            }
            "b" | "big" | "code" | "em" | "font" | "i" | "s" | "small" | "strike" | "strong"
            | "tt" | "u" => {
                self.reconstruct();
                let id = self.insert_html(name, attributes);
                self.push_listed(id);
            }
            "nobr" => {
                self.reconstruct();
                if self.in_scope("nobr", Scope::Default) {
                    self.fault("nested-nobr");
                    self.run_adoption_agency("nobr");
                    self.reconstruct();
                }
                let id = self.insert_html(name, attributes);
                self.push_listed(id);
            }
            "applet" | "marquee" | "object" => {
                self.reconstruct();
                self.insert_html(name, attributes);
                self.listed.push(Listed::Marker);
                self.frameset_ok = false;
            }
            "table" => {
                if self.doc.quirks != Quirks::Full {
                    self.close_p_in_button_scope();
                }
                self.insert_html(name, attributes);
                self.frameset_ok = false;
                self.mode = Mode::InTable;
            }
            "area" | "br" | "embed" | "img" | "keygen" | "wbr" => {
                self.reconstruct();
                self.insert_leaf(name, attributes);
                self.frameset_ok = false;
            }
            "input" => {
                self.reconstruct();
                self.insert_leaf(name, attributes);
                let hidden = attributes
                    .iter()
                    .find(|(held, _)| held == "type")
                    .is_some_and(|(_, value)| value.eq_ignore_ascii_case("hidden"));
                if !hidden {
                    self.frameset_ok = false;
                }
            }
            "param" | "source" | "track" => self.insert_leaf(name, attributes),
            "hr" => {
                self.close_p_in_button_scope();
                self.insert_leaf(name, attributes);
                self.frameset_ok = false;
            }
            "image" => {
                self.fault("image-start-tag");
                let renamed = Token::Start {
                    name: "img".to_string(),
                    attributes: attributes.to_vec(),
                    self_closing,
                };
                self.in_body(&renamed);
            }
            "textarea" => {
                self.insert_html(name, attributes);
                self.skip_lf = true;
                self.tok.state = State::Rcdata;
                self.original = self.mode;
                self.frameset_ok = false;
                self.mode = Mode::Text;
            }
            "xmp" => {
                self.close_p_in_button_scope();
                self.reconstruct();
                self.frameset_ok = false;
                self.generic_text(name, attributes, State::Rawtext);
            }
            "iframe" => {
                self.frameset_ok = false;
                self.generic_text(name, attributes, State::Rawtext);
            }
            "noembed" => self.generic_text(name, attributes, State::Rawtext),
            "noscript" if self.scripting => self.generic_text(name, attributes, State::Rawtext),
            "select" => {
                self.reconstruct();
                self.insert_html(name, attributes);
                self.frameset_ok = false;
                self.mode = if matches!(
                    self.mode,
                    Mode::InTable
                        | Mode::InCaption
                        | Mode::InTableBody
                        | Mode::InRow
                        | Mode::InCell
                ) {
                    Mode::InSelectInTable
                } else {
                    Mode::InSelect
                };
            }
            "optgroup" | "option" => {
                if self.current_is("option") {
                    self.open.pop();
                }
                self.reconstruct();
                self.insert_html(name, attributes);
            }
            "rb" | "rtc" => {
                if self.in_scope("ruby", Scope::Default) {
                    self.generate_implied_end_tags(None);
                    if !self.current_is("ruby") {
                        self.fault("unexpected-open-element");
                    }
                }
                self.insert_html(name, attributes);
            }
            "rp" | "rt" => {
                if self.in_scope("ruby", Scope::Default) {
                    self.generate_implied_end_tags(Some("rtc"));
                    if !self.current_is("rtc") && !self.current_is("ruby") {
                        self.fault("unexpected-open-element");
                    }
                }
                self.insert_html(name, attributes);
            }
            "math" => {
                self.reconstruct();
                let mut adjusted = plain(attributes);
                adjust_mathml(&mut adjusted);
                adjust_foreign(&mut adjusted);
                self.insert_element(Namespace::MathMl, name, adjusted);
                if self_closing {
                    self.open.pop();
                    self.acknowledged = true;
                }
            }
            "svg" => {
                self.reconstruct();
                let mut adjusted = plain(attributes);
                adjust_svg_attributes(&mut adjusted);
                adjust_foreign(&mut adjusted);
                self.insert_element(Namespace::Svg, name, adjusted);
                if self_closing {
                    self.open.pop();
                    self.acknowledged = true;
                }
            }
            "caption" | "col" | "colgroup" | "frame" | "head" | "tbody" | "td" | "tfoot" | "th"
            | "thead" | "tr" => self.fault("unexpected-start-tag"),
            _ => {
                self.reconstruct();
                self.insert_html(name, attributes);
            }
        }
    }

    fn body_end(&mut self, token: &Token, name: &str) {
        match name {
            "template" => self.in_head(token),
            "body" => {
                if self.in_scope("body", Scope::Default) {
                    self.mode = Mode::AfterBody;
                } else {
                    self.fault("unexpected-end-tag");
                }
            }
            "html" => {
                if self.in_scope("body", Scope::Default) {
                    self.mode = Mode::AfterBody;
                    self.reprocess(token);
                } else {
                    self.fault("unexpected-end-tag");
                }
            }
            _ if BLOCK_END.contains(&name) => {
                if self.in_scope(name, Scope::Default) {
                    self.generate_implied_end_tags(None);
                    if !self.current_is(name) {
                        self.fault("unexpected-open-element");
                    }
                    self.pop_until(name);
                } else {
                    self.fault("unexpected-end-tag");
                }
            }
            "form" => {
                if !self.has_template() {
                    let node = self.form.take();
                    match node {
                        Some(held) if self.node_in_scope(held, Scope::Default) => {
                            self.generate_implied_end_tags(None);
                            if self.current() != Some(held) {
                                self.fault("unexpected-open-element");
                            }
                            self.remove_from_stack(held);
                        }
                        _ => self.fault("unexpected-end-tag"),
                    }
                } else if self.in_scope("form", Scope::Default) {
                    self.generate_implied_end_tags(None);
                    if !self.current_is("form") {
                        self.fault("unexpected-open-element");
                    }
                    self.pop_until("form");
                } else {
                    self.fault("unexpected-end-tag");
                }
            }
            "p" => {
                if !self.in_scope("p", Scope::Button) {
                    self.fault("unexpected-end-tag");
                    self.insert_bare("p");
                }
                self.close_p();
            }
            "li" => {
                if self.in_scope("li", Scope::ListItem) {
                    self.generate_implied_end_tags(Some("li"));
                    if !self.current_is("li") {
                        self.fault("unexpected-open-element");
                    }
                    self.pop_until("li");
                } else {
                    self.fault("unexpected-end-tag");
                }
            }
            "dd" | "dt" => {
                if self.in_scope(name, Scope::Default) {
                    self.generate_implied_end_tags(Some(name));
                    if !self.current_is(name) {
                        self.fault("unexpected-open-element");
                    }
                    self.pop_until(name);
                } else {
                    self.fault("unexpected-end-tag");
                }
            }
            _ if HEADINGS.contains(&name) => {
                if self.in_scope_one_of(HEADINGS, Scope::Default) {
                    self.generate_implied_end_tags(None);
                    if !self.current_is(name) {
                        self.fault("unexpected-open-element");
                    }
                    self.pop_until_one_of(HEADINGS);
                } else {
                    self.fault("unexpected-end-tag");
                }
            }
            _ if FORMATTING.contains(&name) => {
                if !self.run_adoption_agency(name) {
                    self.any_other_end(name);
                }
            }
            "applet" | "marquee" | "object" => {
                if self.in_scope(name, Scope::Default) {
                    self.generate_implied_end_tags(None);
                    if !self.current_is(name) {
                        self.fault("unexpected-open-element");
                    }
                    self.pop_until(name);
                    self.clear_to_marker();
                } else {
                    self.fault("unexpected-end-tag");
                }
            }
            "br" => {
                self.fault("unexpected-end-tag-br");
                let start = Token::Start {
                    name: "br".to_string(),
                    attributes: Vec::new(),
                    self_closing: false,
                };
                self.in_body(&start);
            }
            _ => self.any_other_end(name),
        }
    }

    fn any_other_end(&mut self, name: &str) {
        let mut at = self.open.len();
        while at > 0 {
            at -= 1;
            let node = self.open[at];
            if self.is(node, name) {
                self.generate_implied_end_tags(Some(name));
                if self.current() != Some(node) {
                    self.fault("unexpected-open-element");
                }
                self.pop_through(node);
                break;
            }
            if self.is_special(node) {
                self.fault("unexpected-end-tag");
                break;
            }
        }
    }

    fn clear_to(&mut self, names: &[&str]) {
        while let Some(id) = self.current() {
            if self.is_one_of(id, names) {
                break;
            }
            self.open.pop();
        }
    }

    fn table_anything_else(&mut self, token: &Token) {
        self.fault("foster-parenting");
        let saved = self.foster;
        self.foster = true;
        self.in_body(token);
        self.foster = saved;
    }

    fn in_table(&mut self, token: &Token) {
        match token {
            Token::Char(_)
                if self
                    .current_is_one_of(&["table", "tbody", "template", "tfoot", "thead", "tr"]) =>
            {
                self.table_text.clear();
                self.original = self.mode;
                self.mode = Mode::InTableText;
                self.reprocess(token);
            }
            Token::Comment(data) => self.insert_comment(data),
            Token::Doctype { .. } => self.fault("unexpected-doctype"),
            Token::Start {
                name, attributes, ..
            } => match name.as_str() {
                "caption" => {
                    self.clear_to(&["table", "template", "html"]);
                    self.listed.push(Listed::Marker);
                    self.insert_html(name, attributes);
                    self.mode = Mode::InCaption;
                }
                "colgroup" => {
                    self.clear_to(&["table", "template", "html"]);
                    self.insert_html(name, attributes);
                    self.mode = Mode::InColumnGroup;
                }
                "col" => {
                    self.clear_to(&["table", "template", "html"]);
                    self.insert_bare("colgroup");
                    self.mode = Mode::InColumnGroup;
                    self.reprocess(token);
                }
                "tbody" | "tfoot" | "thead" => {
                    self.clear_to(&["table", "template", "html"]);
                    self.insert_html(name, attributes);
                    self.mode = Mode::InTableBody;
                }
                "td" | "th" | "tr" => {
                    self.clear_to(&["table", "template", "html"]);
                    self.insert_bare("tbody");
                    self.mode = Mode::InTableBody;
                    self.reprocess(token);
                }
                "table" => {
                    self.fault("unexpected-start-tag");
                    if self.in_scope("table", Scope::Table) {
                        self.pop_until("table");
                        self.reset_mode();
                        self.reprocess(token);
                    }
                }
                "style" | "script" | "template" => self.in_head(token),
                "input" => {
                    let hidden = attributes
                        .iter()
                        .find(|(held, _)| held == "type")
                        .is_some_and(|(_, value)| value.eq_ignore_ascii_case("hidden"));
                    if hidden {
                        self.fault("unexpected-hidden-input");
                        self.insert_leaf(name, attributes);
                    } else {
                        self.table_anything_else(token);
                    }
                }
                "form" => {
                    self.fault("unexpected-form");
                    if !self.has_template() && self.form.is_none() {
                        let id = self.insert_html(name, attributes);
                        self.form = Some(id);
                        self.open.pop();
                    }
                }
                _ => self.table_anything_else(token),
            },
            Token::End { name } => match name.as_str() {
                "table" => {
                    if self.in_scope("table", Scope::Table) {
                        self.pop_until("table");
                        self.reset_mode();
                    } else {
                        self.fault("unexpected-end-tag");
                    }
                }
                "body" | "caption" | "col" | "colgroup" | "html" | "tbody" | "td" | "tfoot"
                | "th" | "thead" | "tr" => self.fault("unexpected-end-tag"),
                "template" => self.in_head(token),
                _ => self.table_anything_else(token),
            },
            Token::Eof => self.in_body(token),
            _ => self.table_anything_else(token),
        }
    }

    fn in_table_text(&mut self, token: &Token) {
        match token {
            Token::Char('\0') => self.fault("unexpected-null-character"),
            Token::Char(character) => self.table_text.push(*character),
            _ => {
                let held = std::mem::take(&mut self.table_text);
                if held.iter().any(|character| !is_ascii_space(*character)) {
                    for character in held {
                        self.table_anything_else(&Token::Char(character));
                    }
                } else {
                    for character in held {
                        self.insert_char(character);
                    }
                }
                self.mode = self.original;
                self.reprocess(token);
            }
        }
    }

    fn close_caption(&mut self) -> bool {
        if !self.in_scope("caption", Scope::Table) {
            self.fault("unexpected-end-tag");
            return false;
        }
        self.generate_implied_end_tags(None);
        if !self.current_is("caption") {
            self.fault("unexpected-open-element");
        }
        self.pop_until("caption");
        self.clear_to_marker();
        self.mode = Mode::InTable;
        true
    }

    fn in_caption(&mut self, token: &Token) {
        match token {
            Token::End { name } if name == "caption" => {
                self.close_caption();
            }
            Token::Start { name, .. }
                if matches!(
                    name.as_str(),
                    "caption"
                        | "col"
                        | "colgroup"
                        | "tbody"
                        | "td"
                        | "tfoot"
                        | "th"
                        | "thead"
                        | "tr"
                ) =>
            {
                if self.close_caption() {
                    self.reprocess(token);
                }
            }
            Token::End { name } if name == "table" => {
                if self.close_caption() {
                    self.reprocess(token);
                }
            }
            Token::End { name }
                if matches!(
                    name.as_str(),
                    "body"
                        | "col"
                        | "colgroup"
                        | "html"
                        | "tbody"
                        | "td"
                        | "tfoot"
                        | "th"
                        | "thead"
                        | "tr"
                ) =>
            {
                self.fault("unexpected-end-tag")
            }
            _ => self.in_body(token),
        }
    }

    fn in_column_group(&mut self, token: &Token) {
        match token {
            Token::Char(character) if is_ascii_space(*character) => self.insert_char(*character),
            Token::Comment(data) => self.insert_comment(data),
            Token::Doctype { .. } => self.fault("unexpected-doctype"),
            Token::Start { name, .. } if name == "html" => self.in_body(token),
            Token::Start {
                name, attributes, ..
            } if name == "col" => self.insert_leaf(name, attributes),
            Token::End { name } if name == "colgroup" => {
                if self.current_is("colgroup") {
                    self.open.pop();
                    self.mode = Mode::InTable;
                } else {
                    self.fault("unexpected-end-tag");
                }
            }
            Token::End { name } if name == "col" => self.fault("unexpected-end-tag"),
            Token::Start { name, .. } | Token::End { name } if name == "template" => {
                self.in_head(token)
            }
            Token::Eof => self.in_body(token),
            _ => {
                if self.current_is("colgroup") {
                    self.open.pop();
                    self.mode = Mode::InTable;
                    self.reprocess(token);
                } else {
                    self.fault("unexpected-token");
                }
            }
        }
    }

    fn in_table_body(&mut self, token: &Token) {
        match token {
            Token::Start {
                name, attributes, ..
            } if name == "tr" => {
                self.clear_to(&["tbody", "tfoot", "thead", "template", "html"]);
                self.insert_html(name, attributes);
                self.mode = Mode::InRow;
            }
            Token::Start { name, .. } if matches!(name.as_str(), "th" | "td") => {
                self.fault("unexpected-cell");
                self.clear_to(&["tbody", "tfoot", "thead", "template", "html"]);
                self.insert_bare("tr");
                self.mode = Mode::InRow;
                self.reprocess(token);
            }
            Token::End { name } if matches!(name.as_str(), "tbody" | "tfoot" | "thead") => {
                if self.in_scope(name, Scope::Table) {
                    self.clear_to(&["tbody", "tfoot", "thead", "template", "html"]);
                    self.open.pop();
                    self.mode = Mode::InTable;
                } else {
                    self.fault("unexpected-end-tag");
                }
            }
            Token::Start { name, .. }
                if matches!(
                    name.as_str(),
                    "caption" | "col" | "colgroup" | "tbody" | "tfoot" | "thead"
                ) =>
            {
                self.leave_table_body(token)
            }
            Token::End { name } if name == "table" => self.leave_table_body(token),
            Token::End { name }
                if matches!(
                    name.as_str(),
                    "body" | "caption" | "col" | "colgroup" | "html" | "td" | "th" | "tr"
                ) =>
            {
                self.fault("unexpected-end-tag")
            }
            _ => self.in_table(token),
        }
    }

    fn leave_table_body(&mut self, token: &Token) {
        if self.in_scope_one_of(&["tbody", "thead", "tfoot"], Scope::Table) {
            self.clear_to(&["tbody", "tfoot", "thead", "template", "html"]);
            self.open.pop();
            self.mode = Mode::InTable;
            self.reprocess(token);
        } else {
            self.fault("unexpected-token");
        }
    }

    fn in_row(&mut self, token: &Token) {
        match token {
            Token::Start {
                name, attributes, ..
            } if matches!(name.as_str(), "th" | "td") => {
                self.clear_to(&["tr", "template", "html"]);
                self.insert_html(name, attributes);
                self.mode = Mode::InCell;
                self.listed.push(Listed::Marker);
            }
            Token::End { name } if name == "tr" => {
                if self.in_scope("tr", Scope::Table) {
                    self.clear_to(&["tr", "template", "html"]);
                    self.open.pop();
                    self.mode = Mode::InTableBody;
                } else {
                    self.fault("unexpected-end-tag");
                }
            }
            Token::Start { name, .. }
                if matches!(
                    name.as_str(),
                    "caption" | "col" | "colgroup" | "tbody" | "tfoot" | "thead" | "tr"
                ) =>
            {
                self.leave_row(token)
            }
            Token::End { name } if name == "table" => self.leave_row(token),
            Token::End { name } if matches!(name.as_str(), "tbody" | "tfoot" | "thead") => {
                if self.in_scope(name, Scope::Table) {
                    self.leave_row(token);
                } else {
                    self.fault("unexpected-end-tag");
                }
            }
            Token::End { name }
                if matches!(
                    name.as_str(),
                    "body" | "caption" | "col" | "colgroup" | "html" | "td" | "th"
                ) =>
            {
                self.fault("unexpected-end-tag")
            }
            _ => self.in_table(token),
        }
    }

    fn leave_row(&mut self, token: &Token) {
        if self.in_scope("tr", Scope::Table) {
            self.clear_to(&["tr", "template", "html"]);
            self.open.pop();
            self.mode = Mode::InTableBody;
            self.reprocess(token);
        } else {
            self.fault("unexpected-token");
        }
    }

    fn close_cell(&mut self) {
        self.generate_implied_end_tags(None);
        if !self.current_is_one_of(&["td", "th"]) {
            self.fault("unexpected-open-element");
        }
        self.pop_until_one_of(&["td", "th"]);
        self.clear_to_marker();
        self.mode = Mode::InRow;
    }

    fn in_cell(&mut self, token: &Token) {
        match token {
            Token::End { name } if matches!(name.as_str(), "td" | "th") => {
                if self.in_scope(name, Scope::Table) {
                    self.generate_implied_end_tags(None);
                    if !self.current_is(name) {
                        self.fault("unexpected-open-element");
                    }
                    self.pop_until(name);
                    self.clear_to_marker();
                    self.mode = Mode::InRow;
                } else {
                    self.fault("unexpected-end-tag");
                }
            }
            Token::Start { name, .. }
                if matches!(
                    name.as_str(),
                    "caption"
                        | "col"
                        | "colgroup"
                        | "tbody"
                        | "td"
                        | "tfoot"
                        | "th"
                        | "thead"
                        | "tr"
                ) =>
            {
                if self.in_scope_one_of(&["td", "th"], Scope::Table) {
                    self.close_cell();
                    self.reprocess(token);
                } else {
                    self.fault("unexpected-start-tag");
                }
            }
            Token::End { name }
                if matches!(
                    name.as_str(),
                    "body" | "caption" | "col" | "colgroup" | "html"
                ) =>
            {
                self.fault("unexpected-end-tag")
            }
            Token::End { name }
                if matches!(name.as_str(), "table" | "tbody" | "tfoot" | "thead" | "tr") =>
            {
                if self.in_scope(name, Scope::Table) {
                    self.close_cell();
                    self.reprocess(token);
                } else {
                    self.fault("unexpected-end-tag");
                }
            }
            _ => self.in_body(token),
        }
    }

    fn in_select(&mut self, token: &Token) {
        match token {
            Token::Char('\0') => self.fault("unexpected-null-character"),
            Token::Char(character) => {
                self.reconstruct();
                self.insert_char(*character);
            }
            Token::Comment(data) => self.insert_comment(data),
            Token::Doctype { .. } => self.fault("unexpected-doctype"),
            Token::Start { name, .. } => match name.as_str() {
                "html" => self.in_body(token),
                "option" => {
                    if self.current_is("option") {
                        self.open.pop();
                    }
                    self.in_body(token);
                }
                "optgroup" | "hr" => {
                    if self.current_is("option") {
                        self.open.pop();
                    }
                    if self.current_is("optgroup") {
                        self.open.pop();
                    }
                    self.in_body(token);
                }
                "select" => {
                    self.fault("unexpected-start-tag");
                    if self.open.iter().any(|id| self.is(*id, "select")) {
                        self.pop_until("select");
                        self.reset_mode();
                    }
                }
                "input" | "textarea" => {
                    if self.open.iter().any(|id| self.is(*id, "select")) {
                        self.fault("unexpected-start-tag");
                        self.pop_until("select");
                        self.reset_mode();
                        self.reprocess(token);
                    } else {
                        self.in_body(token);
                    }
                }
                "script" | "template" => self.in_head(token),
                _ => self.in_body(token),
            },
            Token::End { name } => match name.as_str() {
                "optgroup" => {
                    if self.current_is("option")
                        && self.open.len() >= 2
                        && self.is(self.open[self.open.len() - 2], "optgroup")
                    {
                        self.open.pop();
                    }
                    if self.current_is("optgroup") {
                        self.open.pop();
                    } else {
                        self.fault("unexpected-end-tag");
                    }
                }
                "option" => {
                    if self.current_is("option") {
                        self.open.pop();
                    } else {
                        self.fault("unexpected-end-tag");
                    }
                }
                "select" => {
                    if self.in_scope("select", Scope::Select) {
                        self.pop_until("select");
                        self.reset_mode();
                    } else {
                        self.fault("unexpected-end-tag");
                    }
                }
                "template" => self.in_head(token),
                _ => self.in_body(token),
            },
            Token::Eof => self.in_body(token),
        }
    }

    fn in_select_in_table(&mut self, token: &Token) {
        match token {
            Token::Start { name, .. }
                if matches!(
                    name.as_str(),
                    "caption" | "table" | "tbody" | "tfoot" | "thead" | "tr" | "td" | "th"
                ) =>
            {
                self.fault("unexpected-start-tag");
                self.pop_until("select");
                self.reset_mode();
                self.reprocess(token);
            }
            Token::End { name }
                if matches!(
                    name.as_str(),
                    "caption" | "table" | "tbody" | "tfoot" | "thead" | "tr" | "td" | "th"
                ) =>
            {
                self.fault("unexpected-end-tag");
                if self.in_scope(name, Scope::Table) {
                    self.pop_until("select");
                    self.reset_mode();
                    self.reprocess(token);
                }
            }
            _ => self.in_select(token),
        }
    }

    fn switch_template(&mut self, mode: Mode, token: &Token) {
        self.templates.pop();
        self.templates.push(mode);
        self.mode = mode;
        self.reprocess(token);
    }

    fn in_template(&mut self, token: &Token) {
        match token {
            Token::Char(_) | Token::Comment(_) | Token::Doctype { .. } => self.in_body(token),
            Token::Start { name, .. } => match name.as_str() {
                "base" | "basefont" | "bgsound" | "link" | "meta" | "noframes" | "script"
                | "style" | "template" | "title" => self.in_head(token),
                "caption" | "colgroup" | "tbody" | "tfoot" | "thead" => {
                    self.switch_template(Mode::InTable, token)
                }
                "col" => self.switch_template(Mode::InColumnGroup, token),
                "tr" => self.switch_template(Mode::InTableBody, token),
                "td" | "th" => self.switch_template(Mode::InRow, token),
                _ => self.switch_template(Mode::InBody, token),
            },
            Token::End { name } => {
                if name == "template" {
                    self.in_head(token);
                } else {
                    self.fault("unexpected-end-tag");
                }
            }
            Token::Eof => {
                if !self.has_template() {
                    self.stopped = true;
                } else {
                    self.fault("eof-in-template");
                    self.pop_until("template");
                    self.clear_to_marker();
                    self.templates.pop();
                    self.reset_mode();
                    self.reprocess(token);
                }
            }
        }
    }

    fn after_body(&mut self, token: &Token) {
        match token {
            Token::Char(_) if is_space_char(token) => self.in_body(token),
            Token::Comment(data) => {
                if let Some(&root) = self.open.first() {
                    self.append_comment(root, data);
                }
            }
            Token::Doctype { .. } => self.fault("unexpected-doctype"),
            Token::Start { name, .. } if name == "html" => self.in_body(token),
            Token::End { name } if name == "html" => {
                if self.context.is_some() {
                    self.fault("unexpected-end-tag");
                } else {
                    self.mode = Mode::AfterAfterBody;
                }
            }
            Token::Eof => self.stopped = true,
            _ => {
                self.fault("unexpected-token");
                self.mode = Mode::InBody;
                self.reprocess(token);
            }
        }
    }

    fn in_frameset(&mut self, token: &Token) {
        match token {
            Token::Char(character) if is_ascii_space(*character) => self.insert_char(*character),
            Token::Comment(data) => self.insert_comment(data),
            Token::Doctype { .. } => self.fault("unexpected-doctype"),
            Token::Start { name, .. } if name == "html" => self.in_body(token),
            Token::Start {
                name, attributes, ..
            } if name == "frameset" => {
                self.insert_html(name, attributes);
            }
            Token::End { name } if name == "frameset" => {
                if self.open.len() == 1 {
                    self.fault("unexpected-end-tag");
                } else {
                    self.open.pop();
                    if self.context.is_none() && !self.current_is("frameset") {
                        self.mode = Mode::AfterFrameset;
                    }
                }
            }
            Token::Start {
                name, attributes, ..
            } if name == "frame" => self.insert_leaf(name, attributes),
            Token::Start { name, .. } if name == "noframes" => self.in_head(token),
            Token::Eof => {
                if self.open.len() != 1 {
                    self.fault("eof-in-frameset");
                }
                self.stopped = true;
            }
            _ => self.fault("unexpected-token"),
        }
    }

    fn after_frameset(&mut self, token: &Token) {
        match token {
            Token::Char(character) if is_ascii_space(*character) => self.insert_char(*character),
            Token::Comment(data) => self.insert_comment(data),
            Token::Doctype { .. } => self.fault("unexpected-doctype"),
            Token::Start { name, .. } if name == "html" => self.in_body(token),
            Token::End { name } if name == "html" => self.mode = Mode::AfterAfterFrameset,
            Token::Start { name, .. } if name == "noframes" => self.in_head(token),
            Token::Eof => self.stopped = true,
            _ => self.fault("unexpected-token"),
        }
    }

    fn after_after_body(&mut self, token: &Token) {
        match token {
            Token::Comment(data) => self.append_comment(0, data),
            Token::Doctype { .. } => self.in_body(token),
            Token::Char(_) if is_space_char(token) => self.in_body(token),
            Token::Start { name, .. } if name == "html" => self.in_body(token),
            Token::Eof => self.stopped = true,
            _ => {
                self.fault("unexpected-token");
                self.mode = Mode::InBody;
                self.reprocess(token);
            }
        }
    }

    fn after_after_frameset(&mut self, token: &Token) {
        match token {
            Token::Comment(data) => self.append_comment(0, data),
            Token::Doctype { .. } => self.in_body(token),
            Token::Char(_) if is_space_char(token) => self.in_body(token),
            Token::Start { name, .. } if name == "html" => self.in_body(token),
            Token::Eof => self.stopped = true,
            Token::Start { name, .. } if name == "noframes" => self.in_head(token),
            _ => self.fault("unexpected-token"),
        }
    }

    fn foreign(&mut self, token: &Token) {
        match token {
            Token::Char('\0') => {
                self.fault("unexpected-null-character");
                self.insert_char('\u{fffd}');
            }
            Token::Char(character) if is_ascii_space(*character) => self.insert_char(*character),
            Token::Char(character) => {
                self.insert_char(*character);
                self.frameset_ok = false;
            }
            Token::Comment(data) => self.insert_comment(data),
            Token::Doctype { .. } => self.fault("unexpected-doctype"),
            Token::Start {
                name,
                attributes,
                self_closing,
            } => {
                let breakout = matches!(
                    name.as_str(),
                    "b" | "big"
                        | "blockquote"
                        | "body"
                        | "br"
                        | "center"
                        | "code"
                        | "dd"
                        | "div"
                        | "dl"
                        | "dt"
                        | "em"
                        | "embed"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                        | "head"
                        | "hr"
                        | "i"
                        | "img"
                        | "li"
                        | "listing"
                        | "menu"
                        | "meta"
                        | "nobr"
                        | "ol"
                        | "p"
                        | "pre"
                        | "ruby"
                        | "s"
                        | "small"
                        | "span"
                        | "strong"
                        | "strike"
                        | "sub"
                        | "sup"
                        | "table"
                        | "tt"
                        | "u"
                        | "ul"
                        | "var"
                ) || (name == "font"
                    && attributes
                        .iter()
                        .any(|(held, _)| matches!(held.as_str(), "color" | "face" | "size")));
                if breakout {
                    self.fault("unexpected-foreign-start-tag");
                    while let Some(node) = self.current() {
                        if self.namespace_of(node) == Namespace::Html
                            || self.math_text_point(node)
                            || self.html_point(node)
                        {
                            break;
                        }
                        self.open.pop();
                    }
                    let mode = self.mode;
                    self.process(mode, token);
                    return;
                }
                let namespace = self
                    .adjusted_current()
                    .map_or(Namespace::Html, |node| self.namespace_of(node));
                let mut adjusted = plain(attributes);
                let mut tag = name.clone();
                match namespace {
                    Namespace::MathMl => adjust_mathml(&mut adjusted),
                    Namespace::Svg => {
                        if let Some((_, fixed)) = SVG_TAGS.iter().find(|(held, _)| held == name) {
                            tag = (*fixed).to_string();
                        }
                        adjust_svg_attributes(&mut adjusted);
                    }
                    Namespace::Html => {}
                }
                adjust_foreign(&mut adjusted);
                self.insert_element(namespace, &tag, adjusted);
                if *self_closing {
                    self.open.pop();
                    self.acknowledged = true;
                }
            }
            Token::End { name } if name == "br" || name == "p" => {
                self.fault("unexpected-foreign-end-tag");
                while let Some(node) = self.current() {
                    if self.namespace_of(node) == Namespace::Html
                        || self.math_text_point(node)
                        || self.html_point(node)
                    {
                        break;
                    }
                    self.open.pop();
                }
                let mode = self.mode;
                self.process(mode, token);
            }
            Token::End { name } => {
                if name == "script" {
                    if let Some(node) = self.current() {
                        if self.namespace_of(node) == Namespace::Svg
                            && self.name_of(node) == "script"
                        {
                            self.open.pop();
                            return;
                        }
                    }
                }
                let mut at = self.open.len();
                if at == 0 {
                    return;
                }
                at -= 1;
                if !self.name_of(self.open[at]).eq_ignore_ascii_case(name) {
                    self.fault("unexpected-end-tag");
                }
                loop {
                    if at == 0 {
                        return;
                    }
                    let node = self.open[at];
                    if self.name_of(node).eq_ignore_ascii_case(name) {
                        self.pop_through(node);
                        return;
                    }
                    at -= 1;
                    if self.namespace_of(self.open[at]) == Namespace::Html {
                        let mode = self.mode;
                        self.process(mode, token);
                        return;
                    }
                }
            }
            Token::Eof => self.stopped = true,
        }
    }
}

fn clone_subtree(document: &mut Document, id: NodeId) -> NodeId {
    let kind = document.nodes[id].kind.clone();
    let copy = document.create(kind);
    let children = document.nodes[id].children.clone();
    for child in children {
        let inner = clone_subtree(document, child);
        document.append(copy, inner);
    }
    copy
}

fn mirror_selected_options(document: &mut Document) {
    let selects: Vec<NodeId> = document
        .descendants(0)
        .into_iter()
        .filter(|id| {
            document.name(*id) == Some("select") && document.attribute(*id, "multiple").is_none()
        })
        .collect();
    for select in selects {
        let inside = document.descendants(select);
        let Some(target) = inside
            .iter()
            .copied()
            .find(|id| document.name(*id) == Some("selectedcontent"))
        else {
            continue;
        };
        let options: Vec<NodeId> = inside
            .iter()
            .copied()
            .filter(|id| document.name(*id) == Some("option"))
            .collect();
        let chosen = options
            .iter()
            .copied()
            .find(|id| document.attribute(*id, "selected").is_some())
            .or_else(|| options.first().copied());
        let Some(chosen) = chosen else {
            continue;
        };
        let held = std::mem::take(&mut document.nodes[target].children);
        for child in held {
            document.nodes[child].parent = None;
        }
        let children = document.nodes[chosen].children.clone();
        for child in children {
            let copy = clone_subtree(document, child);
            document.append(target, copy);
        }
    }
}

fn finish(mut builder: TreeBuilder) -> Document {
    mirror_selected_options(&mut builder.doc);
    let mut errors = std::mem::take(&mut builder.tok.errors);
    errors.append(&mut builder.errors);
    errors.sort_by_key(|held| held.offset);
    builder.doc.errors = errors;
    builder.doc
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenView {
    Doctype {
        name: Option<String>,
        public_id: Option<String>,
        system_id: Option<String>,
        correct: bool,
    },
    Start {
        name: String,
        attributes: Vec<(String, String)>,
        self_closing: bool,
    },
    End {
        name: String,
    },
    Comment(String),
    Characters(String),
}

pub fn tokens(source: &str, state: &str, last_start: &str) -> Vec<TokenView> {
    tokens_and_errors(source, state, last_start).0
}

pub fn tokens_and_errors(
    source: &str,
    state: &str,
    last_start: &str,
) -> (Vec<TokenView>, Vec<&'static str>) {
    let mut tokenizer = Tokenizer::new(source);
    tokenizer.state = match state {
        "PLAINTEXT state" => State::Plaintext,
        "RCDATA state" => State::Rcdata,
        "RAWTEXT state" => State::Rawtext,
        "Script data state" => State::ScriptData,
        "CDATA section state" => State::CdataSection,
        _ => State::Data,
    };
    tokenizer.cdata_allowed = state == "CDATA section state";
    tokenizer.last_start = last_start.to_string();
    let mut out: Vec<TokenView> = Vec::new();
    loop {
        match tokenizer.next_token() {
            Token::Eof => break,
            Token::Char(character) => match out.last_mut() {
                Some(TokenView::Characters(text)) => text.push(character),
                _ => out.push(TokenView::Characters(character.to_string())),
            },
            Token::Comment(text) => out.push(TokenView::Comment(text)),
            Token::End { name } => out.push(TokenView::End { name }),
            Token::Start {
                name,
                attributes,
                self_closing,
            } => out.push(TokenView::Start {
                name,
                attributes,
                self_closing,
            }),
            Token::Doctype {
                name,
                public_id,
                system_id,
                force_quirks,
            } => out.push(TokenView::Doctype {
                name,
                public_id,
                system_id,
                correct: !force_quirks,
            }),
        }
    }
    let mut codes: Vec<&'static str> = tokenizer.errors.iter().map(|held| held.code).collect();
    codes.sort_unstable();
    (out, codes)
}

pub fn parse_document(source: &str) -> Document {
    parse_document_with(source, true)
}

pub fn parse_document_with(source: &str, scripting: bool) -> Document {
    let mut builder = TreeBuilder::new(source, scripting);
    builder.run();
    finish(builder)
}

pub fn parse_fragment(source: &str, context: &str) -> Document {
    parse_fragment_in(source, Namespace::Html, context, true)
}

pub fn parse_fragment_in(
    source: &str,
    namespace: Namespace,
    context: &str,
    scripting: bool,
) -> Document {
    let mut builder = TreeBuilder::new(source, scripting);
    let context_id = builder.make(namespace, context, Vec::new());
    builder.context = Some(context_id);
    if namespace == Namespace::Html {
        let state = match context {
            "title" | "textarea" => Some(State::Rcdata),
            "style" | "xmp" | "iframe" | "noembed" | "noframes" | "noscript" => {
                Some(State::Rawtext)
            }
            "script" => Some(State::ScriptData),
            "plaintext" => Some(State::Plaintext),
            _ => None,
        };
        if let Some(state) = state {
            builder.tok.state = state;
        }
    }
    let root = builder.make(Namespace::Html, "html", Vec::new());
    builder.doc.append(0, root);
    builder.open.push(root);
    if namespace == Namespace::Html && context == "template" {
        builder.templates.push(Mode::InTemplate);
    }
    builder.reset_mode();
    builder.run();
    let mut document = finish(builder);
    let moved: Vec<NodeId> = std::mem::take(&mut document.nodes[root].children);
    document.nodes[0].children.clear();
    for child in moved {
        document.nodes[child].parent = None;
        document.append(0, child);
    }
    document
}

pub fn parse_strict(source: &str, name: &str) -> Result<Document, Diagnostic> {
    let document = parse_document(source);
    match document.errors.first() {
        None => Ok(document),
        Some(held) => {
            let mut line = 1;
            let mut column = 1;
            let mut seen = 0;
            let mut previous_cr = false;
            for character in source.chars() {
                if seen >= held.offset {
                    break;
                }
                if character == '\n' && previous_cr {
                    previous_cr = false;
                    continue;
                }
                previous_cr = character == '\r';
                seen += 1;
                if character == '\n' || character == '\r' {
                    line += 1;
                    column = 1;
                } else {
                    column += 1;
                }
            }
            Err(error("EI720", "The HTML source has an error.")
                .with_location(Location::at(name, line, column))
                .with_context(format!("Reported: {}", held.code)))
        }
    }
}

const VOID: &[&str] = &[
    "area", "base", "basefont", "bgsound", "br", "col", "embed", "frame", "hr", "img", "input",
    "keygen", "link", "meta", "param", "source", "track", "wbr",
];

fn escape_text(text: &str, attribute: bool, out: &mut String) {
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '\u{a0}' => out.push_str("&nbsp;"),
            '"' if attribute => out.push_str("&quot;"),
            '<' if !attribute => out.push_str("&lt;"),
            '>' if !attribute => out.push_str("&gt;"),
            other => out.push(other),
        }
    }
}

fn write_children(document: &Document, id: NodeId, out: &mut String) {
    let raw = match &document.nodes[id].kind {
        NodeKind::Element {
            namespace: Namespace::Html,
            name,
            ..
        } => matches!(
            name.as_str(),
            "style"
                | "script"
                | "xmp"
                | "iframe"
                | "noembed"
                | "noframes"
                | "plaintext"
                | "noscript"
        ),
        _ => false,
    };
    for &child in &document.nodes[id].children {
        match &document.nodes[child].kind {
            NodeKind::Element {
                namespace,
                name,
                attributes,
                contents,
            } => {
                out.push('<');
                out.push_str(name);
                for attribute in attributes {
                    out.push(' ');
                    if let Some(prefix) = attribute.prefix {
                        out.push_str(prefix);
                        out.push(':');
                    }
                    out.push_str(&attribute.name);
                    out.push_str("=\"");
                    escape_text(&attribute.value, true, out);
                    out.push('"');
                }
                out.push('>');
                if *namespace == Namespace::Html && VOID.contains(&name.as_str()) {
                    continue;
                }
                match contents {
                    Some(inner) => write_children(document, *inner, out),
                    None => write_children(document, child, out),
                }
                out.push_str("</");
                out.push_str(name);
                out.push('>');
            }
            NodeKind::Text(text) => {
                if raw {
                    out.push_str(text);
                } else {
                    escape_text(text, false, out);
                }
            }
            NodeKind::Comment(text) => {
                out.push_str("<!--");
                out.push_str(text);
                out.push_str("-->");
            }
            NodeKind::Doctype { name, .. } => {
                out.push_str("<!DOCTYPE ");
                out.push_str(name);
                out.push('>');
            }
            NodeKind::Document | NodeKind::Fragment => write_children(document, child, out),
        }
    }
}

pub fn serialize(document: &Document, id: NodeId) -> String {
    let mut out = String::new();
    match &document.nodes[id].kind {
        NodeKind::Element {
            contents: Some(inner),
            ..
        } => write_children(document, *inner, &mut out),
        _ => write_children(document, id, &mut out),
    }
    out
}

pub fn serialize_outer(document: &Document, id: NodeId) -> String {
    let mut out = String::new();
    let holder = Document {
        nodes: document.nodes.clone(),
        quirks: document.quirks,
        errors: Vec::new(),
    };
    let mut scratch = holder;
    let wrapper = scratch.create(NodeKind::Fragment);
    scratch.nodes[wrapper].children.push(id);
    write_children(&scratch, wrapper, &mut out);
    out
}

fn dump_node(document: &Document, id: NodeId, depth: usize, out: &mut String) {
    let pad = "  ".repeat(depth);
    match &document.nodes[id].kind {
        NodeKind::Doctype {
            name,
            public_id,
            system_id,
        } => {
            if public_id.is_empty() && system_id.is_empty() {
                out.push_str(&format!("| {pad}<!DOCTYPE {name}>\n"));
            } else {
                out.push_str(&format!(
                    "| {pad}<!DOCTYPE {name} \"{public_id}\" \"{system_id}\">\n"
                ));
            }
        }
        NodeKind::Element {
            namespace,
            name,
            attributes,
            contents,
        } => {
            let qualified = match namespace {
                Namespace::Html => name.clone(),
                Namespace::Svg => format!("svg {name}"),
                Namespace::MathMl => format!("math {name}"),
            };
            out.push_str(&format!("| {pad}<{qualified}>\n"));
            let mut sorted: Vec<(String, &str)> = attributes
                .iter()
                .map(|attribute| {
                    let shown = match attribute.prefix {
                        Some(prefix) => format!("{prefix} {}", attribute.name),
                        None => attribute.name.clone(),
                    };
                    (shown, attribute.value.as_str())
                })
                .collect();
            sorted.sort();
            for (shown, value) in sorted {
                out.push_str(&format!("| {pad}  {shown}=\"{value}\"\n"));
            }
            if let Some(inner) = contents {
                out.push_str(&format!("| {pad}  content\n"));
                for child in &document.nodes[*inner].children {
                    dump_node(document, *child, depth + 2, out);
                }
            }
            for child in &document.nodes[id].children {
                dump_node(document, *child, depth + 1, out);
            }
        }
        NodeKind::Text(text) => out.push_str(&format!("| {pad}\"{text}\"\n")),
        NodeKind::Comment(text) => out.push_str(&format!("| {pad}<!-- {text} -->\n")),
        NodeKind::Document | NodeKind::Fragment => {}
    }
}

pub fn dump(document: &Document) -> String {
    let mut out = String::new();
    for child in &document.nodes[0].children {
        dump_node(document, *child, 0, &mut out);
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScriptKind {
    Classic,
    Module,
    TypeScript,
    ImportMap,
    Data,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Script {
    pub node: NodeId,
    pub kind: ScriptKind,
    pub source: Option<String>,
    pub text: String,
    pub is_async: bool,
    pub defer: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Style {
    pub node: NodeId,
    pub text: String,
    pub media: Option<String>,
}

fn script_kind(document: &Document, id: NodeId) -> ScriptKind {
    let language = document.attribute(id, "lang").unwrap_or("");
    let declared = document
        .attribute(id, "type")
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    if matches!(language, "ts" | "typescript") {
        return ScriptKind::TypeScript;
    }
    match declared.as_str() {
        ""
        | "text/javascript"
        | "application/javascript"
        | "text/ecmascript"
        | "application/ecmascript"
        | "text/jscript"
        | "text/livescript"
        | "text/x-javascript"
        | "application/x-javascript"
        | "text/javascript1.0"
        | "text/javascript1.1"
        | "text/javascript1.2"
        | "text/javascript1.3"
        | "text/javascript1.4"
        | "text/javascript1.5"
        | "text/x-ecmascript"
        | "application/x-ecmascript"
        | "text/jscript1.0" => ScriptKind::Classic,
        "module" => ScriptKind::Module,
        "text/typescript" | "application/typescript" | "text/ts" => ScriptKind::TypeScript,
        "importmap" => ScriptKind::ImportMap,
        _ => ScriptKind::Data,
    }
}

pub fn extract_scripts(document: &Document) -> Vec<Script> {
    document
        .elements_named("script")
        .into_iter()
        .filter(|id| {
            matches!(
                document.node(*id).kind,
                NodeKind::Element {
                    namespace: Namespace::Html,
                    ..
                }
            )
        })
        .map(|id| Script {
            node: id,
            kind: script_kind(document, id),
            source: document.attribute(id, "src").map(str::to_string),
            text: document.text_content(id),
            is_async: document.attribute(id, "async").is_some(),
            defer: document.attribute(id, "defer").is_some(),
        })
        .collect()
}

pub fn extract_styles(document: &Document) -> Vec<Style> {
    document
        .elements_named("style")
        .into_iter()
        .filter(|id| {
            matches!(
                document.node(*id).kind,
                NodeKind::Element {
                    namespace: Namespace::Html,
                    ..
                }
            )
        })
        .filter(|id| {
            let declared = document.attribute(*id, "type").unwrap_or("").trim();
            declared.is_empty() || declared.eq_ignore_ascii_case("text/css")
        })
        .map(|id| Style {
            node: id,
            text: document.text_content(id),
            media: document.attribute(id, "media").map(str::to_string),
        })
        .collect()
}

pub fn extract_inline_styles(document: &Document) -> Vec<(NodeId, String)> {
    document
        .descendants(0)
        .into_iter()
        .filter_map(|id| {
            document
                .attribute(id, "style")
                .map(|value| (id, value.to_string()))
        })
        .collect()
}

pub fn check_scripts(document: &Document, name: &str) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    for script in extract_scripts(document) {
        if script.source.is_some() && script.text.trim().is_empty() {
            continue;
        }
        let module = match script.kind {
            ScriptKind::Classic => false,
            ScriptKind::Module => true,
            _ => continue,
        };
        if let Err(problem) = crate::compilers::js::parse(&script.text, module, name) {
            problems.push(problem);
        }
    }
    problems
}

pub fn check_embedded(document: &Document, name: &str) -> Vec<Diagnostic> {
    let mut problems = Vec::new();
    for style in extract_styles(document) {
        if let Err(problem) = crate::compilers::css::parse_stylesheet(&style.text) {
            problems.push(problem);
        }
    }
    for (_, declarations) in extract_inline_styles(document) {
        if let Err(problem) = crate::compilers::css::parse_declarations(&declarations) {
            problems.push(problem);
        }
    }
    for script in extract_scripts(document) {
        if script.source.is_some() && script.text.trim().is_empty() {
            continue;
        }
        let outcome = match script.kind {
            ScriptKind::Classic => {
                crate::compilers::js::parse(&script.text, false, name).map(|_| ())
            }
            ScriptKind::Module => crate::compilers::js::parse(&script.text, true, name).map(|_| ()),
            ScriptKind::TypeScript => crate::compilers::ts::transpile(
                &script.text,
                &crate::compilers::ts::Options::default(),
            )
            .and_then(|output| {
                let module = output.code.contains("import ") || output.code.contains("export ");
                crate::compilers::js::parse(&output.code, module, name).map(|_| ())
            }),
            _ => Ok(()),
        };
        if let Err(problem) = outcome {
            problems.push(problem);
        }
    }
    problems
}

pub static CONTRACT: crate::plugin::Contract = crate::plugin::Contract {
    id: "plugin.html",
    display_name: "HTML",
    version: crate::plugin::Version::new(0, 2, 0),
    produces_artifacts: true,
    summary: "Reads HTML documents the way a browser does, checks the style sheets and scripts they carry, and writes each document back in a canonical form.",
    inputs: &["html.source"],
    outputs: &["web.document"],
    required_capabilities: &[
        crate::caps::Capability::FsRead,
        crate::caps::Capability::TempStorage,
    ],
    forbidden_capabilities: &[
        crate::caps::Capability::Network,
        crate::caps::Capability::Internet,
        crate::caps::Capability::KeyAccess,
        crate::caps::Capability::SensitiveOutput,
    ],
    non_responsibilities: &[
        "Laying out or drawing a page; the web view does that.",
        "Fetching pages or their resources from a network.",
    ],
};

pub struct HtmlPlugin;

pub static PLUGIN: HtmlPlugin = HtmlPlugin;

impl crate::plugin::Plugin for HtmlPlugin {
    fn contract(&self) -> &'static crate::plugin::Contract {
        &CONTRACT
    }

    fn execute(
        &self,
        _ctx: &mut crate::plugin::Context<'_>,
    ) -> Result<crate::plugin::Outcome, Diagnostic> {
        Err(Diagnostic::new(
            "E0001",
            Severity::Error,
            FailureClass::InternalError,
            CONTRACT.id,
            "HTML compiles through the compiler contract, not through this one.",
        )
        .with_suggestion(
            "The plugin surface has no request to compile and nowhere to put what comes \
             out. Use crate::compiler::Compiler, which has both.",
        ))
    }
}

pub struct HtmlCompiler;

pub static COMPILER: HtmlCompiler = HtmlCompiler;

impl crate::compiler::Compiler for HtmlCompiler {
    fn contract(&self) -> &'static crate::plugin::Contract {
        &CONTRACT
    }

    fn probe(&self) -> crate::compiler::Probe {
        crate::compiler::Probe::usable(
            crate::compiler::Identity::new("html", "build.html", env!("CARGO_PKG_VERSION"), "web")
                .with("standard", "WHATWG HTML Living Standard"),
            crate::compiler::Reproducibility::Always,
        )
    }

    fn plan(
        &self,
        request: &crate::compiler::Request,
    ) -> Result<crate::compiler::Plan, Diagnostic> {
        let identity = self
            .probe()
            .identity
            .expect("this compiler is always usable");
        let expected = request
            .ordered_sources()
            .iter()
            .map(|source| crate::compiler::Expected {
                name: source.path.clone(),
                kind: crate::compiler::Kind::Resource,
            })
            .collect();
        Ok(crate::compiler::Plan {
            key: crate::compiler::Plan::key_for(&identity, request),
            identity,
            expected,
            reproducibility: crate::compiler::Reproducibility::Always,
        })
    }

    fn compile(
        &self,
        plan: &crate::compiler::Plan,
        request: &crate::compiler::Request,
        mut session: crate::compiler::Session<'_>,
    ) -> Result<crate::compiler::Compiled, Diagnostic> {
        for source in request.ordered_sources() {
            session.carry_on()?;
            let bytes = session.store().get(source.digest)?;
            let text = String::from_utf8(bytes).map_err(|_| {
                error("EI721", "An HTML source file is not text.")
                    .with_context(format!("Path: {}", source.path))
            })?;
            let document = parse_document(&text);
            if let Some(problem) = check_embedded(&document, &source.path).into_iter().next() {
                return Err(problem.with_context(format!("File: {}", source.path)));
            }
            session.add_input(
                source.path.clone(),
                crate::compiler::Kind::Resource,
                serialize(&document, document.root()).into_bytes(),
            )?;
        }
        session.commit(plan)
    }
}

const ENTITIES: &[(&str, &str)] = &[
    ("AElig", "\u{c6}"),
    ("AElig;", "\u{c6}"),
    ("AMP", "\u{26}"),
    ("AMP;", "\u{26}"),
    ("Aacute", "\u{c1}"),
    ("Aacute;", "\u{c1}"),
    ("Abreve;", "\u{102}"),
    ("Acirc", "\u{c2}"),
    ("Acirc;", "\u{c2}"),
    ("Acy;", "\u{410}"),
    ("Afr;", "\u{1d504}"),
    ("Agrave", "\u{c0}"),
    ("Agrave;", "\u{c0}"),
    ("Alpha;", "\u{391}"),
    ("Amacr;", "\u{100}"),
    ("And;", "\u{2a53}"),
    ("Aogon;", "\u{104}"),
    ("Aopf;", "\u{1d538}"),
    ("ApplyFunction;", "\u{2061}"),
    ("Aring", "\u{c5}"),
    ("Aring;", "\u{c5}"),
    ("Ascr;", "\u{1d49c}"),
    ("Assign;", "\u{2254}"),
    ("Atilde", "\u{c3}"),
    ("Atilde;", "\u{c3}"),
    ("Auml", "\u{c4}"),
    ("Auml;", "\u{c4}"),
    ("Backslash;", "\u{2216}"),
    ("Barv;", "\u{2ae7}"),
    ("Barwed;", "\u{2306}"),
    ("Bcy;", "\u{411}"),
    ("Because;", "\u{2235}"),
    ("Bernoullis;", "\u{212c}"),
    ("Beta;", "\u{392}"),
    ("Bfr;", "\u{1d505}"),
    ("Bopf;", "\u{1d539}"),
    ("Breve;", "\u{2d8}"),
    ("Bscr;", "\u{212c}"),
    ("Bumpeq;", "\u{224e}"),
    ("CHcy;", "\u{427}"),
    ("COPY", "\u{a9}"),
    ("COPY;", "\u{a9}"),
    ("Cacute;", "\u{106}"),
    ("Cap;", "\u{22d2}"),
    ("CapitalDifferentialD;", "\u{2145}"),
    ("Cayleys;", "\u{212d}"),
    ("Ccaron;", "\u{10c}"),
    ("Ccedil", "\u{c7}"),
    ("Ccedil;", "\u{c7}"),
    ("Ccirc;", "\u{108}"),
    ("Cconint;", "\u{2230}"),
    ("Cdot;", "\u{10a}"),
    ("Cedilla;", "\u{b8}"),
    ("CenterDot;", "\u{b7}"),
    ("Cfr;", "\u{212d}"),
    ("Chi;", "\u{3a7}"),
    ("CircleDot;", "\u{2299}"),
    ("CircleMinus;", "\u{2296}"),
    ("CirclePlus;", "\u{2295}"),
    ("CircleTimes;", "\u{2297}"),
    ("ClockwiseContourIntegral;", "\u{2232}"),
    ("CloseCurlyDoubleQuote;", "\u{201d}"),
    ("CloseCurlyQuote;", "\u{2019}"),
    ("Colon;", "\u{2237}"),
    ("Colone;", "\u{2a74}"),
    ("Congruent;", "\u{2261}"),
    ("Conint;", "\u{222f}"),
    ("ContourIntegral;", "\u{222e}"),
    ("Copf;", "\u{2102}"),
    ("Coproduct;", "\u{2210}"),
    ("CounterClockwiseContourIntegral;", "\u{2233}"),
    ("Cross;", "\u{2a2f}"),
    ("Cscr;", "\u{1d49e}"),
    ("Cup;", "\u{22d3}"),
    ("CupCap;", "\u{224d}"),
    ("DD;", "\u{2145}"),
    ("DDotrahd;", "\u{2911}"),
    ("DJcy;", "\u{402}"),
    ("DScy;", "\u{405}"),
    ("DZcy;", "\u{40f}"),
    ("Dagger;", "\u{2021}"),
    ("Darr;", "\u{21a1}"),
    ("Dashv;", "\u{2ae4}"),
    ("Dcaron;", "\u{10e}"),
    ("Dcy;", "\u{414}"),
    ("Del;", "\u{2207}"),
    ("Delta;", "\u{394}"),
    ("Dfr;", "\u{1d507}"),
    ("DiacriticalAcute;", "\u{b4}"),
    ("DiacriticalDot;", "\u{2d9}"),
    ("DiacriticalDoubleAcute;", "\u{2dd}"),
    ("DiacriticalGrave;", "\u{60}"),
    ("DiacriticalTilde;", "\u{2dc}"),
    ("Diamond;", "\u{22c4}"),
    ("DifferentialD;", "\u{2146}"),
    ("Dopf;", "\u{1d53b}"),
    ("Dot;", "\u{a8}"),
    ("DotDot;", "\u{20dc}"),
    ("DotEqual;", "\u{2250}"),
    ("DoubleContourIntegral;", "\u{222f}"),
    ("DoubleDot;", "\u{a8}"),
    ("DoubleDownArrow;", "\u{21d3}"),
    ("DoubleLeftArrow;", "\u{21d0}"),
    ("DoubleLeftRightArrow;", "\u{21d4}"),
    ("DoubleLeftTee;", "\u{2ae4}"),
    ("DoubleLongLeftArrow;", "\u{27f8}"),
    ("DoubleLongLeftRightArrow;", "\u{27fa}"),
    ("DoubleLongRightArrow;", "\u{27f9}"),
    ("DoubleRightArrow;", "\u{21d2}"),
    ("DoubleRightTee;", "\u{22a8}"),
    ("DoubleUpArrow;", "\u{21d1}"),
    ("DoubleUpDownArrow;", "\u{21d5}"),
    ("DoubleVerticalBar;", "\u{2225}"),
    ("DownArrow;", "\u{2193}"),
    ("DownArrowBar;", "\u{2913}"),
    ("DownArrowUpArrow;", "\u{21f5}"),
    ("DownBreve;", "\u{311}"),
    ("DownLeftRightVector;", "\u{2950}"),
    ("DownLeftTeeVector;", "\u{295e}"),
    ("DownLeftVector;", "\u{21bd}"),
    ("DownLeftVectorBar;", "\u{2956}"),
    ("DownRightTeeVector;", "\u{295f}"),
    ("DownRightVector;", "\u{21c1}"),
    ("DownRightVectorBar;", "\u{2957}"),
    ("DownTee;", "\u{22a4}"),
    ("DownTeeArrow;", "\u{21a7}"),
    ("Downarrow;", "\u{21d3}"),
    ("Dscr;", "\u{1d49f}"),
    ("Dstrok;", "\u{110}"),
    ("ENG;", "\u{14a}"),
    ("ETH", "\u{d0}"),
    ("ETH;", "\u{d0}"),
    ("Eacute", "\u{c9}"),
    ("Eacute;", "\u{c9}"),
    ("Ecaron;", "\u{11a}"),
    ("Ecirc", "\u{ca}"),
    ("Ecirc;", "\u{ca}"),
    ("Ecy;", "\u{42d}"),
    ("Edot;", "\u{116}"),
    ("Efr;", "\u{1d508}"),
    ("Egrave", "\u{c8}"),
    ("Egrave;", "\u{c8}"),
    ("Element;", "\u{2208}"),
    ("Emacr;", "\u{112}"),
    ("EmptySmallSquare;", "\u{25fb}"),
    ("EmptyVerySmallSquare;", "\u{25ab}"),
    ("Eogon;", "\u{118}"),
    ("Eopf;", "\u{1d53c}"),
    ("Epsilon;", "\u{395}"),
    ("Equal;", "\u{2a75}"),
    ("EqualTilde;", "\u{2242}"),
    ("Equilibrium;", "\u{21cc}"),
    ("Escr;", "\u{2130}"),
    ("Esim;", "\u{2a73}"),
    ("Eta;", "\u{397}"),
    ("Euml", "\u{cb}"),
    ("Euml;", "\u{cb}"),
    ("Exists;", "\u{2203}"),
    ("ExponentialE;", "\u{2147}"),
    ("Fcy;", "\u{424}"),
    ("Ffr;", "\u{1d509}"),
    ("FilledSmallSquare;", "\u{25fc}"),
    ("FilledVerySmallSquare;", "\u{25aa}"),
    ("Fopf;", "\u{1d53d}"),
    ("ForAll;", "\u{2200}"),
    ("Fouriertrf;", "\u{2131}"),
    ("Fscr;", "\u{2131}"),
    ("GJcy;", "\u{403}"),
    ("GT", "\u{3e}"),
    ("GT;", "\u{3e}"),
    ("Gamma;", "\u{393}"),
    ("Gammad;", "\u{3dc}"),
    ("Gbreve;", "\u{11e}"),
    ("Gcedil;", "\u{122}"),
    ("Gcirc;", "\u{11c}"),
    ("Gcy;", "\u{413}"),
    ("Gdot;", "\u{120}"),
    ("Gfr;", "\u{1d50a}"),
    ("Gg;", "\u{22d9}"),
    ("Gopf;", "\u{1d53e}"),
    ("GreaterEqual;", "\u{2265}"),
    ("GreaterEqualLess;", "\u{22db}"),
    ("GreaterFullEqual;", "\u{2267}"),
    ("GreaterGreater;", "\u{2aa2}"),
    ("GreaterLess;", "\u{2277}"),
    ("GreaterSlantEqual;", "\u{2a7e}"),
    ("GreaterTilde;", "\u{2273}"),
    ("Gscr;", "\u{1d4a2}"),
    ("Gt;", "\u{226b}"),
    ("HARDcy;", "\u{42a}"),
    ("Hacek;", "\u{2c7}"),
    ("Hat;", "\u{5e}"),
    ("Hcirc;", "\u{124}"),
    ("Hfr;", "\u{210c}"),
    ("HilbertSpace;", "\u{210b}"),
    ("Hopf;", "\u{210d}"),
    ("HorizontalLine;", "\u{2500}"),
    ("Hscr;", "\u{210b}"),
    ("Hstrok;", "\u{126}"),
    ("HumpDownHump;", "\u{224e}"),
    ("HumpEqual;", "\u{224f}"),
    ("IEcy;", "\u{415}"),
    ("IJlig;", "\u{132}"),
    ("IOcy;", "\u{401}"),
    ("Iacute", "\u{cd}"),
    ("Iacute;", "\u{cd}"),
    ("Icirc", "\u{ce}"),
    ("Icirc;", "\u{ce}"),
    ("Icy;", "\u{418}"),
    ("Idot;", "\u{130}"),
    ("Ifr;", "\u{2111}"),
    ("Igrave", "\u{cc}"),
    ("Igrave;", "\u{cc}"),
    ("Im;", "\u{2111}"),
    ("Imacr;", "\u{12a}"),
    ("ImaginaryI;", "\u{2148}"),
    ("Implies;", "\u{21d2}"),
    ("Int;", "\u{222c}"),
    ("Integral;", "\u{222b}"),
    ("Intersection;", "\u{22c2}"),
    ("InvisibleComma;", "\u{2063}"),
    ("InvisibleTimes;", "\u{2062}"),
    ("Iogon;", "\u{12e}"),
    ("Iopf;", "\u{1d540}"),
    ("Iota;", "\u{399}"),
    ("Iscr;", "\u{2110}"),
    ("Itilde;", "\u{128}"),
    ("Iukcy;", "\u{406}"),
    ("Iuml", "\u{cf}"),
    ("Iuml;", "\u{cf}"),
    ("Jcirc;", "\u{134}"),
    ("Jcy;", "\u{419}"),
    ("Jfr;", "\u{1d50d}"),
    ("Jopf;", "\u{1d541}"),
    ("Jscr;", "\u{1d4a5}"),
    ("Jsercy;", "\u{408}"),
    ("Jukcy;", "\u{404}"),
    ("KHcy;", "\u{425}"),
    ("KJcy;", "\u{40c}"),
    ("Kappa;", "\u{39a}"),
    ("Kcedil;", "\u{136}"),
    ("Kcy;", "\u{41a}"),
    ("Kfr;", "\u{1d50e}"),
    ("Kopf;", "\u{1d542}"),
    ("Kscr;", "\u{1d4a6}"),
    ("LJcy;", "\u{409}"),
    ("LT", "\u{3c}"),
    ("LT;", "\u{3c}"),
    ("Lacute;", "\u{139}"),
    ("Lambda;", "\u{39b}"),
    ("Lang;", "\u{27ea}"),
    ("Laplacetrf;", "\u{2112}"),
    ("Larr;", "\u{219e}"),
    ("Lcaron;", "\u{13d}"),
    ("Lcedil;", "\u{13b}"),
    ("Lcy;", "\u{41b}"),
    ("LeftAngleBracket;", "\u{27e8}"),
    ("LeftArrow;", "\u{2190}"),
    ("LeftArrowBar;", "\u{21e4}"),
    ("LeftArrowRightArrow;", "\u{21c6}"),
    ("LeftCeiling;", "\u{2308}"),
    ("LeftDoubleBracket;", "\u{27e6}"),
    ("LeftDownTeeVector;", "\u{2961}"),
    ("LeftDownVector;", "\u{21c3}"),
    ("LeftDownVectorBar;", "\u{2959}"),
    ("LeftFloor;", "\u{230a}"),
    ("LeftRightArrow;", "\u{2194}"),
    ("LeftRightVector;", "\u{294e}"),
    ("LeftTee;", "\u{22a3}"),
    ("LeftTeeArrow;", "\u{21a4}"),
    ("LeftTeeVector;", "\u{295a}"),
    ("LeftTriangle;", "\u{22b2}"),
    ("LeftTriangleBar;", "\u{29cf}"),
    ("LeftTriangleEqual;", "\u{22b4}"),
    ("LeftUpDownVector;", "\u{2951}"),
    ("LeftUpTeeVector;", "\u{2960}"),
    ("LeftUpVector;", "\u{21bf}"),
    ("LeftUpVectorBar;", "\u{2958}"),
    ("LeftVector;", "\u{21bc}"),
    ("LeftVectorBar;", "\u{2952}"),
    ("Leftarrow;", "\u{21d0}"),
    ("Leftrightarrow;", "\u{21d4}"),
    ("LessEqualGreater;", "\u{22da}"),
    ("LessFullEqual;", "\u{2266}"),
    ("LessGreater;", "\u{2276}"),
    ("LessLess;", "\u{2aa1}"),
    ("LessSlantEqual;", "\u{2a7d}"),
    ("LessTilde;", "\u{2272}"),
    ("Lfr;", "\u{1d50f}"),
    ("Ll;", "\u{22d8}"),
    ("Lleftarrow;", "\u{21da}"),
    ("Lmidot;", "\u{13f}"),
    ("LongLeftArrow;", "\u{27f5}"),
    ("LongLeftRightArrow;", "\u{27f7}"),
    ("LongRightArrow;", "\u{27f6}"),
    ("Longleftarrow;", "\u{27f8}"),
    ("Longleftrightarrow;", "\u{27fa}"),
    ("Longrightarrow;", "\u{27f9}"),
    ("Lopf;", "\u{1d543}"),
    ("LowerLeftArrow;", "\u{2199}"),
    ("LowerRightArrow;", "\u{2198}"),
    ("Lscr;", "\u{2112}"),
    ("Lsh;", "\u{21b0}"),
    ("Lstrok;", "\u{141}"),
    ("Lt;", "\u{226a}"),
    ("Map;", "\u{2905}"),
    ("Mcy;", "\u{41c}"),
    ("MediumSpace;", "\u{205f}"),
    ("Mellintrf;", "\u{2133}"),
    ("Mfr;", "\u{1d510}"),
    ("MinusPlus;", "\u{2213}"),
    ("Mopf;", "\u{1d544}"),
    ("Mscr;", "\u{2133}"),
    ("Mu;", "\u{39c}"),
    ("NJcy;", "\u{40a}"),
    ("Nacute;", "\u{143}"),
    ("Ncaron;", "\u{147}"),
    ("Ncedil;", "\u{145}"),
    ("Ncy;", "\u{41d}"),
    ("NegativeMediumSpace;", "\u{200b}"),
    ("NegativeThickSpace;", "\u{200b}"),
    ("NegativeThinSpace;", "\u{200b}"),
    ("NegativeVeryThinSpace;", "\u{200b}"),
    ("NestedGreaterGreater;", "\u{226b}"),
    ("NestedLessLess;", "\u{226a}"),
    ("NewLine;", "\u{a}"),
    ("Nfr;", "\u{1d511}"),
    ("NoBreak;", "\u{2060}"),
    ("NonBreakingSpace;", "\u{a0}"),
    ("Nopf;", "\u{2115}"),
    ("Not;", "\u{2aec}"),
    ("NotCongruent;", "\u{2262}"),
    ("NotCupCap;", "\u{226d}"),
    ("NotDoubleVerticalBar;", "\u{2226}"),
    ("NotElement;", "\u{2209}"),
    ("NotEqual;", "\u{2260}"),
    ("NotEqualTilde;", "\u{2242}\u{338}"),
    ("NotExists;", "\u{2204}"),
    ("NotGreater;", "\u{226f}"),
    ("NotGreaterEqual;", "\u{2271}"),
    ("NotGreaterFullEqual;", "\u{2267}\u{338}"),
    ("NotGreaterGreater;", "\u{226b}\u{338}"),
    ("NotGreaterLess;", "\u{2279}"),
    ("NotGreaterSlantEqual;", "\u{2a7e}\u{338}"),
    ("NotGreaterTilde;", "\u{2275}"),
    ("NotHumpDownHump;", "\u{224e}\u{338}"),
    ("NotHumpEqual;", "\u{224f}\u{338}"),
    ("NotLeftTriangle;", "\u{22ea}"),
    ("NotLeftTriangleBar;", "\u{29cf}\u{338}"),
    ("NotLeftTriangleEqual;", "\u{22ec}"),
    ("NotLess;", "\u{226e}"),
    ("NotLessEqual;", "\u{2270}"),
    ("NotLessGreater;", "\u{2278}"),
    ("NotLessLess;", "\u{226a}\u{338}"),
    ("NotLessSlantEqual;", "\u{2a7d}\u{338}"),
    ("NotLessTilde;", "\u{2274}"),
    ("NotNestedGreaterGreater;", "\u{2aa2}\u{338}"),
    ("NotNestedLessLess;", "\u{2aa1}\u{338}"),
    ("NotPrecedes;", "\u{2280}"),
    ("NotPrecedesEqual;", "\u{2aaf}\u{338}"),
    ("NotPrecedesSlantEqual;", "\u{22e0}"),
    ("NotReverseElement;", "\u{220c}"),
    ("NotRightTriangle;", "\u{22eb}"),
    ("NotRightTriangleBar;", "\u{29d0}\u{338}"),
    ("NotRightTriangleEqual;", "\u{22ed}"),
    ("NotSquareSubset;", "\u{228f}\u{338}"),
    ("NotSquareSubsetEqual;", "\u{22e2}"),
    ("NotSquareSuperset;", "\u{2290}\u{338}"),
    ("NotSquareSupersetEqual;", "\u{22e3}"),
    ("NotSubset;", "\u{2282}\u{20d2}"),
    ("NotSubsetEqual;", "\u{2288}"),
    ("NotSucceeds;", "\u{2281}"),
    ("NotSucceedsEqual;", "\u{2ab0}\u{338}"),
    ("NotSucceedsSlantEqual;", "\u{22e1}"),
    ("NotSucceedsTilde;", "\u{227f}\u{338}"),
    ("NotSuperset;", "\u{2283}\u{20d2}"),
    ("NotSupersetEqual;", "\u{2289}"),
    ("NotTilde;", "\u{2241}"),
    ("NotTildeEqual;", "\u{2244}"),
    ("NotTildeFullEqual;", "\u{2247}"),
    ("NotTildeTilde;", "\u{2249}"),
    ("NotVerticalBar;", "\u{2224}"),
    ("Nscr;", "\u{1d4a9}"),
    ("Ntilde", "\u{d1}"),
    ("Ntilde;", "\u{d1}"),
    ("Nu;", "\u{39d}"),
    ("OElig;", "\u{152}"),
    ("Oacute", "\u{d3}"),
    ("Oacute;", "\u{d3}"),
    ("Ocirc", "\u{d4}"),
    ("Ocirc;", "\u{d4}"),
    ("Ocy;", "\u{41e}"),
    ("Odblac;", "\u{150}"),
    ("Ofr;", "\u{1d512}"),
    ("Ograve", "\u{d2}"),
    ("Ograve;", "\u{d2}"),
    ("Omacr;", "\u{14c}"),
    ("Omega;", "\u{3a9}"),
    ("Omicron;", "\u{39f}"),
    ("Oopf;", "\u{1d546}"),
    ("OpenCurlyDoubleQuote;", "\u{201c}"),
    ("OpenCurlyQuote;", "\u{2018}"),
    ("Or;", "\u{2a54}"),
    ("Oscr;", "\u{1d4aa}"),
    ("Oslash", "\u{d8}"),
    ("Oslash;", "\u{d8}"),
    ("Otilde", "\u{d5}"),
    ("Otilde;", "\u{d5}"),
    ("Otimes;", "\u{2a37}"),
    ("Ouml", "\u{d6}"),
    ("Ouml;", "\u{d6}"),
    ("OverBar;", "\u{203e}"),
    ("OverBrace;", "\u{23de}"),
    ("OverBracket;", "\u{23b4}"),
    ("OverParenthesis;", "\u{23dc}"),
    ("PartialD;", "\u{2202}"),
    ("Pcy;", "\u{41f}"),
    ("Pfr;", "\u{1d513}"),
    ("Phi;", "\u{3a6}"),
    ("Pi;", "\u{3a0}"),
    ("PlusMinus;", "\u{b1}"),
    ("Poincareplane;", "\u{210c}"),
    ("Popf;", "\u{2119}"),
    ("Pr;", "\u{2abb}"),
    ("Precedes;", "\u{227a}"),
    ("PrecedesEqual;", "\u{2aaf}"),
    ("PrecedesSlantEqual;", "\u{227c}"),
    ("PrecedesTilde;", "\u{227e}"),
    ("Prime;", "\u{2033}"),
    ("Product;", "\u{220f}"),
    ("Proportion;", "\u{2237}"),
    ("Proportional;", "\u{221d}"),
    ("Pscr;", "\u{1d4ab}"),
    ("Psi;", "\u{3a8}"),
    ("QUOT", "\u{22}"),
    ("QUOT;", "\u{22}"),
    ("Qfr;", "\u{1d514}"),
    ("Qopf;", "\u{211a}"),
    ("Qscr;", "\u{1d4ac}"),
    ("RBarr;", "\u{2910}"),
    ("REG", "\u{ae}"),
    ("REG;", "\u{ae}"),
    ("Racute;", "\u{154}"),
    ("Rang;", "\u{27eb}"),
    ("Rarr;", "\u{21a0}"),
    ("Rarrtl;", "\u{2916}"),
    ("Rcaron;", "\u{158}"),
    ("Rcedil;", "\u{156}"),
    ("Rcy;", "\u{420}"),
    ("Re;", "\u{211c}"),
    ("ReverseElement;", "\u{220b}"),
    ("ReverseEquilibrium;", "\u{21cb}"),
    ("ReverseUpEquilibrium;", "\u{296f}"),
    ("Rfr;", "\u{211c}"),
    ("Rho;", "\u{3a1}"),
    ("RightAngleBracket;", "\u{27e9}"),
    ("RightArrow;", "\u{2192}"),
    ("RightArrowBar;", "\u{21e5}"),
    ("RightArrowLeftArrow;", "\u{21c4}"),
    ("RightCeiling;", "\u{2309}"),
    ("RightDoubleBracket;", "\u{27e7}"),
    ("RightDownTeeVector;", "\u{295d}"),
    ("RightDownVector;", "\u{21c2}"),
    ("RightDownVectorBar;", "\u{2955}"),
    ("RightFloor;", "\u{230b}"),
    ("RightTee;", "\u{22a2}"),
    ("RightTeeArrow;", "\u{21a6}"),
    ("RightTeeVector;", "\u{295b}"),
    ("RightTriangle;", "\u{22b3}"),
    ("RightTriangleBar;", "\u{29d0}"),
    ("RightTriangleEqual;", "\u{22b5}"),
    ("RightUpDownVector;", "\u{294f}"),
    ("RightUpTeeVector;", "\u{295c}"),
    ("RightUpVector;", "\u{21be}"),
    ("RightUpVectorBar;", "\u{2954}"),
    ("RightVector;", "\u{21c0}"),
    ("RightVectorBar;", "\u{2953}"),
    ("Rightarrow;", "\u{21d2}"),
    ("Ropf;", "\u{211d}"),
    ("RoundImplies;", "\u{2970}"),
    ("Rrightarrow;", "\u{21db}"),
    ("Rscr;", "\u{211b}"),
    ("Rsh;", "\u{21b1}"),
    ("RuleDelayed;", "\u{29f4}"),
    ("SHCHcy;", "\u{429}"),
    ("SHcy;", "\u{428}"),
    ("SOFTcy;", "\u{42c}"),
    ("Sacute;", "\u{15a}"),
    ("Sc;", "\u{2abc}"),
    ("Scaron;", "\u{160}"),
    ("Scedil;", "\u{15e}"),
    ("Scirc;", "\u{15c}"),
    ("Scy;", "\u{421}"),
    ("Sfr;", "\u{1d516}"),
    ("ShortDownArrow;", "\u{2193}"),
    ("ShortLeftArrow;", "\u{2190}"),
    ("ShortRightArrow;", "\u{2192}"),
    ("ShortUpArrow;", "\u{2191}"),
    ("Sigma;", "\u{3a3}"),
    ("SmallCircle;", "\u{2218}"),
    ("Sopf;", "\u{1d54a}"),
    ("Sqrt;", "\u{221a}"),
    ("Square;", "\u{25a1}"),
    ("SquareIntersection;", "\u{2293}"),
    ("SquareSubset;", "\u{228f}"),
    ("SquareSubsetEqual;", "\u{2291}"),
    ("SquareSuperset;", "\u{2290}"),
    ("SquareSupersetEqual;", "\u{2292}"),
    ("SquareUnion;", "\u{2294}"),
    ("Sscr;", "\u{1d4ae}"),
    ("Star;", "\u{22c6}"),
    ("Sub;", "\u{22d0}"),
    ("Subset;", "\u{22d0}"),
    ("SubsetEqual;", "\u{2286}"),
    ("Succeeds;", "\u{227b}"),
    ("SucceedsEqual;", "\u{2ab0}"),
    ("SucceedsSlantEqual;", "\u{227d}"),
    ("SucceedsTilde;", "\u{227f}"),
    ("SuchThat;", "\u{220b}"),
    ("Sum;", "\u{2211}"),
    ("Sup;", "\u{22d1}"),
    ("Superset;", "\u{2283}"),
    ("SupersetEqual;", "\u{2287}"),
    ("Supset;", "\u{22d1}"),
    ("THORN", "\u{de}"),
    ("THORN;", "\u{de}"),
    ("TRADE;", "\u{2122}"),
    ("TSHcy;", "\u{40b}"),
    ("TScy;", "\u{426}"),
    ("Tab;", "\u{9}"),
    ("Tau;", "\u{3a4}"),
    ("Tcaron;", "\u{164}"),
    ("Tcedil;", "\u{162}"),
    ("Tcy;", "\u{422}"),
    ("Tfr;", "\u{1d517}"),
    ("Therefore;", "\u{2234}"),
    ("Theta;", "\u{398}"),
    ("ThickSpace;", "\u{205f}\u{200a}"),
    ("ThinSpace;", "\u{2009}"),
    ("Tilde;", "\u{223c}"),
    ("TildeEqual;", "\u{2243}"),
    ("TildeFullEqual;", "\u{2245}"),
    ("TildeTilde;", "\u{2248}"),
    ("Topf;", "\u{1d54b}"),
    ("TripleDot;", "\u{20db}"),
    ("Tscr;", "\u{1d4af}"),
    ("Tstrok;", "\u{166}"),
    ("Uacute", "\u{da}"),
    ("Uacute;", "\u{da}"),
    ("Uarr;", "\u{219f}"),
    ("Uarrocir;", "\u{2949}"),
    ("Ubrcy;", "\u{40e}"),
    ("Ubreve;", "\u{16c}"),
    ("Ucirc", "\u{db}"),
    ("Ucirc;", "\u{db}"),
    ("Ucy;", "\u{423}"),
    ("Udblac;", "\u{170}"),
    ("Ufr;", "\u{1d518}"),
    ("Ugrave", "\u{d9}"),
    ("Ugrave;", "\u{d9}"),
    ("Umacr;", "\u{16a}"),
    ("UnderBar;", "\u{5f}"),
    ("UnderBrace;", "\u{23df}"),
    ("UnderBracket;", "\u{23b5}"),
    ("UnderParenthesis;", "\u{23dd}"),
    ("Union;", "\u{22c3}"),
    ("UnionPlus;", "\u{228e}"),
    ("Uogon;", "\u{172}"),
    ("Uopf;", "\u{1d54c}"),
    ("UpArrow;", "\u{2191}"),
    ("UpArrowBar;", "\u{2912}"),
    ("UpArrowDownArrow;", "\u{21c5}"),
    ("UpDownArrow;", "\u{2195}"),
    ("UpEquilibrium;", "\u{296e}"),
    ("UpTee;", "\u{22a5}"),
    ("UpTeeArrow;", "\u{21a5}"),
    ("Uparrow;", "\u{21d1}"),
    ("Updownarrow;", "\u{21d5}"),
    ("UpperLeftArrow;", "\u{2196}"),
    ("UpperRightArrow;", "\u{2197}"),
    ("Upsi;", "\u{3d2}"),
    ("Upsilon;", "\u{3a5}"),
    ("Uring;", "\u{16e}"),
    ("Uscr;", "\u{1d4b0}"),
    ("Utilde;", "\u{168}"),
    ("Uuml", "\u{dc}"),
    ("Uuml;", "\u{dc}"),
    ("VDash;", "\u{22ab}"),
    ("Vbar;", "\u{2aeb}"),
    ("Vcy;", "\u{412}"),
    ("Vdash;", "\u{22a9}"),
    ("Vdashl;", "\u{2ae6}"),
    ("Vee;", "\u{22c1}"),
    ("Verbar;", "\u{2016}"),
    ("Vert;", "\u{2016}"),
    ("VerticalBar;", "\u{2223}"),
    ("VerticalLine;", "\u{7c}"),
    ("VerticalSeparator;", "\u{2758}"),
    ("VerticalTilde;", "\u{2240}"),
    ("VeryThinSpace;", "\u{200a}"),
    ("Vfr;", "\u{1d519}"),
    ("Vopf;", "\u{1d54d}"),
    ("Vscr;", "\u{1d4b1}"),
    ("Vvdash;", "\u{22aa}"),
    ("Wcirc;", "\u{174}"),
    ("Wedge;", "\u{22c0}"),
    ("Wfr;", "\u{1d51a}"),
    ("Wopf;", "\u{1d54e}"),
    ("Wscr;", "\u{1d4b2}"),
    ("Xfr;", "\u{1d51b}"),
    ("Xi;", "\u{39e}"),
    ("Xopf;", "\u{1d54f}"),
    ("Xscr;", "\u{1d4b3}"),
    ("YAcy;", "\u{42f}"),
    ("YIcy;", "\u{407}"),
    ("YUcy;", "\u{42e}"),
    ("Yacute", "\u{dd}"),
    ("Yacute;", "\u{dd}"),
    ("Ycirc;", "\u{176}"),
    ("Ycy;", "\u{42b}"),
    ("Yfr;", "\u{1d51c}"),
    ("Yopf;", "\u{1d550}"),
    ("Yscr;", "\u{1d4b4}"),
    ("Yuml;", "\u{178}"),
    ("ZHcy;", "\u{416}"),
    ("Zacute;", "\u{179}"),
    ("Zcaron;", "\u{17d}"),
    ("Zcy;", "\u{417}"),
    ("Zdot;", "\u{17b}"),
    ("ZeroWidthSpace;", "\u{200b}"),
    ("Zeta;", "\u{396}"),
    ("Zfr;", "\u{2128}"),
    ("Zopf;", "\u{2124}"),
    ("Zscr;", "\u{1d4b5}"),
    ("aacute", "\u{e1}"),
    ("aacute;", "\u{e1}"),
    ("abreve;", "\u{103}"),
    ("ac;", "\u{223e}"),
    ("acE;", "\u{223e}\u{333}"),
    ("acd;", "\u{223f}"),
    ("acirc", "\u{e2}"),
    ("acirc;", "\u{e2}"),
    ("acute", "\u{b4}"),
    ("acute;", "\u{b4}"),
    ("acy;", "\u{430}"),
    ("aelig", "\u{e6}"),
    ("aelig;", "\u{e6}"),
    ("af;", "\u{2061}"),
    ("afr;", "\u{1d51e}"),
    ("agrave", "\u{e0}"),
    ("agrave;", "\u{e0}"),
    ("alefsym;", "\u{2135}"),
    ("aleph;", "\u{2135}"),
    ("alpha;", "\u{3b1}"),
    ("amacr;", "\u{101}"),
    ("amalg;", "\u{2a3f}"),
    ("amp", "\u{26}"),
    ("amp;", "\u{26}"),
    ("and;", "\u{2227}"),
    ("andand;", "\u{2a55}"),
    ("andd;", "\u{2a5c}"),
    ("andslope;", "\u{2a58}"),
    ("andv;", "\u{2a5a}"),
    ("ang;", "\u{2220}"),
    ("ange;", "\u{29a4}"),
    ("angle;", "\u{2220}"),
    ("angmsd;", "\u{2221}"),
    ("angmsdaa;", "\u{29a8}"),
    ("angmsdab;", "\u{29a9}"),
    ("angmsdac;", "\u{29aa}"),
    ("angmsdad;", "\u{29ab}"),
    ("angmsdae;", "\u{29ac}"),
    ("angmsdaf;", "\u{29ad}"),
    ("angmsdag;", "\u{29ae}"),
    ("angmsdah;", "\u{29af}"),
    ("angrt;", "\u{221f}"),
    ("angrtvb;", "\u{22be}"),
    ("angrtvbd;", "\u{299d}"),
    ("angsph;", "\u{2222}"),
    ("angst;", "\u{c5}"),
    ("angzarr;", "\u{237c}"),
    ("aogon;", "\u{105}"),
    ("aopf;", "\u{1d552}"),
    ("ap;", "\u{2248}"),
    ("apE;", "\u{2a70}"),
    ("apacir;", "\u{2a6f}"),
    ("ape;", "\u{224a}"),
    ("apid;", "\u{224b}"),
    ("apos;", "\u{27}"),
    ("approx;", "\u{2248}"),
    ("approxeq;", "\u{224a}"),
    ("aring", "\u{e5}"),
    ("aring;", "\u{e5}"),
    ("ascr;", "\u{1d4b6}"),
    ("ast;", "\u{2a}"),
    ("asymp;", "\u{2248}"),
    ("asympeq;", "\u{224d}"),
    ("atilde", "\u{e3}"),
    ("atilde;", "\u{e3}"),
    ("auml", "\u{e4}"),
    ("auml;", "\u{e4}"),
    ("awconint;", "\u{2233}"),
    ("awint;", "\u{2a11}"),
    ("bNot;", "\u{2aed}"),
    ("backcong;", "\u{224c}"),
    ("backepsilon;", "\u{3f6}"),
    ("backprime;", "\u{2035}"),
    ("backsim;", "\u{223d}"),
    ("backsimeq;", "\u{22cd}"),
    ("barvee;", "\u{22bd}"),
    ("barwed;", "\u{2305}"),
    ("barwedge;", "\u{2305}"),
    ("bbrk;", "\u{23b5}"),
    ("bbrktbrk;", "\u{23b6}"),
    ("bcong;", "\u{224c}"),
    ("bcy;", "\u{431}"),
    ("bdquo;", "\u{201e}"),
    ("becaus;", "\u{2235}"),
    ("because;", "\u{2235}"),
    ("bemptyv;", "\u{29b0}"),
    ("bepsi;", "\u{3f6}"),
    ("bernou;", "\u{212c}"),
    ("beta;", "\u{3b2}"),
    ("beth;", "\u{2136}"),
    ("between;", "\u{226c}"),
    ("bfr;", "\u{1d51f}"),
    ("bigcap;", "\u{22c2}"),
    ("bigcirc;", "\u{25ef}"),
    ("bigcup;", "\u{22c3}"),
    ("bigodot;", "\u{2a00}"),
    ("bigoplus;", "\u{2a01}"),
    ("bigotimes;", "\u{2a02}"),
    ("bigsqcup;", "\u{2a06}"),
    ("bigstar;", "\u{2605}"),
    ("bigtriangledown;", "\u{25bd}"),
    ("bigtriangleup;", "\u{25b3}"),
    ("biguplus;", "\u{2a04}"),
    ("bigvee;", "\u{22c1}"),
    ("bigwedge;", "\u{22c0}"),
    ("bkarow;", "\u{290d}"),
    ("blacklozenge;", "\u{29eb}"),
    ("blacksquare;", "\u{25aa}"),
    ("blacktriangle;", "\u{25b4}"),
    ("blacktriangledown;", "\u{25be}"),
    ("blacktriangleleft;", "\u{25c2}"),
    ("blacktriangleright;", "\u{25b8}"),
    ("blank;", "\u{2423}"),
    ("blk12;", "\u{2592}"),
    ("blk14;", "\u{2591}"),
    ("blk34;", "\u{2593}"),
    ("block;", "\u{2588}"),
    ("bne;", "\u{3d}\u{20e5}"),
    ("bnequiv;", "\u{2261}\u{20e5}"),
    ("bnot;", "\u{2310}"),
    ("bopf;", "\u{1d553}"),
    ("bot;", "\u{22a5}"),
    ("bottom;", "\u{22a5}"),
    ("bowtie;", "\u{22c8}"),
    ("boxDL;", "\u{2557}"),
    ("boxDR;", "\u{2554}"),
    ("boxDl;", "\u{2556}"),
    ("boxDr;", "\u{2553}"),
    ("boxH;", "\u{2550}"),
    ("boxHD;", "\u{2566}"),
    ("boxHU;", "\u{2569}"),
    ("boxHd;", "\u{2564}"),
    ("boxHu;", "\u{2567}"),
    ("boxUL;", "\u{255d}"),
    ("boxUR;", "\u{255a}"),
    ("boxUl;", "\u{255c}"),
    ("boxUr;", "\u{2559}"),
    ("boxV;", "\u{2551}"),
    ("boxVH;", "\u{256c}"),
    ("boxVL;", "\u{2563}"),
    ("boxVR;", "\u{2560}"),
    ("boxVh;", "\u{256b}"),
    ("boxVl;", "\u{2562}"),
    ("boxVr;", "\u{255f}"),
    ("boxbox;", "\u{29c9}"),
    ("boxdL;", "\u{2555}"),
    ("boxdR;", "\u{2552}"),
    ("boxdl;", "\u{2510}"),
    ("boxdr;", "\u{250c}"),
    ("boxh;", "\u{2500}"),
    ("boxhD;", "\u{2565}"),
    ("boxhU;", "\u{2568}"),
    ("boxhd;", "\u{252c}"),
    ("boxhu;", "\u{2534}"),
    ("boxminus;", "\u{229f}"),
    ("boxplus;", "\u{229e}"),
    ("boxtimes;", "\u{22a0}"),
    ("boxuL;", "\u{255b}"),
    ("boxuR;", "\u{2558}"),
    ("boxul;", "\u{2518}"),
    ("boxur;", "\u{2514}"),
    ("boxv;", "\u{2502}"),
    ("boxvH;", "\u{256a}"),
    ("boxvL;", "\u{2561}"),
    ("boxvR;", "\u{255e}"),
    ("boxvh;", "\u{253c}"),
    ("boxvl;", "\u{2524}"),
    ("boxvr;", "\u{251c}"),
    ("bprime;", "\u{2035}"),
    ("breve;", "\u{2d8}"),
    ("brvbar", "\u{a6}"),
    ("brvbar;", "\u{a6}"),
    ("bscr;", "\u{1d4b7}"),
    ("bsemi;", "\u{204f}"),
    ("bsim;", "\u{223d}"),
    ("bsime;", "\u{22cd}"),
    ("bsol;", "\u{5c}"),
    ("bsolb;", "\u{29c5}"),
    ("bsolhsub;", "\u{27c8}"),
    ("bull;", "\u{2022}"),
    ("bullet;", "\u{2022}"),
    ("bump;", "\u{224e}"),
    ("bumpE;", "\u{2aae}"),
    ("bumpe;", "\u{224f}"),
    ("bumpeq;", "\u{224f}"),
    ("cacute;", "\u{107}"),
    ("cap;", "\u{2229}"),
    ("capand;", "\u{2a44}"),
    ("capbrcup;", "\u{2a49}"),
    ("capcap;", "\u{2a4b}"),
    ("capcup;", "\u{2a47}"),
    ("capdot;", "\u{2a40}"),
    ("caps;", "\u{2229}\u{fe00}"),
    ("caret;", "\u{2041}"),
    ("caron;", "\u{2c7}"),
    ("ccaps;", "\u{2a4d}"),
    ("ccaron;", "\u{10d}"),
    ("ccedil", "\u{e7}"),
    ("ccedil;", "\u{e7}"),
    ("ccirc;", "\u{109}"),
    ("ccups;", "\u{2a4c}"),
    ("ccupssm;", "\u{2a50}"),
    ("cdot;", "\u{10b}"),
    ("cedil", "\u{b8}"),
    ("cedil;", "\u{b8}"),
    ("cemptyv;", "\u{29b2}"),
    ("cent", "\u{a2}"),
    ("cent;", "\u{a2}"),
    ("centerdot;", "\u{b7}"),
    ("cfr;", "\u{1d520}"),
    ("chcy;", "\u{447}"),
    ("check;", "\u{2713}"),
    ("checkmark;", "\u{2713}"),
    ("chi;", "\u{3c7}"),
    ("cir;", "\u{25cb}"),
    ("cirE;", "\u{29c3}"),
    ("circ;", "\u{2c6}"),
    ("circeq;", "\u{2257}"),
    ("circlearrowleft;", "\u{21ba}"),
    ("circlearrowright;", "\u{21bb}"),
    ("circledR;", "\u{ae}"),
    ("circledS;", "\u{24c8}"),
    ("circledast;", "\u{229b}"),
    ("circledcirc;", "\u{229a}"),
    ("circleddash;", "\u{229d}"),
    ("cire;", "\u{2257}"),
    ("cirfnint;", "\u{2a10}"),
    ("cirmid;", "\u{2aef}"),
    ("cirscir;", "\u{29c2}"),
    ("clubs;", "\u{2663}"),
    ("clubsuit;", "\u{2663}"),
    ("colon;", "\u{3a}"),
    ("colone;", "\u{2254}"),
    ("coloneq;", "\u{2254}"),
    ("comma;", "\u{2c}"),
    ("commat;", "\u{40}"),
    ("comp;", "\u{2201}"),
    ("compfn;", "\u{2218}"),
    ("complement;", "\u{2201}"),
    ("complexes;", "\u{2102}"),
    ("cong;", "\u{2245}"),
    ("congdot;", "\u{2a6d}"),
    ("conint;", "\u{222e}"),
    ("copf;", "\u{1d554}"),
    ("coprod;", "\u{2210}"),
    ("copy", "\u{a9}"),
    ("copy;", "\u{a9}"),
    ("copysr;", "\u{2117}"),
    ("crarr;", "\u{21b5}"),
    ("cross;", "\u{2717}"),
    ("cscr;", "\u{1d4b8}"),
    ("csub;", "\u{2acf}"),
    ("csube;", "\u{2ad1}"),
    ("csup;", "\u{2ad0}"),
    ("csupe;", "\u{2ad2}"),
    ("ctdot;", "\u{22ef}"),
    ("cudarrl;", "\u{2938}"),
    ("cudarrr;", "\u{2935}"),
    ("cuepr;", "\u{22de}"),
    ("cuesc;", "\u{22df}"),
    ("cularr;", "\u{21b6}"),
    ("cularrp;", "\u{293d}"),
    ("cup;", "\u{222a}"),
    ("cupbrcap;", "\u{2a48}"),
    ("cupcap;", "\u{2a46}"),
    ("cupcup;", "\u{2a4a}"),
    ("cupdot;", "\u{228d}"),
    ("cupor;", "\u{2a45}"),
    ("cups;", "\u{222a}\u{fe00}"),
    ("curarr;", "\u{21b7}"),
    ("curarrm;", "\u{293c}"),
    ("curlyeqprec;", "\u{22de}"),
    ("curlyeqsucc;", "\u{22df}"),
    ("curlyvee;", "\u{22ce}"),
    ("curlywedge;", "\u{22cf}"),
    ("curren", "\u{a4}"),
    ("curren;", "\u{a4}"),
    ("curvearrowleft;", "\u{21b6}"),
    ("curvearrowright;", "\u{21b7}"),
    ("cuvee;", "\u{22ce}"),
    ("cuwed;", "\u{22cf}"),
    ("cwconint;", "\u{2232}"),
    ("cwint;", "\u{2231}"),
    ("cylcty;", "\u{232d}"),
    ("dArr;", "\u{21d3}"),
    ("dHar;", "\u{2965}"),
    ("dagger;", "\u{2020}"),
    ("daleth;", "\u{2138}"),
    ("darr;", "\u{2193}"),
    ("dash;", "\u{2010}"),
    ("dashv;", "\u{22a3}"),
    ("dbkarow;", "\u{290f}"),
    ("dblac;", "\u{2dd}"),
    ("dcaron;", "\u{10f}"),
    ("dcy;", "\u{434}"),
    ("dd;", "\u{2146}"),
    ("ddagger;", "\u{2021}"),
    ("ddarr;", "\u{21ca}"),
    ("ddotseq;", "\u{2a77}"),
    ("deg", "\u{b0}"),
    ("deg;", "\u{b0}"),
    ("delta;", "\u{3b4}"),
    ("demptyv;", "\u{29b1}"),
    ("dfisht;", "\u{297f}"),
    ("dfr;", "\u{1d521}"),
    ("dharl;", "\u{21c3}"),
    ("dharr;", "\u{21c2}"),
    ("diam;", "\u{22c4}"),
    ("diamond;", "\u{22c4}"),
    ("diamondsuit;", "\u{2666}"),
    ("diams;", "\u{2666}"),
    ("die;", "\u{a8}"),
    ("digamma;", "\u{3dd}"),
    ("disin;", "\u{22f2}"),
    ("div;", "\u{f7}"),
    ("divide", "\u{f7}"),
    ("divide;", "\u{f7}"),
    ("divideontimes;", "\u{22c7}"),
    ("divonx;", "\u{22c7}"),
    ("djcy;", "\u{452}"),
    ("dlcorn;", "\u{231e}"),
    ("dlcrop;", "\u{230d}"),
    ("dollar;", "\u{24}"),
    ("dopf;", "\u{1d555}"),
    ("dot;", "\u{2d9}"),
    ("doteq;", "\u{2250}"),
    ("doteqdot;", "\u{2251}"),
    ("dotminus;", "\u{2238}"),
    ("dotplus;", "\u{2214}"),
    ("dotsquare;", "\u{22a1}"),
    ("doublebarwedge;", "\u{2306}"),
    ("downarrow;", "\u{2193}"),
    ("downdownarrows;", "\u{21ca}"),
    ("downharpoonleft;", "\u{21c3}"),
    ("downharpoonright;", "\u{21c2}"),
    ("drbkarow;", "\u{2910}"),
    ("drcorn;", "\u{231f}"),
    ("drcrop;", "\u{230c}"),
    ("dscr;", "\u{1d4b9}"),
    ("dscy;", "\u{455}"),
    ("dsol;", "\u{29f6}"),
    ("dstrok;", "\u{111}"),
    ("dtdot;", "\u{22f1}"),
    ("dtri;", "\u{25bf}"),
    ("dtrif;", "\u{25be}"),
    ("duarr;", "\u{21f5}"),
    ("duhar;", "\u{296f}"),
    ("dwangle;", "\u{29a6}"),
    ("dzcy;", "\u{45f}"),
    ("dzigrarr;", "\u{27ff}"),
    ("eDDot;", "\u{2a77}"),
    ("eDot;", "\u{2251}"),
    ("eacute", "\u{e9}"),
    ("eacute;", "\u{e9}"),
    ("easter;", "\u{2a6e}"),
    ("ecaron;", "\u{11b}"),
    ("ecir;", "\u{2256}"),
    ("ecirc", "\u{ea}"),
    ("ecirc;", "\u{ea}"),
    ("ecolon;", "\u{2255}"),
    ("ecy;", "\u{44d}"),
    ("edot;", "\u{117}"),
    ("ee;", "\u{2147}"),
    ("efDot;", "\u{2252}"),
    ("efr;", "\u{1d522}"),
    ("eg;", "\u{2a9a}"),
    ("egrave", "\u{e8}"),
    ("egrave;", "\u{e8}"),
    ("egs;", "\u{2a96}"),
    ("egsdot;", "\u{2a98}"),
    ("el;", "\u{2a99}"),
    ("elinters;", "\u{23e7}"),
    ("ell;", "\u{2113}"),
    ("els;", "\u{2a95}"),
    ("elsdot;", "\u{2a97}"),
    ("emacr;", "\u{113}"),
    ("empty;", "\u{2205}"),
    ("emptyset;", "\u{2205}"),
    ("emptyv;", "\u{2205}"),
    ("emsp13;", "\u{2004}"),
    ("emsp14;", "\u{2005}"),
    ("emsp;", "\u{2003}"),
    ("eng;", "\u{14b}"),
    ("ensp;", "\u{2002}"),
    ("eogon;", "\u{119}"),
    ("eopf;", "\u{1d556}"),
    ("epar;", "\u{22d5}"),
    ("eparsl;", "\u{29e3}"),
    ("eplus;", "\u{2a71}"),
    ("epsi;", "\u{3b5}"),
    ("epsilon;", "\u{3b5}"),
    ("epsiv;", "\u{3f5}"),
    ("eqcirc;", "\u{2256}"),
    ("eqcolon;", "\u{2255}"),
    ("eqsim;", "\u{2242}"),
    ("eqslantgtr;", "\u{2a96}"),
    ("eqslantless;", "\u{2a95}"),
    ("equals;", "\u{3d}"),
    ("equest;", "\u{225f}"),
    ("equiv;", "\u{2261}"),
    ("equivDD;", "\u{2a78}"),
    ("eqvparsl;", "\u{29e5}"),
    ("erDot;", "\u{2253}"),
    ("erarr;", "\u{2971}"),
    ("escr;", "\u{212f}"),
    ("esdot;", "\u{2250}"),
    ("esim;", "\u{2242}"),
    ("eta;", "\u{3b7}"),
    ("eth", "\u{f0}"),
    ("eth;", "\u{f0}"),
    ("euml", "\u{eb}"),
    ("euml;", "\u{eb}"),
    ("euro;", "\u{20ac}"),
    ("excl;", "\u{21}"),
    ("exist;", "\u{2203}"),
    ("expectation;", "\u{2130}"),
    ("exponentiale;", "\u{2147}"),
    ("fallingdotseq;", "\u{2252}"),
    ("fcy;", "\u{444}"),
    ("female;", "\u{2640}"),
    ("ffilig;", "\u{fb03}"),
    ("fflig;", "\u{fb00}"),
    ("ffllig;", "\u{fb04}"),
    ("ffr;", "\u{1d523}"),
    ("filig;", "\u{fb01}"),
    ("fjlig;", "\u{66}\u{6a}"),
    ("flat;", "\u{266d}"),
    ("fllig;", "\u{fb02}"),
    ("fltns;", "\u{25b1}"),
    ("fnof;", "\u{192}"),
    ("fopf;", "\u{1d557}"),
    ("forall;", "\u{2200}"),
    ("fork;", "\u{22d4}"),
    ("forkv;", "\u{2ad9}"),
    ("fpartint;", "\u{2a0d}"),
    ("frac12", "\u{bd}"),
    ("frac12;", "\u{bd}"),
    ("frac13;", "\u{2153}"),
    ("frac14", "\u{bc}"),
    ("frac14;", "\u{bc}"),
    ("frac15;", "\u{2155}"),
    ("frac16;", "\u{2159}"),
    ("frac18;", "\u{215b}"),
    ("frac23;", "\u{2154}"),
    ("frac25;", "\u{2156}"),
    ("frac34", "\u{be}"),
    ("frac34;", "\u{be}"),
    ("frac35;", "\u{2157}"),
    ("frac38;", "\u{215c}"),
    ("frac45;", "\u{2158}"),
    ("frac56;", "\u{215a}"),
    ("frac58;", "\u{215d}"),
    ("frac78;", "\u{215e}"),
    ("frasl;", "\u{2044}"),
    ("frown;", "\u{2322}"),
    ("fscr;", "\u{1d4bb}"),
    ("gE;", "\u{2267}"),
    ("gEl;", "\u{2a8c}"),
    ("gacute;", "\u{1f5}"),
    ("gamma;", "\u{3b3}"),
    ("gammad;", "\u{3dd}"),
    ("gap;", "\u{2a86}"),
    ("gbreve;", "\u{11f}"),
    ("gcirc;", "\u{11d}"),
    ("gcy;", "\u{433}"),
    ("gdot;", "\u{121}"),
    ("ge;", "\u{2265}"),
    ("gel;", "\u{22db}"),
    ("geq;", "\u{2265}"),
    ("geqq;", "\u{2267}"),
    ("geqslant;", "\u{2a7e}"),
    ("ges;", "\u{2a7e}"),
    ("gescc;", "\u{2aa9}"),
    ("gesdot;", "\u{2a80}"),
    ("gesdoto;", "\u{2a82}"),
    ("gesdotol;", "\u{2a84}"),
    ("gesl;", "\u{22db}\u{fe00}"),
    ("gesles;", "\u{2a94}"),
    ("gfr;", "\u{1d524}"),
    ("gg;", "\u{226b}"),
    ("ggg;", "\u{22d9}"),
    ("gimel;", "\u{2137}"),
    ("gjcy;", "\u{453}"),
    ("gl;", "\u{2277}"),
    ("glE;", "\u{2a92}"),
    ("gla;", "\u{2aa5}"),
    ("glj;", "\u{2aa4}"),
    ("gnE;", "\u{2269}"),
    ("gnap;", "\u{2a8a}"),
    ("gnapprox;", "\u{2a8a}"),
    ("gne;", "\u{2a88}"),
    ("gneq;", "\u{2a88}"),
    ("gneqq;", "\u{2269}"),
    ("gnsim;", "\u{22e7}"),
    ("gopf;", "\u{1d558}"),
    ("grave;", "\u{60}"),
    ("gscr;", "\u{210a}"),
    ("gsim;", "\u{2273}"),
    ("gsime;", "\u{2a8e}"),
    ("gsiml;", "\u{2a90}"),
    ("gt", "\u{3e}"),
    ("gt;", "\u{3e}"),
    ("gtcc;", "\u{2aa7}"),
    ("gtcir;", "\u{2a7a}"),
    ("gtdot;", "\u{22d7}"),
    ("gtlPar;", "\u{2995}"),
    ("gtquest;", "\u{2a7c}"),
    ("gtrapprox;", "\u{2a86}"),
    ("gtrarr;", "\u{2978}"),
    ("gtrdot;", "\u{22d7}"),
    ("gtreqless;", "\u{22db}"),
    ("gtreqqless;", "\u{2a8c}"),
    ("gtrless;", "\u{2277}"),
    ("gtrsim;", "\u{2273}"),
    ("gvertneqq;", "\u{2269}\u{fe00}"),
    ("gvnE;", "\u{2269}\u{fe00}"),
    ("hArr;", "\u{21d4}"),
    ("hairsp;", "\u{200a}"),
    ("half;", "\u{bd}"),
    ("hamilt;", "\u{210b}"),
    ("hardcy;", "\u{44a}"),
    ("harr;", "\u{2194}"),
    ("harrcir;", "\u{2948}"),
    ("harrw;", "\u{21ad}"),
    ("hbar;", "\u{210f}"),
    ("hcirc;", "\u{125}"),
    ("hearts;", "\u{2665}"),
    ("heartsuit;", "\u{2665}"),
    ("hellip;", "\u{2026}"),
    ("hercon;", "\u{22b9}"),
    ("hfr;", "\u{1d525}"),
    ("hksearow;", "\u{2925}"),
    ("hkswarow;", "\u{2926}"),
    ("hoarr;", "\u{21ff}"),
    ("homtht;", "\u{223b}"),
    ("hookleftarrow;", "\u{21a9}"),
    ("hookrightarrow;", "\u{21aa}"),
    ("hopf;", "\u{1d559}"),
    ("horbar;", "\u{2015}"),
    ("hscr;", "\u{1d4bd}"),
    ("hslash;", "\u{210f}"),
    ("hstrok;", "\u{127}"),
    ("hybull;", "\u{2043}"),
    ("hyphen;", "\u{2010}"),
    ("iacute", "\u{ed}"),
    ("iacute;", "\u{ed}"),
    ("ic;", "\u{2063}"),
    ("icirc", "\u{ee}"),
    ("icirc;", "\u{ee}"),
    ("icy;", "\u{438}"),
    ("iecy;", "\u{435}"),
    ("iexcl", "\u{a1}"),
    ("iexcl;", "\u{a1}"),
    ("iff;", "\u{21d4}"),
    ("ifr;", "\u{1d526}"),
    ("igrave", "\u{ec}"),
    ("igrave;", "\u{ec}"),
    ("ii;", "\u{2148}"),
    ("iiiint;", "\u{2a0c}"),
    ("iiint;", "\u{222d}"),
    ("iinfin;", "\u{29dc}"),
    ("iiota;", "\u{2129}"),
    ("ijlig;", "\u{133}"),
    ("imacr;", "\u{12b}"),
    ("image;", "\u{2111}"),
    ("imagline;", "\u{2110}"),
    ("imagpart;", "\u{2111}"),
    ("imath;", "\u{131}"),
    ("imof;", "\u{22b7}"),
    ("imped;", "\u{1b5}"),
    ("in;", "\u{2208}"),
    ("incare;", "\u{2105}"),
    ("infin;", "\u{221e}"),
    ("infintie;", "\u{29dd}"),
    ("inodot;", "\u{131}"),
    ("int;", "\u{222b}"),
    ("intcal;", "\u{22ba}"),
    ("integers;", "\u{2124}"),
    ("intercal;", "\u{22ba}"),
    ("intlarhk;", "\u{2a17}"),
    ("intprod;", "\u{2a3c}"),
    ("iocy;", "\u{451}"),
    ("iogon;", "\u{12f}"),
    ("iopf;", "\u{1d55a}"),
    ("iota;", "\u{3b9}"),
    ("iprod;", "\u{2a3c}"),
    ("iquest", "\u{bf}"),
    ("iquest;", "\u{bf}"),
    ("iscr;", "\u{1d4be}"),
    ("isin;", "\u{2208}"),
    ("isinE;", "\u{22f9}"),
    ("isindot;", "\u{22f5}"),
    ("isins;", "\u{22f4}"),
    ("isinsv;", "\u{22f3}"),
    ("isinv;", "\u{2208}"),
    ("it;", "\u{2062}"),
    ("itilde;", "\u{129}"),
    ("iukcy;", "\u{456}"),
    ("iuml", "\u{ef}"),
    ("iuml;", "\u{ef}"),
    ("jcirc;", "\u{135}"),
    ("jcy;", "\u{439}"),
    ("jfr;", "\u{1d527}"),
    ("jmath;", "\u{237}"),
    ("jopf;", "\u{1d55b}"),
    ("jscr;", "\u{1d4bf}"),
    ("jsercy;", "\u{458}"),
    ("jukcy;", "\u{454}"),
    ("kappa;", "\u{3ba}"),
    ("kappav;", "\u{3f0}"),
    ("kcedil;", "\u{137}"),
    ("kcy;", "\u{43a}"),
    ("kfr;", "\u{1d528}"),
    ("kgreen;", "\u{138}"),
    ("khcy;", "\u{445}"),
    ("kjcy;", "\u{45c}"),
    ("kopf;", "\u{1d55c}"),
    ("kscr;", "\u{1d4c0}"),
    ("lAarr;", "\u{21da}"),
    ("lArr;", "\u{21d0}"),
    ("lAtail;", "\u{291b}"),
    ("lBarr;", "\u{290e}"),
    ("lE;", "\u{2266}"),
    ("lEg;", "\u{2a8b}"),
    ("lHar;", "\u{2962}"),
    ("lacute;", "\u{13a}"),
    ("laemptyv;", "\u{29b4}"),
    ("lagran;", "\u{2112}"),
    ("lambda;", "\u{3bb}"),
    ("lang;", "\u{27e8}"),
    ("langd;", "\u{2991}"),
    ("langle;", "\u{27e8}"),
    ("lap;", "\u{2a85}"),
    ("laquo", "\u{ab}"),
    ("laquo;", "\u{ab}"),
    ("larr;", "\u{2190}"),
    ("larrb;", "\u{21e4}"),
    ("larrbfs;", "\u{291f}"),
    ("larrfs;", "\u{291d}"),
    ("larrhk;", "\u{21a9}"),
    ("larrlp;", "\u{21ab}"),
    ("larrpl;", "\u{2939}"),
    ("larrsim;", "\u{2973}"),
    ("larrtl;", "\u{21a2}"),
    ("lat;", "\u{2aab}"),
    ("latail;", "\u{2919}"),
    ("late;", "\u{2aad}"),
    ("lates;", "\u{2aad}\u{fe00}"),
    ("lbarr;", "\u{290c}"),
    ("lbbrk;", "\u{2772}"),
    ("lbrace;", "\u{7b}"),
    ("lbrack;", "\u{5b}"),
    ("lbrke;", "\u{298b}"),
    ("lbrksld;", "\u{298f}"),
    ("lbrkslu;", "\u{298d}"),
    ("lcaron;", "\u{13e}"),
    ("lcedil;", "\u{13c}"),
    ("lceil;", "\u{2308}"),
    ("lcub;", "\u{7b}"),
    ("lcy;", "\u{43b}"),
    ("ldca;", "\u{2936}"),
    ("ldquo;", "\u{201c}"),
    ("ldquor;", "\u{201e}"),
    ("ldrdhar;", "\u{2967}"),
    ("ldrushar;", "\u{294b}"),
    ("ldsh;", "\u{21b2}"),
    ("le;", "\u{2264}"),
    ("leftarrow;", "\u{2190}"),
    ("leftarrowtail;", "\u{21a2}"),
    ("leftharpoondown;", "\u{21bd}"),
    ("leftharpoonup;", "\u{21bc}"),
    ("leftleftarrows;", "\u{21c7}"),
    ("leftrightarrow;", "\u{2194}"),
    ("leftrightarrows;", "\u{21c6}"),
    ("leftrightharpoons;", "\u{21cb}"),
    ("leftrightsquigarrow;", "\u{21ad}"),
    ("leftthreetimes;", "\u{22cb}"),
    ("leg;", "\u{22da}"),
    ("leq;", "\u{2264}"),
    ("leqq;", "\u{2266}"),
    ("leqslant;", "\u{2a7d}"),
    ("les;", "\u{2a7d}"),
    ("lescc;", "\u{2aa8}"),
    ("lesdot;", "\u{2a7f}"),
    ("lesdoto;", "\u{2a81}"),
    ("lesdotor;", "\u{2a83}"),
    ("lesg;", "\u{22da}\u{fe00}"),
    ("lesges;", "\u{2a93}"),
    ("lessapprox;", "\u{2a85}"),
    ("lessdot;", "\u{22d6}"),
    ("lesseqgtr;", "\u{22da}"),
    ("lesseqqgtr;", "\u{2a8b}"),
    ("lessgtr;", "\u{2276}"),
    ("lesssim;", "\u{2272}"),
    ("lfisht;", "\u{297c}"),
    ("lfloor;", "\u{230a}"),
    ("lfr;", "\u{1d529}"),
    ("lg;", "\u{2276}"),
    ("lgE;", "\u{2a91}"),
    ("lhard;", "\u{21bd}"),
    ("lharu;", "\u{21bc}"),
    ("lharul;", "\u{296a}"),
    ("lhblk;", "\u{2584}"),
    ("ljcy;", "\u{459}"),
    ("ll;", "\u{226a}"),
    ("llarr;", "\u{21c7}"),
    ("llcorner;", "\u{231e}"),
    ("llhard;", "\u{296b}"),
    ("lltri;", "\u{25fa}"),
    ("lmidot;", "\u{140}"),
    ("lmoust;", "\u{23b0}"),
    ("lmoustache;", "\u{23b0}"),
    ("lnE;", "\u{2268}"),
    ("lnap;", "\u{2a89}"),
    ("lnapprox;", "\u{2a89}"),
    ("lne;", "\u{2a87}"),
    ("lneq;", "\u{2a87}"),
    ("lneqq;", "\u{2268}"),
    ("lnsim;", "\u{22e6}"),
    ("loang;", "\u{27ec}"),
    ("loarr;", "\u{21fd}"),
    ("lobrk;", "\u{27e6}"),
    ("longleftarrow;", "\u{27f5}"),
    ("longleftrightarrow;", "\u{27f7}"),
    ("longmapsto;", "\u{27fc}"),
    ("longrightarrow;", "\u{27f6}"),
    ("looparrowleft;", "\u{21ab}"),
    ("looparrowright;", "\u{21ac}"),
    ("lopar;", "\u{2985}"),
    ("lopf;", "\u{1d55d}"),
    ("loplus;", "\u{2a2d}"),
    ("lotimes;", "\u{2a34}"),
    ("lowast;", "\u{2217}"),
    ("lowbar;", "\u{5f}"),
    ("loz;", "\u{25ca}"),
    ("lozenge;", "\u{25ca}"),
    ("lozf;", "\u{29eb}"),
    ("lpar;", "\u{28}"),
    ("lparlt;", "\u{2993}"),
    ("lrarr;", "\u{21c6}"),
    ("lrcorner;", "\u{231f}"),
    ("lrhar;", "\u{21cb}"),
    ("lrhard;", "\u{296d}"),
    ("lrm;", "\u{200e}"),
    ("lrtri;", "\u{22bf}"),
    ("lsaquo;", "\u{2039}"),
    ("lscr;", "\u{1d4c1}"),
    ("lsh;", "\u{21b0}"),
    ("lsim;", "\u{2272}"),
    ("lsime;", "\u{2a8d}"),
    ("lsimg;", "\u{2a8f}"),
    ("lsqb;", "\u{5b}"),
    ("lsquo;", "\u{2018}"),
    ("lsquor;", "\u{201a}"),
    ("lstrok;", "\u{142}"),
    ("lt", "\u{3c}"),
    ("lt;", "\u{3c}"),
    ("ltcc;", "\u{2aa6}"),
    ("ltcir;", "\u{2a79}"),
    ("ltdot;", "\u{22d6}"),
    ("lthree;", "\u{22cb}"),
    ("ltimes;", "\u{22c9}"),
    ("ltlarr;", "\u{2976}"),
    ("ltquest;", "\u{2a7b}"),
    ("ltrPar;", "\u{2996}"),
    ("ltri;", "\u{25c3}"),
    ("ltrie;", "\u{22b4}"),
    ("ltrif;", "\u{25c2}"),
    ("lurdshar;", "\u{294a}"),
    ("luruhar;", "\u{2966}"),
    ("lvertneqq;", "\u{2268}\u{fe00}"),
    ("lvnE;", "\u{2268}\u{fe00}"),
    ("mDDot;", "\u{223a}"),
    ("macr", "\u{af}"),
    ("macr;", "\u{af}"),
    ("male;", "\u{2642}"),
    ("malt;", "\u{2720}"),
    ("maltese;", "\u{2720}"),
    ("map;", "\u{21a6}"),
    ("mapsto;", "\u{21a6}"),
    ("mapstodown;", "\u{21a7}"),
    ("mapstoleft;", "\u{21a4}"),
    ("mapstoup;", "\u{21a5}"),
    ("marker;", "\u{25ae}"),
    ("mcomma;", "\u{2a29}"),
    ("mcy;", "\u{43c}"),
    ("mdash;", "\u{2014}"),
    ("measuredangle;", "\u{2221}"),
    ("mfr;", "\u{1d52a}"),
    ("mho;", "\u{2127}"),
    ("micro", "\u{b5}"),
    ("micro;", "\u{b5}"),
    ("mid;", "\u{2223}"),
    ("midast;", "\u{2a}"),
    ("midcir;", "\u{2af0}"),
    ("middot", "\u{b7}"),
    ("middot;", "\u{b7}"),
    ("minus;", "\u{2212}"),
    ("minusb;", "\u{229f}"),
    ("minusd;", "\u{2238}"),
    ("minusdu;", "\u{2a2a}"),
    ("mlcp;", "\u{2adb}"),
    ("mldr;", "\u{2026}"),
    ("mnplus;", "\u{2213}"),
    ("models;", "\u{22a7}"),
    ("mopf;", "\u{1d55e}"),
    ("mp;", "\u{2213}"),
    ("mscr;", "\u{1d4c2}"),
    ("mstpos;", "\u{223e}"),
    ("mu;", "\u{3bc}"),
    ("multimap;", "\u{22b8}"),
    ("mumap;", "\u{22b8}"),
    ("nGg;", "\u{22d9}\u{338}"),
    ("nGt;", "\u{226b}\u{20d2}"),
    ("nGtv;", "\u{226b}\u{338}"),
    ("nLeftarrow;", "\u{21cd}"),
    ("nLeftrightarrow;", "\u{21ce}"),
    ("nLl;", "\u{22d8}\u{338}"),
    ("nLt;", "\u{226a}\u{20d2}"),
    ("nLtv;", "\u{226a}\u{338}"),
    ("nRightarrow;", "\u{21cf}"),
    ("nVDash;", "\u{22af}"),
    ("nVdash;", "\u{22ae}"),
    ("nabla;", "\u{2207}"),
    ("nacute;", "\u{144}"),
    ("nang;", "\u{2220}\u{20d2}"),
    ("nap;", "\u{2249}"),
    ("napE;", "\u{2a70}\u{338}"),
    ("napid;", "\u{224b}\u{338}"),
    ("napos;", "\u{149}"),
    ("napprox;", "\u{2249}"),
    ("natur;", "\u{266e}"),
    ("natural;", "\u{266e}"),
    ("naturals;", "\u{2115}"),
    ("nbsp", "\u{a0}"),
    ("nbsp;", "\u{a0}"),
    ("nbump;", "\u{224e}\u{338}"),
    ("nbumpe;", "\u{224f}\u{338}"),
    ("ncap;", "\u{2a43}"),
    ("ncaron;", "\u{148}"),
    ("ncedil;", "\u{146}"),
    ("ncong;", "\u{2247}"),
    ("ncongdot;", "\u{2a6d}\u{338}"),
    ("ncup;", "\u{2a42}"),
    ("ncy;", "\u{43d}"),
    ("ndash;", "\u{2013}"),
    ("ne;", "\u{2260}"),
    ("neArr;", "\u{21d7}"),
    ("nearhk;", "\u{2924}"),
    ("nearr;", "\u{2197}"),
    ("nearrow;", "\u{2197}"),
    ("nedot;", "\u{2250}\u{338}"),
    ("nequiv;", "\u{2262}"),
    ("nesear;", "\u{2928}"),
    ("nesim;", "\u{2242}\u{338}"),
    ("nexist;", "\u{2204}"),
    ("nexists;", "\u{2204}"),
    ("nfr;", "\u{1d52b}"),
    ("ngE;", "\u{2267}\u{338}"),
    ("nge;", "\u{2271}"),
    ("ngeq;", "\u{2271}"),
    ("ngeqq;", "\u{2267}\u{338}"),
    ("ngeqslant;", "\u{2a7e}\u{338}"),
    ("nges;", "\u{2a7e}\u{338}"),
    ("ngsim;", "\u{2275}"),
    ("ngt;", "\u{226f}"),
    ("ngtr;", "\u{226f}"),
    ("nhArr;", "\u{21ce}"),
    ("nharr;", "\u{21ae}"),
    ("nhpar;", "\u{2af2}"),
    ("ni;", "\u{220b}"),
    ("nis;", "\u{22fc}"),
    ("nisd;", "\u{22fa}"),
    ("niv;", "\u{220b}"),
    ("njcy;", "\u{45a}"),
    ("nlArr;", "\u{21cd}"),
    ("nlE;", "\u{2266}\u{338}"),
    ("nlarr;", "\u{219a}"),
    ("nldr;", "\u{2025}"),
    ("nle;", "\u{2270}"),
    ("nleftarrow;", "\u{219a}"),
    ("nleftrightarrow;", "\u{21ae}"),
    ("nleq;", "\u{2270}"),
    ("nleqq;", "\u{2266}\u{338}"),
    ("nleqslant;", "\u{2a7d}\u{338}"),
    ("nles;", "\u{2a7d}\u{338}"),
    ("nless;", "\u{226e}"),
    ("nlsim;", "\u{2274}"),
    ("nlt;", "\u{226e}"),
    ("nltri;", "\u{22ea}"),
    ("nltrie;", "\u{22ec}"),
    ("nmid;", "\u{2224}"),
    ("nopf;", "\u{1d55f}"),
    ("not", "\u{ac}"),
    ("not;", "\u{ac}"),
    ("notin;", "\u{2209}"),
    ("notinE;", "\u{22f9}\u{338}"),
    ("notindot;", "\u{22f5}\u{338}"),
    ("notinva;", "\u{2209}"),
    ("notinvb;", "\u{22f7}"),
    ("notinvc;", "\u{22f6}"),
    ("notni;", "\u{220c}"),
    ("notniva;", "\u{220c}"),
    ("notnivb;", "\u{22fe}"),
    ("notnivc;", "\u{22fd}"),
    ("npar;", "\u{2226}"),
    ("nparallel;", "\u{2226}"),
    ("nparsl;", "\u{2afd}\u{20e5}"),
    ("npart;", "\u{2202}\u{338}"),
    ("npolint;", "\u{2a14}"),
    ("npr;", "\u{2280}"),
    ("nprcue;", "\u{22e0}"),
    ("npre;", "\u{2aaf}\u{338}"),
    ("nprec;", "\u{2280}"),
    ("npreceq;", "\u{2aaf}\u{338}"),
    ("nrArr;", "\u{21cf}"),
    ("nrarr;", "\u{219b}"),
    ("nrarrc;", "\u{2933}\u{338}"),
    ("nrarrw;", "\u{219d}\u{338}"),
    ("nrightarrow;", "\u{219b}"),
    ("nrtri;", "\u{22eb}"),
    ("nrtrie;", "\u{22ed}"),
    ("nsc;", "\u{2281}"),
    ("nsccue;", "\u{22e1}"),
    ("nsce;", "\u{2ab0}\u{338}"),
    ("nscr;", "\u{1d4c3}"),
    ("nshortmid;", "\u{2224}"),
    ("nshortparallel;", "\u{2226}"),
    ("nsim;", "\u{2241}"),
    ("nsime;", "\u{2244}"),
    ("nsimeq;", "\u{2244}"),
    ("nsmid;", "\u{2224}"),
    ("nspar;", "\u{2226}"),
    ("nsqsube;", "\u{22e2}"),
    ("nsqsupe;", "\u{22e3}"),
    ("nsub;", "\u{2284}"),
    ("nsubE;", "\u{2ac5}\u{338}"),
    ("nsube;", "\u{2288}"),
    ("nsubset;", "\u{2282}\u{20d2}"),
    ("nsubseteq;", "\u{2288}"),
    ("nsubseteqq;", "\u{2ac5}\u{338}"),
    ("nsucc;", "\u{2281}"),
    ("nsucceq;", "\u{2ab0}\u{338}"),
    ("nsup;", "\u{2285}"),
    ("nsupE;", "\u{2ac6}\u{338}"),
    ("nsupe;", "\u{2289}"),
    ("nsupset;", "\u{2283}\u{20d2}"),
    ("nsupseteq;", "\u{2289}"),
    ("nsupseteqq;", "\u{2ac6}\u{338}"),
    ("ntgl;", "\u{2279}"),
    ("ntilde", "\u{f1}"),
    ("ntilde;", "\u{f1}"),
    ("ntlg;", "\u{2278}"),
    ("ntriangleleft;", "\u{22ea}"),
    ("ntrianglelefteq;", "\u{22ec}"),
    ("ntriangleright;", "\u{22eb}"),
    ("ntrianglerighteq;", "\u{22ed}"),
    ("nu;", "\u{3bd}"),
    ("num;", "\u{23}"),
    ("numero;", "\u{2116}"),
    ("numsp;", "\u{2007}"),
    ("nvDash;", "\u{22ad}"),
    ("nvHarr;", "\u{2904}"),
    ("nvap;", "\u{224d}\u{20d2}"),
    ("nvdash;", "\u{22ac}"),
    ("nvge;", "\u{2265}\u{20d2}"),
    ("nvgt;", "\u{3e}\u{20d2}"),
    ("nvinfin;", "\u{29de}"),
    ("nvlArr;", "\u{2902}"),
    ("nvle;", "\u{2264}\u{20d2}"),
    ("nvlt;", "\u{3c}\u{20d2}"),
    ("nvltrie;", "\u{22b4}\u{20d2}"),
    ("nvrArr;", "\u{2903}"),
    ("nvrtrie;", "\u{22b5}\u{20d2}"),
    ("nvsim;", "\u{223c}\u{20d2}"),
    ("nwArr;", "\u{21d6}"),
    ("nwarhk;", "\u{2923}"),
    ("nwarr;", "\u{2196}"),
    ("nwarrow;", "\u{2196}"),
    ("nwnear;", "\u{2927}"),
    ("oS;", "\u{24c8}"),
    ("oacute", "\u{f3}"),
    ("oacute;", "\u{f3}"),
    ("oast;", "\u{229b}"),
    ("ocir;", "\u{229a}"),
    ("ocirc", "\u{f4}"),
    ("ocirc;", "\u{f4}"),
    ("ocy;", "\u{43e}"),
    ("odash;", "\u{229d}"),
    ("odblac;", "\u{151}"),
    ("odiv;", "\u{2a38}"),
    ("odot;", "\u{2299}"),
    ("odsold;", "\u{29bc}"),
    ("oelig;", "\u{153}"),
    ("ofcir;", "\u{29bf}"),
    ("ofr;", "\u{1d52c}"),
    ("ogon;", "\u{2db}"),
    ("ograve", "\u{f2}"),
    ("ograve;", "\u{f2}"),
    ("ogt;", "\u{29c1}"),
    ("ohbar;", "\u{29b5}"),
    ("ohm;", "\u{3a9}"),
    ("oint;", "\u{222e}"),
    ("olarr;", "\u{21ba}"),
    ("olcir;", "\u{29be}"),
    ("olcross;", "\u{29bb}"),
    ("oline;", "\u{203e}"),
    ("olt;", "\u{29c0}"),
    ("omacr;", "\u{14d}"),
    ("omega;", "\u{3c9}"),
    ("omicron;", "\u{3bf}"),
    ("omid;", "\u{29b6}"),
    ("ominus;", "\u{2296}"),
    ("oopf;", "\u{1d560}"),
    ("opar;", "\u{29b7}"),
    ("operp;", "\u{29b9}"),
    ("oplus;", "\u{2295}"),
    ("or;", "\u{2228}"),
    ("orarr;", "\u{21bb}"),
    ("ord;", "\u{2a5d}"),
    ("order;", "\u{2134}"),
    ("orderof;", "\u{2134}"),
    ("ordf", "\u{aa}"),
    ("ordf;", "\u{aa}"),
    ("ordm", "\u{ba}"),
    ("ordm;", "\u{ba}"),
    ("origof;", "\u{22b6}"),
    ("oror;", "\u{2a56}"),
    ("orslope;", "\u{2a57}"),
    ("orv;", "\u{2a5b}"),
    ("oscr;", "\u{2134}"),
    ("oslash", "\u{f8}"),
    ("oslash;", "\u{f8}"),
    ("osol;", "\u{2298}"),
    ("otilde", "\u{f5}"),
    ("otilde;", "\u{f5}"),
    ("otimes;", "\u{2297}"),
    ("otimesas;", "\u{2a36}"),
    ("ouml", "\u{f6}"),
    ("ouml;", "\u{f6}"),
    ("ovbar;", "\u{233d}"),
    ("par;", "\u{2225}"),
    ("para", "\u{b6}"),
    ("para;", "\u{b6}"),
    ("parallel;", "\u{2225}"),
    ("parsim;", "\u{2af3}"),
    ("parsl;", "\u{2afd}"),
    ("part;", "\u{2202}"),
    ("pcy;", "\u{43f}"),
    ("percnt;", "\u{25}"),
    ("period;", "\u{2e}"),
    ("permil;", "\u{2030}"),
    ("perp;", "\u{22a5}"),
    ("pertenk;", "\u{2031}"),
    ("pfr;", "\u{1d52d}"),
    ("phi;", "\u{3c6}"),
    ("phiv;", "\u{3d5}"),
    ("phmmat;", "\u{2133}"),
    ("phone;", "\u{260e}"),
    ("pi;", "\u{3c0}"),
    ("pitchfork;", "\u{22d4}"),
    ("piv;", "\u{3d6}"),
    ("planck;", "\u{210f}"),
    ("planckh;", "\u{210e}"),
    ("plankv;", "\u{210f}"),
    ("plus;", "\u{2b}"),
    ("plusacir;", "\u{2a23}"),
    ("plusb;", "\u{229e}"),
    ("pluscir;", "\u{2a22}"),
    ("plusdo;", "\u{2214}"),
    ("plusdu;", "\u{2a25}"),
    ("pluse;", "\u{2a72}"),
    ("plusmn", "\u{b1}"),
    ("plusmn;", "\u{b1}"),
    ("plussim;", "\u{2a26}"),
    ("plustwo;", "\u{2a27}"),
    ("pm;", "\u{b1}"),
    ("pointint;", "\u{2a15}"),
    ("popf;", "\u{1d561}"),
    ("pound", "\u{a3}"),
    ("pound;", "\u{a3}"),
    ("pr;", "\u{227a}"),
    ("prE;", "\u{2ab3}"),
    ("prap;", "\u{2ab7}"),
    ("prcue;", "\u{227c}"),
    ("pre;", "\u{2aaf}"),
    ("prec;", "\u{227a}"),
    ("precapprox;", "\u{2ab7}"),
    ("preccurlyeq;", "\u{227c}"),
    ("preceq;", "\u{2aaf}"),
    ("precnapprox;", "\u{2ab9}"),
    ("precneqq;", "\u{2ab5}"),
    ("precnsim;", "\u{22e8}"),
    ("precsim;", "\u{227e}"),
    ("prime;", "\u{2032}"),
    ("primes;", "\u{2119}"),
    ("prnE;", "\u{2ab5}"),
    ("prnap;", "\u{2ab9}"),
    ("prnsim;", "\u{22e8}"),
    ("prod;", "\u{220f}"),
    ("profalar;", "\u{232e}"),
    ("profline;", "\u{2312}"),
    ("profsurf;", "\u{2313}"),
    ("prop;", "\u{221d}"),
    ("propto;", "\u{221d}"),
    ("prsim;", "\u{227e}"),
    ("prurel;", "\u{22b0}"),
    ("pscr;", "\u{1d4c5}"),
    ("psi;", "\u{3c8}"),
    ("puncsp;", "\u{2008}"),
    ("qfr;", "\u{1d52e}"),
    ("qint;", "\u{2a0c}"),
    ("qopf;", "\u{1d562}"),
    ("qprime;", "\u{2057}"),
    ("qscr;", "\u{1d4c6}"),
    ("quaternions;", "\u{210d}"),
    ("quatint;", "\u{2a16}"),
    ("quest;", "\u{3f}"),
    ("questeq;", "\u{225f}"),
    ("quot", "\u{22}"),
    ("quot;", "\u{22}"),
    ("rAarr;", "\u{21db}"),
    ("rArr;", "\u{21d2}"),
    ("rAtail;", "\u{291c}"),
    ("rBarr;", "\u{290f}"),
    ("rHar;", "\u{2964}"),
    ("race;", "\u{223d}\u{331}"),
    ("racute;", "\u{155}"),
    ("radic;", "\u{221a}"),
    ("raemptyv;", "\u{29b3}"),
    ("rang;", "\u{27e9}"),
    ("rangd;", "\u{2992}"),
    ("range;", "\u{29a5}"),
    ("rangle;", "\u{27e9}"),
    ("raquo", "\u{bb}"),
    ("raquo;", "\u{bb}"),
    ("rarr;", "\u{2192}"),
    ("rarrap;", "\u{2975}"),
    ("rarrb;", "\u{21e5}"),
    ("rarrbfs;", "\u{2920}"),
    ("rarrc;", "\u{2933}"),
    ("rarrfs;", "\u{291e}"),
    ("rarrhk;", "\u{21aa}"),
    ("rarrlp;", "\u{21ac}"),
    ("rarrpl;", "\u{2945}"),
    ("rarrsim;", "\u{2974}"),
    ("rarrtl;", "\u{21a3}"),
    ("rarrw;", "\u{219d}"),
    ("ratail;", "\u{291a}"),
    ("ratio;", "\u{2236}"),
    ("rationals;", "\u{211a}"),
    ("rbarr;", "\u{290d}"),
    ("rbbrk;", "\u{2773}"),
    ("rbrace;", "\u{7d}"),
    ("rbrack;", "\u{5d}"),
    ("rbrke;", "\u{298c}"),
    ("rbrksld;", "\u{298e}"),
    ("rbrkslu;", "\u{2990}"),
    ("rcaron;", "\u{159}"),
    ("rcedil;", "\u{157}"),
    ("rceil;", "\u{2309}"),
    ("rcub;", "\u{7d}"),
    ("rcy;", "\u{440}"),
    ("rdca;", "\u{2937}"),
    ("rdldhar;", "\u{2969}"),
    ("rdquo;", "\u{201d}"),
    ("rdquor;", "\u{201d}"),
    ("rdsh;", "\u{21b3}"),
    ("real;", "\u{211c}"),
    ("realine;", "\u{211b}"),
    ("realpart;", "\u{211c}"),
    ("reals;", "\u{211d}"),
    ("rect;", "\u{25ad}"),
    ("reg", "\u{ae}"),
    ("reg;", "\u{ae}"),
    ("rfisht;", "\u{297d}"),
    ("rfloor;", "\u{230b}"),
    ("rfr;", "\u{1d52f}"),
    ("rhard;", "\u{21c1}"),
    ("rharu;", "\u{21c0}"),
    ("rharul;", "\u{296c}"),
    ("rho;", "\u{3c1}"),
    ("rhov;", "\u{3f1}"),
    ("rightarrow;", "\u{2192}"),
    ("rightarrowtail;", "\u{21a3}"),
    ("rightharpoondown;", "\u{21c1}"),
    ("rightharpoonup;", "\u{21c0}"),
    ("rightleftarrows;", "\u{21c4}"),
    ("rightleftharpoons;", "\u{21cc}"),
    ("rightrightarrows;", "\u{21c9}"),
    ("rightsquigarrow;", "\u{219d}"),
    ("rightthreetimes;", "\u{22cc}"),
    ("ring;", "\u{2da}"),
    ("risingdotseq;", "\u{2253}"),
    ("rlarr;", "\u{21c4}"),
    ("rlhar;", "\u{21cc}"),
    ("rlm;", "\u{200f}"),
    ("rmoust;", "\u{23b1}"),
    ("rmoustache;", "\u{23b1}"),
    ("rnmid;", "\u{2aee}"),
    ("roang;", "\u{27ed}"),
    ("roarr;", "\u{21fe}"),
    ("robrk;", "\u{27e7}"),
    ("ropar;", "\u{2986}"),
    ("ropf;", "\u{1d563}"),
    ("roplus;", "\u{2a2e}"),
    ("rotimes;", "\u{2a35}"),
    ("rpar;", "\u{29}"),
    ("rpargt;", "\u{2994}"),
    ("rppolint;", "\u{2a12}"),
    ("rrarr;", "\u{21c9}"),
    ("rsaquo;", "\u{203a}"),
    ("rscr;", "\u{1d4c7}"),
    ("rsh;", "\u{21b1}"),
    ("rsqb;", "\u{5d}"),
    ("rsquo;", "\u{2019}"),
    ("rsquor;", "\u{2019}"),
    ("rthree;", "\u{22cc}"),
    ("rtimes;", "\u{22ca}"),
    ("rtri;", "\u{25b9}"),
    ("rtrie;", "\u{22b5}"),
    ("rtrif;", "\u{25b8}"),
    ("rtriltri;", "\u{29ce}"),
    ("ruluhar;", "\u{2968}"),
    ("rx;", "\u{211e}"),
    ("sacute;", "\u{15b}"),
    ("sbquo;", "\u{201a}"),
    ("sc;", "\u{227b}"),
    ("scE;", "\u{2ab4}"),
    ("scap;", "\u{2ab8}"),
    ("scaron;", "\u{161}"),
    ("sccue;", "\u{227d}"),
    ("sce;", "\u{2ab0}"),
    ("scedil;", "\u{15f}"),
    ("scirc;", "\u{15d}"),
    ("scnE;", "\u{2ab6}"),
    ("scnap;", "\u{2aba}"),
    ("scnsim;", "\u{22e9}"),
    ("scpolint;", "\u{2a13}"),
    ("scsim;", "\u{227f}"),
    ("scy;", "\u{441}"),
    ("sdot;", "\u{22c5}"),
    ("sdotb;", "\u{22a1}"),
    ("sdote;", "\u{2a66}"),
    ("seArr;", "\u{21d8}"),
    ("searhk;", "\u{2925}"),
    ("searr;", "\u{2198}"),
    ("searrow;", "\u{2198}"),
    ("sect", "\u{a7}"),
    ("sect;", "\u{a7}"),
    ("semi;", "\u{3b}"),
    ("seswar;", "\u{2929}"),
    ("setminus;", "\u{2216}"),
    ("setmn;", "\u{2216}"),
    ("sext;", "\u{2736}"),
    ("sfr;", "\u{1d530}"),
    ("sfrown;", "\u{2322}"),
    ("sharp;", "\u{266f}"),
    ("shchcy;", "\u{449}"),
    ("shcy;", "\u{448}"),
    ("shortmid;", "\u{2223}"),
    ("shortparallel;", "\u{2225}"),
    ("shy", "\u{ad}"),
    ("shy;", "\u{ad}"),
    ("sigma;", "\u{3c3}"),
    ("sigmaf;", "\u{3c2}"),
    ("sigmav;", "\u{3c2}"),
    ("sim;", "\u{223c}"),
    ("simdot;", "\u{2a6a}"),
    ("sime;", "\u{2243}"),
    ("simeq;", "\u{2243}"),
    ("simg;", "\u{2a9e}"),
    ("simgE;", "\u{2aa0}"),
    ("siml;", "\u{2a9d}"),
    ("simlE;", "\u{2a9f}"),
    ("simne;", "\u{2246}"),
    ("simplus;", "\u{2a24}"),
    ("simrarr;", "\u{2972}"),
    ("slarr;", "\u{2190}"),
    ("smallsetminus;", "\u{2216}"),
    ("smashp;", "\u{2a33}"),
    ("smeparsl;", "\u{29e4}"),
    ("smid;", "\u{2223}"),
    ("smile;", "\u{2323}"),
    ("smt;", "\u{2aaa}"),
    ("smte;", "\u{2aac}"),
    ("smtes;", "\u{2aac}\u{fe00}"),
    ("softcy;", "\u{44c}"),
    ("sol;", "\u{2f}"),
    ("solb;", "\u{29c4}"),
    ("solbar;", "\u{233f}"),
    ("sopf;", "\u{1d564}"),
    ("spades;", "\u{2660}"),
    ("spadesuit;", "\u{2660}"),
    ("spar;", "\u{2225}"),
    ("sqcap;", "\u{2293}"),
    ("sqcaps;", "\u{2293}\u{fe00}"),
    ("sqcup;", "\u{2294}"),
    ("sqcups;", "\u{2294}\u{fe00}"),
    ("sqsub;", "\u{228f}"),
    ("sqsube;", "\u{2291}"),
    ("sqsubset;", "\u{228f}"),
    ("sqsubseteq;", "\u{2291}"),
    ("sqsup;", "\u{2290}"),
    ("sqsupe;", "\u{2292}"),
    ("sqsupset;", "\u{2290}"),
    ("sqsupseteq;", "\u{2292}"),
    ("squ;", "\u{25a1}"),
    ("square;", "\u{25a1}"),
    ("squarf;", "\u{25aa}"),
    ("squf;", "\u{25aa}"),
    ("srarr;", "\u{2192}"),
    ("sscr;", "\u{1d4c8}"),
    ("ssetmn;", "\u{2216}"),
    ("ssmile;", "\u{2323}"),
    ("sstarf;", "\u{22c6}"),
    ("star;", "\u{2606}"),
    ("starf;", "\u{2605}"),
    ("straightepsilon;", "\u{3f5}"),
    ("straightphi;", "\u{3d5}"),
    ("strns;", "\u{af}"),
    ("sub;", "\u{2282}"),
    ("subE;", "\u{2ac5}"),
    ("subdot;", "\u{2abd}"),
    ("sube;", "\u{2286}"),
    ("subedot;", "\u{2ac3}"),
    ("submult;", "\u{2ac1}"),
    ("subnE;", "\u{2acb}"),
    ("subne;", "\u{228a}"),
    ("subplus;", "\u{2abf}"),
    ("subrarr;", "\u{2979}"),
    ("subset;", "\u{2282}"),
    ("subseteq;", "\u{2286}"),
    ("subseteqq;", "\u{2ac5}"),
    ("subsetneq;", "\u{228a}"),
    ("subsetneqq;", "\u{2acb}"),
    ("subsim;", "\u{2ac7}"),
    ("subsub;", "\u{2ad5}"),
    ("subsup;", "\u{2ad3}"),
    ("succ;", "\u{227b}"),
    ("succapprox;", "\u{2ab8}"),
    ("succcurlyeq;", "\u{227d}"),
    ("succeq;", "\u{2ab0}"),
    ("succnapprox;", "\u{2aba}"),
    ("succneqq;", "\u{2ab6}"),
    ("succnsim;", "\u{22e9}"),
    ("succsim;", "\u{227f}"),
    ("sum;", "\u{2211}"),
    ("sung;", "\u{266a}"),
    ("sup1", "\u{b9}"),
    ("sup1;", "\u{b9}"),
    ("sup2", "\u{b2}"),
    ("sup2;", "\u{b2}"),
    ("sup3", "\u{b3}"),
    ("sup3;", "\u{b3}"),
    ("sup;", "\u{2283}"),
    ("supE;", "\u{2ac6}"),
    ("supdot;", "\u{2abe}"),
    ("supdsub;", "\u{2ad8}"),
    ("supe;", "\u{2287}"),
    ("supedot;", "\u{2ac4}"),
    ("suphsol;", "\u{27c9}"),
    ("suphsub;", "\u{2ad7}"),
    ("suplarr;", "\u{297b}"),
    ("supmult;", "\u{2ac2}"),
    ("supnE;", "\u{2acc}"),
    ("supne;", "\u{228b}"),
    ("supplus;", "\u{2ac0}"),
    ("supset;", "\u{2283}"),
    ("supseteq;", "\u{2287}"),
    ("supseteqq;", "\u{2ac6}"),
    ("supsetneq;", "\u{228b}"),
    ("supsetneqq;", "\u{2acc}"),
    ("supsim;", "\u{2ac8}"),
    ("supsub;", "\u{2ad4}"),
    ("supsup;", "\u{2ad6}"),
    ("swArr;", "\u{21d9}"),
    ("swarhk;", "\u{2926}"),
    ("swarr;", "\u{2199}"),
    ("swarrow;", "\u{2199}"),
    ("swnwar;", "\u{292a}"),
    ("szlig", "\u{df}"),
    ("szlig;", "\u{df}"),
    ("target;", "\u{2316}"),
    ("tau;", "\u{3c4}"),
    ("tbrk;", "\u{23b4}"),
    ("tcaron;", "\u{165}"),
    ("tcedil;", "\u{163}"),
    ("tcy;", "\u{442}"),
    ("tdot;", "\u{20db}"),
    ("telrec;", "\u{2315}"),
    ("tfr;", "\u{1d531}"),
    ("there4;", "\u{2234}"),
    ("therefore;", "\u{2234}"),
    ("theta;", "\u{3b8}"),
    ("thetasym;", "\u{3d1}"),
    ("thetav;", "\u{3d1}"),
    ("thickapprox;", "\u{2248}"),
    ("thicksim;", "\u{223c}"),
    ("thinsp;", "\u{2009}"),
    ("thkap;", "\u{2248}"),
    ("thksim;", "\u{223c}"),
    ("thorn", "\u{fe}"),
    ("thorn;", "\u{fe}"),
    ("tilde;", "\u{2dc}"),
    ("times", "\u{d7}"),
    ("times;", "\u{d7}"),
    ("timesb;", "\u{22a0}"),
    ("timesbar;", "\u{2a31}"),
    ("timesd;", "\u{2a30}"),
    ("tint;", "\u{222d}"),
    ("toea;", "\u{2928}"),
    ("top;", "\u{22a4}"),
    ("topbot;", "\u{2336}"),
    ("topcir;", "\u{2af1}"),
    ("topf;", "\u{1d565}"),
    ("topfork;", "\u{2ada}"),
    ("tosa;", "\u{2929}"),
    ("tprime;", "\u{2034}"),
    ("trade;", "\u{2122}"),
    ("triangle;", "\u{25b5}"),
    ("triangledown;", "\u{25bf}"),
    ("triangleleft;", "\u{25c3}"),
    ("trianglelefteq;", "\u{22b4}"),
    ("triangleq;", "\u{225c}"),
    ("triangleright;", "\u{25b9}"),
    ("trianglerighteq;", "\u{22b5}"),
    ("tridot;", "\u{25ec}"),
    ("trie;", "\u{225c}"),
    ("triminus;", "\u{2a3a}"),
    ("triplus;", "\u{2a39}"),
    ("trisb;", "\u{29cd}"),
    ("tritime;", "\u{2a3b}"),
    ("trpezium;", "\u{23e2}"),
    ("tscr;", "\u{1d4c9}"),
    ("tscy;", "\u{446}"),
    ("tshcy;", "\u{45b}"),
    ("tstrok;", "\u{167}"),
    ("twixt;", "\u{226c}"),
    ("twoheadleftarrow;", "\u{219e}"),
    ("twoheadrightarrow;", "\u{21a0}"),
    ("uArr;", "\u{21d1}"),
    ("uHar;", "\u{2963}"),
    ("uacute", "\u{fa}"),
    ("uacute;", "\u{fa}"),
    ("uarr;", "\u{2191}"),
    ("ubrcy;", "\u{45e}"),
    ("ubreve;", "\u{16d}"),
    ("ucirc", "\u{fb}"),
    ("ucirc;", "\u{fb}"),
    ("ucy;", "\u{443}"),
    ("udarr;", "\u{21c5}"),
    ("udblac;", "\u{171}"),
    ("udhar;", "\u{296e}"),
    ("ufisht;", "\u{297e}"),
    ("ufr;", "\u{1d532}"),
    ("ugrave", "\u{f9}"),
    ("ugrave;", "\u{f9}"),
    ("uharl;", "\u{21bf}"),
    ("uharr;", "\u{21be}"),
    ("uhblk;", "\u{2580}"),
    ("ulcorn;", "\u{231c}"),
    ("ulcorner;", "\u{231c}"),
    ("ulcrop;", "\u{230f}"),
    ("ultri;", "\u{25f8}"),
    ("umacr;", "\u{16b}"),
    ("uml", "\u{a8}"),
    ("uml;", "\u{a8}"),
    ("uogon;", "\u{173}"),
    ("uopf;", "\u{1d566}"),
    ("uparrow;", "\u{2191}"),
    ("updownarrow;", "\u{2195}"),
    ("upharpoonleft;", "\u{21bf}"),
    ("upharpoonright;", "\u{21be}"),
    ("uplus;", "\u{228e}"),
    ("upsi;", "\u{3c5}"),
    ("upsih;", "\u{3d2}"),
    ("upsilon;", "\u{3c5}"),
    ("upuparrows;", "\u{21c8}"),
    ("urcorn;", "\u{231d}"),
    ("urcorner;", "\u{231d}"),
    ("urcrop;", "\u{230e}"),
    ("uring;", "\u{16f}"),
    ("urtri;", "\u{25f9}"),
    ("uscr;", "\u{1d4ca}"),
    ("utdot;", "\u{22f0}"),
    ("utilde;", "\u{169}"),
    ("utri;", "\u{25b5}"),
    ("utrif;", "\u{25b4}"),
    ("uuarr;", "\u{21c8}"),
    ("uuml", "\u{fc}"),
    ("uuml;", "\u{fc}"),
    ("uwangle;", "\u{29a7}"),
    ("vArr;", "\u{21d5}"),
    ("vBar;", "\u{2ae8}"),
    ("vBarv;", "\u{2ae9}"),
    ("vDash;", "\u{22a8}"),
    ("vangrt;", "\u{299c}"),
    ("varepsilon;", "\u{3f5}"),
    ("varkappa;", "\u{3f0}"),
    ("varnothing;", "\u{2205}"),
    ("varphi;", "\u{3d5}"),
    ("varpi;", "\u{3d6}"),
    ("varpropto;", "\u{221d}"),
    ("varr;", "\u{2195}"),
    ("varrho;", "\u{3f1}"),
    ("varsigma;", "\u{3c2}"),
    ("varsubsetneq;", "\u{228a}\u{fe00}"),
    ("varsubsetneqq;", "\u{2acb}\u{fe00}"),
    ("varsupsetneq;", "\u{228b}\u{fe00}"),
    ("varsupsetneqq;", "\u{2acc}\u{fe00}"),
    ("vartheta;", "\u{3d1}"),
    ("vartriangleleft;", "\u{22b2}"),
    ("vartriangleright;", "\u{22b3}"),
    ("vcy;", "\u{432}"),
    ("vdash;", "\u{22a2}"),
    ("vee;", "\u{2228}"),
    ("veebar;", "\u{22bb}"),
    ("veeeq;", "\u{225a}"),
    ("vellip;", "\u{22ee}"),
    ("verbar;", "\u{7c}"),
    ("vert;", "\u{7c}"),
    ("vfr;", "\u{1d533}"),
    ("vltri;", "\u{22b2}"),
    ("vnsub;", "\u{2282}\u{20d2}"),
    ("vnsup;", "\u{2283}\u{20d2}"),
    ("vopf;", "\u{1d567}"),
    ("vprop;", "\u{221d}"),
    ("vrtri;", "\u{22b3}"),
    ("vscr;", "\u{1d4cb}"),
    ("vsubnE;", "\u{2acb}\u{fe00}"),
    ("vsubne;", "\u{228a}\u{fe00}"),
    ("vsupnE;", "\u{2acc}\u{fe00}"),
    ("vsupne;", "\u{228b}\u{fe00}"),
    ("vzigzag;", "\u{299a}"),
    ("wcirc;", "\u{175}"),
    ("wedbar;", "\u{2a5f}"),
    ("wedge;", "\u{2227}"),
    ("wedgeq;", "\u{2259}"),
    ("weierp;", "\u{2118}"),
    ("wfr;", "\u{1d534}"),
    ("wopf;", "\u{1d568}"),
    ("wp;", "\u{2118}"),
    ("wr;", "\u{2240}"),
    ("wreath;", "\u{2240}"),
    ("wscr;", "\u{1d4cc}"),
    ("xcap;", "\u{22c2}"),
    ("xcirc;", "\u{25ef}"),
    ("xcup;", "\u{22c3}"),
    ("xdtri;", "\u{25bd}"),
    ("xfr;", "\u{1d535}"),
    ("xhArr;", "\u{27fa}"),
    ("xharr;", "\u{27f7}"),
    ("xi;", "\u{3be}"),
    ("xlArr;", "\u{27f8}"),
    ("xlarr;", "\u{27f5}"),
    ("xmap;", "\u{27fc}"),
    ("xnis;", "\u{22fb}"),
    ("xodot;", "\u{2a00}"),
    ("xopf;", "\u{1d569}"),
    ("xoplus;", "\u{2a01}"),
    ("xotime;", "\u{2a02}"),
    ("xrArr;", "\u{27f9}"),
    ("xrarr;", "\u{27f6}"),
    ("xscr;", "\u{1d4cd}"),
    ("xsqcup;", "\u{2a06}"),
    ("xuplus;", "\u{2a04}"),
    ("xutri;", "\u{25b3}"),
    ("xvee;", "\u{22c1}"),
    ("xwedge;", "\u{22c0}"),
    ("yacute", "\u{fd}"),
    ("yacute;", "\u{fd}"),
    ("yacy;", "\u{44f}"),
    ("ycirc;", "\u{177}"),
    ("ycy;", "\u{44b}"),
    ("yen", "\u{a5}"),
    ("yen;", "\u{a5}"),
    ("yfr;", "\u{1d536}"),
    ("yicy;", "\u{457}"),
    ("yopf;", "\u{1d56a}"),
    ("yscr;", "\u{1d4ce}"),
    ("yucy;", "\u{44e}"),
    ("yuml", "\u{ff}"),
    ("yuml;", "\u{ff}"),
    ("zacute;", "\u{17a}"),
    ("zcaron;", "\u{17e}"),
    ("zcy;", "\u{437}"),
    ("zdot;", "\u{17c}"),
    ("zeetrf;", "\u{2128}"),
    ("zeta;", "\u{3b6}"),
    ("zfr;", "\u{1d537}"),
    ("zhcy;", "\u{436}"),
    ("zigrarr;", "\u{21dd}"),
    ("zopf;", "\u{1d56b}"),
    ("zscr;", "\u{1d4cf}"),
    ("zwj;", "\u{200d}"),
    ("zwnj;", "\u{200c}"),
];
