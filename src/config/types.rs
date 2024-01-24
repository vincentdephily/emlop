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
