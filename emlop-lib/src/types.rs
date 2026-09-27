use std::{ops::RangeInclusive, str::FromStr};

/// Parsing trait for args
///
/// Similar to std::convert::From but takes extra context and returns a custom error
pub trait ArgParse<T, A> {
    /// Convert `T` to `Self`, using `A` for context and `src` for error reporting
    fn parse(val: &T, arg: A, src: &'static str) -> Result<Self, ArgError>
        where Self: Sized;
}
impl ArgParse<String, ()> for String {
    fn parse(s: &String, _: (), _src: &'static str) -> Result<Self, ArgError> {
        Ok((*s).clone())
    }
}
impl ArgParse<bool, ()> for bool {
    fn parse(b: &bool, _: (), _src: &'static str) -> Result<Self, ArgError> {
        Ok(*b)
    }
}
impl ArgParse<String, ()> for bool {
    fn parse(s: &String, _: (), src: &'static str) -> Result<Self, ArgError> {
        match s.as_str() {
            "y" | "yes" => Ok(true),
            "n" | "no" => Ok(false),
            _ => Err(ArgError::new(s, src).pos("y(es) n(o)")),
        }
    }
}
impl ArgParse<String, RangeInclusive<i64>> for i64 {
    fn parse(s: &String, r: RangeInclusive<i64>, src: &'static str) -> Result<Self, ArgError> {
        let i = i64::from_str(s).map_err(|_| ArgError::new(s, src).msg("Not an integer"))?;
        Self::parse(&i, r, src)
    }
}
impl ArgParse<i64, RangeInclusive<i64>> for i64 {
    fn parse(i: &i64, r: RangeInclusive<i64>, src: &'static str) -> Result<Self, ArgError> {
        if r.contains(i) {
            Ok(*i)
        } else {
            Err(ArgError::new(i, src).msg(format!("Should be between {} and {}",
                                                  r.start(),
                                                  r.end())))
        }
    }
}
impl ArgParse<String, bool> for bool {
    fn parse(v: &String, default: bool, s: &'static str) -> Result<Self, ArgError> {
        match v.as_str() {
            "auto" | "a" => Ok(default),
            "yes" | "y" => Ok(true),
            "no" | "n" => Ok(false),
            _ => Err(ArgError::new(v, s).pos("(y)es (n)o (a)uto")),
        }
    }
}


/// Argument parsing error
///
/// Designed to look like clap::Error, but more tailored to our usecase
#[derive(Debug, PartialEq)]
pub struct ArgError {
    val: String,
    src: &'static str,
    msg: String,
    possible: &'static str,
}
impl ArgError {
    /// Instantiate basic error with value and source
    pub fn new(val: impl ToString, src: &'static str) -> Self {
        Self { val: val.to_string(), src, msg: String::new(), possible: "" }
    }
    /// Set extra error message
    pub fn msg(mut self, msg: impl ToString) -> Self {
        self.msg = msg.to_string();
        self
    }
    /// Set possible values (as a space-delimited string or as a set of letters)
    pub const fn pos(mut self, possible: &'static str) -> Self {
        self.possible = possible;
        self
    }
}
impl std::error::Error for ArgError {}
impl std::fmt::Display for ArgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let r = "\x1B[1;31m";
        let g = "\x1B[32m";
        let b = "\x1B[33m";
        let c = "\x1B[m";
        write!(f, "{r}error{c}: invalid value '{b}{}{c}' for '{g}{}{c}'", self.val, self.src)?;
        if !self.msg.is_empty() {
            write!(f, ": {}", self.msg)?;
        }
        if !self.possible.is_empty() {
            if self.possible.contains(' ') {
                let mut sep = "\n  possible values: ";
                for p in self.possible.split_ascii_whitespace() {
                    write!(f, "{sep}{g}{p}{c}")?;
                    sep = ", ";
                }
            } else {
                let mut sep = "\n  possible value: combination of ";
                for p in self.possible.chars() {
                    write!(f, "{sep}{g}{p}{c}")?;
                    sep = ", ";
                }
            }
        }
        write!(f, "\n\nFor more information, try '{g}--help{c}'.")
    }
}
