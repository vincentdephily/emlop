use clap::error::{ContextKind, ContextValue, Error as ClapError, ErrorKind};

pub fn err(val: String, src: &'static str, possible: &'static str) -> ClapError {
    let mut err = ClapError::new(ErrorKind::InvalidValue);
    err.insert(ContextKind::InvalidValue, ContextValue::String(val));
    let p = possible.split_ascii_whitespace().map(|s| s.to_string()).collect();
    err.insert(ContextKind::ValidValue, ContextValue::Strings(p));
    err.insert(ContextKind::InvalidArg, ContextValue::String(src.to_string()));
    err
}

pub trait ArgParse<T, A> {
    fn parse(val: &T, arg: A, src: &'static str) -> Result<Self, ClapError>
        where Self: Sized;
}
impl ArgParse<bool, ()> for bool {
    fn parse(b: &bool, _: (), _src: &'static str) -> Result<Self, ClapError> {
        Ok(*b)
    }
}
impl ArgParse<String, ()> for bool {
    fn parse(s: &String, _: (), src: &'static str) -> Result<Self, ClapError> {
        match s.as_str() {
            "y" | "yes" => Ok(true),
            "n" | "no" => Ok(false),
            _ => Err(err(s.to_owned(), src, "y(es) n(o)")),
        }
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
    fn parse(s: &String, _: (), src: &'static str) -> Result<Self, ClapError> {
        use Average::*;
        match s.as_str() {
            "a" | "arith" => Ok(Arith),
            "m" | "median" => Ok(Median),
            "wa" | "weighted-arith" => Ok(WeightedArith),
            "wm" | "weighted-median" => Ok(WeightedMedian),
            _ => Err(err(s.to_owned(), src, "arith median weightedarith weigtedmedian a m wa wm")),
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
    fn parse(show: &String, valid: &'static str, src: &'static str) -> Result<Self, ClapError> {
        debug_assert!(valid.is_ascii()); // Because we use `chars()` we need to stick to ascii for `valid`.
        if show.chars().all(|c| valid.contains(c)) {
            Ok(Self { pkg: show.contains('p') || show.contains('a'),
                      tot: show.contains('t') || show.contains('a'),
                      sync: show.contains('s') || show.contains('a'),
                      merge: show.contains('m') || show.contains('a'),
                      unmerge: show.contains('u') || show.contains('a'),
                      emerge: show.contains('e') || show.contains('a') })
        } else {
            Err(err(show.to_string(), src, valid))
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
