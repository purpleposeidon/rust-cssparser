/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::ascii::AsciiExt;
use std::fmt::{self, Write};

use super::{Token, NumericValue, PercentageValue};


/// Trait for things the can serialize themselves in CSS syntax.
pub trait ToCss {
    /// Serialize `self` in CSS syntax, writing to `dest`.
    fn to_css<W>(&self, dest: &mut W) -> fmt::Result where W: fmt::Write;

    /// Serialize `self` in CSS syntax and return a string.
    ///
    /// (This is a convenience wrapper for `to_css` and probably should not be overridden.)
    #[inline]
    fn to_css_string(&self) -> String {
        let mut s = String::new();
        self.to_css(&mut s).unwrap();
        s
    }

    /// Serialize `self` in CSS syntax and return a result compatible with `std::fmt::Show`.
    ///
    /// Typical usage is, for a `Foo` that implements `ToCss`:
    ///
    /// ```{rust,ignore}
    /// use std::fmt;
    /// impl fmt::Show for Foo {
    ///     #[inline] fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { self.fmt_to_css(f) }
    /// }
    /// ```
    ///
    /// (This is a convenience wrapper for `to_css` and probably should not be overridden.)
    #[inline]
    fn fmt_to_css<W>(&self, dest: &mut W) -> fmt::Result where W: fmt::Write {
        self.to_css(dest).map_err(|_| fmt::Error)
    }
}


#[inline]
fn write_numeric<W>(value: NumericValue, dest: &mut W) -> fmt::Result where W: fmt::Write {
    // `value.value >= 0` is true for negative 0.
    if value.has_sign && value.value.is_sign_positive() {
        try!(dest.write_str("+"));
    }

    if value.value == 0.0 && value.value.is_sign_negative() {
        // Negative zero. Work around #20596.
        try!(dest.write_str("-0"))
    } else {
        try!(write!(dest, "{}", value.value))
    }

    if value.int_value.is_none() && value.value.fract() == 0. {
        try!(dest.write_str(".0"));
    }
    Ok(())
}


impl<'a> ToCss for Token<'a> {
    fn to_css<W>(&self, dest: &mut W) -> fmt::Result where W: fmt::Write {
        match *self {
            Token::Ident(ref value) => try!(serialize_identifier(&**value, dest)),
            Token::AtKeyword(ref value) => {
                try!(dest.write_str("@"));
                try!(serialize_identifier(&**value, dest));
            },
            Token::Hash(ref value) => {
                try!(dest.write_str("#"));
                try!(serialize_name(value, dest));
            },
            Token::IDHash(ref value) => {
                try!(dest.write_str("#"));
                try!(serialize_identifier(&**value, dest));
            }
            Token::QuotedString(ref value) => try!(serialize_string(&**value, dest)),
            Token::UnquotedUrl(ref value) => {
                try!(dest.write_str("url("));
                try!(serialize_unquoted_url(&**value, dest));
                try!(dest.write_str(")"));
            },
            Token::Delim(value) => try!(write!(dest, "{}", value)),

            Token::Number(value) => try!(write_numeric(value, dest)),
            Token::Percentage(PercentageValue { unit_value, int_value, has_sign }) => {
                let value = NumericValue {
                    value: unit_value * 100.,
                    int_value: int_value,
                    has_sign: has_sign,
                };
                try!(write_numeric(value, dest));
                try!(dest.write_str("%"));
            },
            Token::Dimension(value, ref unit) => {
                try!(write_numeric(value, dest));
                // Disambiguate with scientific notation.
                let unit = &**unit;
                if unit == "e" || unit == "E" || unit.starts_with("e-") || unit.starts_with("E-") {
                    try!(dest.write_str("\\65 "));
                    try!(serialize_name(&unit[1..], dest));
                } else {
                    try!(serialize_identifier(unit, dest));
                }
            },

            Token::WhiteSpace(content) => try!(dest.write_str(content)),
            Token::Comment(content) => try!(write!(dest, "/*{}*/", content)),
            Token::Colon => try!(dest.write_str(":")),
            Token::Semicolon => try!(dest.write_str(";")),
            Token::Comma => try!(dest.write_str(",")),
            Token::IncludeMatch => try!(dest.write_str("~=")),
            Token::DashMatch => try!(dest.write_str("|=")),
            Token::PrefixMatch => try!(dest.write_str("^=")),
            Token::SuffixMatch => try!(dest.write_str("$=")),
            Token::SubstringMatch => try!(dest.write_str("*=")),
            Token::Column => try!(dest.write_str("||")),
            Token::CDO => try!(dest.write_str("<!--")),
            Token::CDC => try!(dest.write_str("-->")),

            Token::Function(ref name) => {
                try!(serialize_identifier(&**name, dest));
                try!(dest.write_str("("));
            },
            Token::ParenthesisBlock => try!(dest.write_str("(")),
            Token::SquareBracketBlock => try!(dest.write_str("[")),
            Token::CurlyBracketBlock => try!(dest.write_str("{")),

            Token::BadUrl => try!(dest.write_str("url(<bad url>)")),
            Token::BadString => try!(dest.write_str("\"<bad string>\n")),
            Token::CloseParenthesis => try!(dest.write_str(")")),
            Token::CloseSquareBracket => try!(dest.write_str("]")),
            Token::CloseCurlyBracket => try!(dest.write_str("}")),
        }
        Ok(())
    }
}


