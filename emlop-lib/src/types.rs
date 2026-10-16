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
    fn parse(v: &String, default: bool, src: &'static str) -> Result<Self, ArgError> {
        match v.as_str() {
            "auto" | "a" => Ok(default),
            "yes" | "y" => Ok(true),
            "no" | "n" => Ok(false),
            _ => Err(ArgError::new(v, src).pos("(y)es (n)o (a)uto")),
        }
    }
}


/// Argument parsing error
///
/// Designed to look like clap::Error, but more tailored to our usecase
#[derive(Debug, PartialEq)]
pub struct ArgError {
    /// The stringified value that failed parsing
    pub val: String,
    /// The source (eg argument name) of the value
    pub src: &'static str,
    /// Core error message (eg "not a number")
    pub msg: Option<String>,
    /// Possible error values (see `pos()` fn)
    pub pos: Option<&'static str>,
}
impl ArgError {
    /// Instantiate basic error with value and source
    pub fn new(val: impl ToString, src: &'static str) -> Self {
        Self { val: val.to_string(), src, msg: None, pos: None }
    }
    /// Set extra error message
    pub fn msg(mut self, msg: impl ToString) -> Self {
        self.msg = Some(msg.to_string());
        self
    }
    /// Set possible values (as a space-delimited string or as a set of letters)
    pub const fn pos(mut self, pos: &'static str) -> Self {
        self.pos = Some(pos);
        self
    }
}
impl std::error::Error for ArgError {}
impl std::fmt::Display for ArgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        match &self.msg {
            Some(msg) => write!(f, "{} '{}': {msg}", self.src, self.val)?,
            None => write!(f, "{} '{}': Invalid value", self.src, self.val)?,
        }
        if let Some(pos) = self.pos {
            if pos.contains(' ') {
                let mut sep = ", try one of ";
                for p in pos.split_ascii_whitespace() {
                    write!(f, "{sep}{p}")?;
                    sep = ", ";
                }
            } else {
                let mut sep = ", try combination of ";
                for p in pos.chars() {
                    write!(f, "{sep}{p}")?;
                    sep = ", ";
                }
            }
        }
        Ok(())
    }
}
