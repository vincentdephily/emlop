/// Parsing trait for args
///
/// Similar to std::convert::From but takes extra context and returns a custom error
pub trait ArgParse<T, A> {
    fn parse(val: &T, arg: A, src: &'static str) -> Result<Self, ArgError>
        where Self: Sized;
}
impl ArgParse<bool, ()> for bool {
    fn parse(b: &bool, _: (), _src: &'static str) -> Result<Self, ArgError> {
        Ok(*b)
    }
}
impl ArgParse<String, ()> for String {
    fn parse(s: &String, _: (), _src: &'static str) -> Result<Self, ArgError> {
        Ok((*s).clone())
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
    pub fn new(val: impl Into<String>, src: &'static str) -> Self {
        Self { val: val.into(), src, msg: String::new(), possible: "" }
    }
    /// Set extra error message
    pub fn msg(mut self, msg: impl Into<String>) -> Self {
        self.msg = msg.into();
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


#[derive(Clone, Copy)]
pub enum Average {
    Arith,
    Median,
    WeightedArith,
    WeightedMedian,
}
impl ArgParse<String, ()> for Average {
    fn parse(v: &String, _: (), s: &'static str) -> Result<Self, ArgError> {
        match v.as_str() {
            "a" | "arith" => Ok(Self::Arith),
            "m" | "median" => Ok(Self::Median),
            "wa" | "weighted-arith" => Ok(Self::WeightedArith),
            "wm" | "weighted-median" => Ok(Self::WeightedMedian),
            _ => Err(ArgError::new(v, s).pos("arith median weightedarith weigtedmedian a m wa wm")),
        }
    }
}


#[derive(Clone, Copy)]
pub struct Show {
    pub pkg: bool,
    pub tot: bool,
    pub sync: bool,
    pub merge: bool,
    pub unmerge: bool,
    pub emerge: bool,
}
impl Show {
    pub const fn m() -> Self {
        Self { pkg: false, tot: false, sync: false, merge: true, unmerge: false, emerge: false }
    }
    pub const fn emt() -> Self {
        Self { pkg: false, tot: true, sync: false, merge: true, unmerge: false, emerge: true }
    }
    pub const fn p() -> Self {
        Self { pkg: true, tot: false, sync: false, merge: false, unmerge: false, emerge: false }
    }
    pub const fn mt() -> Self {
        Self { pkg: false, tot: true, sync: false, merge: true, unmerge: false, emerge: false }
    }
}
impl ArgParse<String, &'static str> for Show {
    fn parse(show: &String, valid: &'static str, src: &'static str) -> Result<Self, ArgError> {
        debug_assert!(valid.is_ascii()); // Because we use `chars()` we need to stick to ascii for `valid`.
        if show.chars().all(|c| valid.contains(c)) {
            Ok(Self { pkg: show.contains('p') || show.contains('a'),
                      tot: show.contains('t') || show.contains('a'),
                      sync: show.contains('s') || show.contains('a'),
                      merge: show.contains('m') || show.contains('a'),
                      unmerge: show.contains('u') || show.contains('a'),
                      emerge: show.contains('e') || show.contains('a') })
        } else {
            Err(ArgError::new(show, src).msg("Invalid letter").pos(valid))
        }
    }
}
impl std::fmt::Display for Show {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut sep = "";
        for (b, s) in [(self.pkg, "pkg"),
                       (self.tot, "total"),
                       (self.sync, "sync"),
                       (self.merge, "merge"),
                       (self.unmerge, "unmerge"),
                       (self.emerge, "emerge")]
        {
            if b {
                write!(f, "{sep}{s}")?;
                sep = ",";
            }
        }
        Ok(())
    }
}