/// Write a CSS identifier, escaping characters as necessary.
pub fn serialize_identifier<W>(mut value: &str, dest: &mut W) -> fmt::Result where W:fmt::Write {
    if value.is_empty() {
        return Ok(())
    }

    if value.starts_with("--") {
        try!(dest.write_str("--"));
        serialize_name(&value[2..], dest)
    } else if value == "-" {
        dest.write_str("\\-")
    } else {
        if value.as_bytes()[0] == b'-' {
            try!(dest.write_str("-"));
            value = &value[1..];
        }
        if let digit @ b'0'...b'9' = value.as_bytes()[0] {
            try!(write!(dest, "\\3{} ", digit as char));
            value = &value[1..];
        }
        serialize_name(value, dest)
    }
}


fn serialize_name<W>(value: &str, dest: &mut W) -> fmt::Result where W:fmt::Write {
    let mut chunk_start = 0;
    for (i, b) in value.bytes().enumerate() {
        let escaped = match b {
            b'0'...b'9' | b'A'...b'Z' | b'a'...b'z' | b'_' | b'-' => continue,
            _ if !b.is_ascii() => continue,
            b'\0' => Some("\u{FFFD}"),
            _ => None,
        };
        try!(dest.write_str(&value[chunk_start..i]));
        if let Some(escaped) = escaped {
            try!(dest.write_str(escaped));
        } else if (b >= b'\x01' && b <= b'\x1F') || b == b'\x7F' {
            try!(write!(dest, "\\{:x} ", b));
        } else {
            try!(write!(dest, "\\{}", b as char));
        }
        chunk_start = i + 1;
    }
    dest.write_str(&value[chunk_start..])
}


fn serialize_unquoted_url<W>(value: &str, dest: &mut W) -> fmt::Result where W:fmt::Write {
    let mut chunk_start = 0;
    for (i, b) in value.bytes().enumerate() {
        let hex = match b {
            b'\0' ... b' ' | b'\x7F' => true,
            b'(' | b')' | b'"' | b'\'' | b'\\' => false,
            _ => continue
        };
        try!(dest.write_str(&value[chunk_start..i]));
        if hex {
            try!(write!(dest, "\\{:X} ", b));
        } else {
            try!(write!(dest, "\\{}", b as char));
        }
        chunk_start = i + 1;
    }
    dest.write_str(&value[chunk_start..])
}


/// Write a double-quoted CSS string token, escaping content as necessary.
pub fn serialize_string<W>(value: &str, dest: &mut W) -> fmt::Result where W: fmt::Write {
    try!(dest.write_str("\""));
    try!(CssStringWriter::new(dest).write_str(value));
    try!(dest.write_str("\""));
    Ok(())
}


/// A `fmt::Write` adapter that escapes text for writing as a double-quoted CSS string.
/// Quotes are not included.
///
/// Typical usage:
///
/// ```{rust,ignore}
/// fn write_foo<W>(foo: &Foo, dest: &mut W) -> fmt::Result where W: fmt::Write {
///     try!(dest.write_str("\""));
///     {
///         let mut string_dest = CssStringWriter::new(dest);
///         // Write into string_dest...
///     }
///     try!(dest.write_str("\""));
///     Ok(())
/// }
/// ```
pub struct CssStringWriter<'a, W: 'a> {
    inner: &'a mut W,
}

impl<'a, W> CssStringWriter<'a, W> where W: fmt::Write {
    /// Wrap a text writer to create a `CssStringWriter`.
    pub fn new(inner: &'a mut W) -> CssStringWriter<'a, W> {
        CssStringWriter { inner: inner }
    }
}

impl<'a, W> fmt::Write for CssStringWriter<'a, W> where W: fmt::Write {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let mut chunk_start = 0;
        for (i, b) in s.bytes().enumerate() {
            let escaped = match b {
                b'"' => Some("\\\""),
                b'\\' => Some("\\\\"),
                b'\n' => Some("\\A "),
                b'\r' => Some("\\D "),
                b'\0' => Some("\u{FFFD}"),
                b'\x01'...b'\x1F' | b'\x7F' => None,
                _ => continue,
            };
            try!(self.inner.write_str(&s[chunk_start..i]));
            match escaped {
                Some(x) => try!(self.inner.write_str(x)),
                None => try!(write!(self.inner, "\\{:x} ", b)),
            };
            chunk_start = i + 1;
        }
        self.inner.write_str(&s[chunk_start..])
    }
}


macro_rules! impl_tocss_for_number {
    ($T: ty) => {
        impl<'a> ToCss for $T {
            fn to_css<W>(&self, dest: &mut W) -> fmt::Result where W: fmt::Write {
                write!(dest, "{}", *self)
            }
        }
    }
}

impl_tocss_for_number!(f32);
impl_tocss_for_number!(f64);
impl_tocss_for_number!(i8);
impl_tocss_for_number!(u8);
impl_tocss_for_number!(i16);
impl_tocss_for_number!(u16);
impl_tocss_for_number!(i32);
impl_tocss_for_number!(u32);
impl_tocss_for_number!(i64);
impl_tocss_for_number!(u64);


/// A category of token. See the `needs_separator_when_before` method.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct TokenSerializationType(TokenSerializationTypeVariants);

#[cfg(feature = "heapsize")]
known_heap_size!(0, TokenSerializationType);

impl TokenSerializationType {
    /// Return a value that represents the absence of a token, e.g. before the start of the input.
    pub fn nothing() -> TokenSerializationType {
        TokenSerializationType(TokenSerializationTypeVariants::Nothing)
    }

    /// If this value is `TokenSerializationType::nothing()`, set it to the given value instead.
    pub fn set_if_nothing(&mut self, new_value: TokenSerializationType) {
        if self.0 == TokenSerializationTypeVariants::Nothing {
            self.0 = new_value.0
        }
    }

    /// Return true if, when a token of category `self` is serialized just before
    /// a token of category `other` with no whitespace in between,
    /// an empty comment `/**/` needs to be inserted between them
    /// so that they are not re-parsed as a single token.
    ///
    /// See https://drafts.csswg.org/css-syntax/#serialization
    pub fn needs_separator_when_before(self, other: TokenSerializationType) -> bool {
        use self::TokenSerializationTypeVariants::*;
        match self.0 {
            Ident => matches!(other.0,
                Ident | Function | UrlOrBadUrl | DelimMinus | Number | Percentage | Dimension |
                CDC | OpenParen),
            AtKeywordOrHash | Dimension => matches!(other.0,
                Ident | Function | UrlOrBadUrl | DelimMinus | Number | Percentage | Dimension |
                CDC),
            DelimHash | DelimMinus | Number => matches!(other.0,
                Ident | Function | UrlOrBadUrl | DelimMinus | Number | Percentage | Dimension),
            DelimAt => matches!(other.0,
                Ident | Function | UrlOrBadUrl | DelimMinus),
            DelimDotOrPlus => matches!(other.0, Number | Percentage | Dimension),
            DelimAssorted | DelimAsterisk => matches!(other.0, DelimEquals),
            DelimBar => matches!(other.0, DelimEquals | DelimBar | DashMatch),
            DelimSlash => matches!(other.0, DelimAsterisk | SubstringMatch),
            Nothing | WhiteSpace | Percentage | UrlOrBadUrl | Function | CDC | OpenParen |
            DashMatch | SubstringMatch | DelimQuestion | DelimEquals | Other => false,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum TokenSerializationTypeVariants {
    Nothing,
    WhiteSpace,
    AtKeywordOrHash,
    Number,
    Dimension,
    Percentage,
    UrlOrBadUrl,
    Function,
    Ident,
    CDC,
    DashMatch,
    SubstringMatch,
    OpenParen,         // '('
    DelimHash,         // '#'
    DelimAt,           // '@'
    DelimDotOrPlus,    // '.', '+'
    DelimMinus,        // '-'
    DelimQuestion,     // '?'
    DelimAssorted,     // '$', '^', '~'
    DelimEquals,       // '='
    DelimBar,          // '|'
    DelimSlash,        // '/'
    DelimAsterisk,     // '*'
    Other,             // anything else
}

impl<'a> Token<'a> {
    /// Categorize a token into a type that determines when `/**/` needs to be inserted
    /// between two tokens when serialized next to each other without whitespace in between.
    ///
    /// See the `TokenSerializationType::needs_separator_when_before` method.
    pub fn serialization_type(&self) -> TokenSerializationType {
        use self::TokenSerializationTypeVariants::*;
        TokenSerializationType(match *self {
            Token::Ident(_) => Ident,
            Token::AtKeyword(_) | Token::Hash(_) | Token::IDHash(_) => AtKeywordOrHash,
            Token::UnquotedUrl(_) | Token::BadUrl => UrlOrBadUrl,
            Token::Delim('#') => DelimHash,
            Token::Delim('@') => DelimAt,
            Token::Delim('.') | Token::Delim('+') => DelimDotOrPlus,
            Token::Delim('-') => DelimMinus,
            Token::Delim('?') => DelimQuestion,
            Token::Delim('$') | Token::Delim('^') | Token::Delim('~') => DelimAssorted,
            Token::Delim('=') => DelimEquals,
            Token::Delim('|') => DelimBar,
            Token::Delim('/') => DelimSlash,
            Token::Delim('*') => DelimAsterisk,
            Token::Number(_) => Number,
            Token::Percentage(_) => Percentage,
            Token::Dimension(..) => Dimension,
            Token::WhiteSpace(_) => WhiteSpace,
            Token::Comment(_) => DelimSlash,
            Token::DashMatch => DashMatch,
            Token::SubstringMatch => SubstringMatch,
            Token::Column => DelimBar,
            Token::CDC => CDC,
            Token::Function(_) => Function,
            Token::ParenthesisBlock => OpenParen,
            Token::SquareBracketBlock | Token::CurlyBracketBlock |
            Token::CloseParenthesis | Token::CloseSquareBracket | Token::CloseCurlyBracket |
            Token::QuotedString(_) | Token::BadString |
            Token::Delim(_) | Token::Colon | Token::Semicolon | Token::Comma | Token::CDO |
            Token::IncludeMatch | Token::PrefixMatch | Token::SuffixMatch
            => Other,
        })
    }
}
