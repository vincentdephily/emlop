use emlop_lib::{ArgError, ArgParse};

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
            _ => Err(ArgError::new(v, s).pos("(a)rith (m)edian wa/weightedarith wm/weigtedmedian")),
        }
    }
}
impl Average {
    pub fn as_str(&self) -> &'static str {
        match &self {
            Self::Arith => "Arithetic mean",
            Self::Median => "Median",
            Self::WeightedArith => "Weighted arithmetic mean",
            Self::WeightedMedian => "Weighted median",
        }
    }
}

#[derive(Clone, Copy)]
pub enum DurationStyle {
    Hms,
    HmsFixed,
    Secs,
    Human,
}
impl ArgParse<String, ()> for DurationStyle {
    fn parse(v: &String, _: (), s: &'static str) -> Result<Self, ArgError> {
        match v.as_str() {
            "hms" => Ok(Self::Hms),
            "hmsfixed" => Ok(Self::HmsFixed),
            "s" | "secs" => Ok(Self::Secs),
            "h" | "human" => Ok(Self::Human),
            _ => Err(ArgError::new(v, s).pos("hms hmsfixed (s)ecs (h)uman")),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum OutStyle {
    Columns,
    Tab,
}
impl ArgParse<String, bool> for OutStyle {
    fn parse(v: &String, isterm: bool, s: &'static str) -> Result<Self, ArgError> {
        match v.as_str() {
            "auto" | "a" => Ok(if isterm { Self::Columns } else { Self::Tab }),
            "tab" | "t" => Ok(Self::Tab),
            "columns" | "c" => Ok(Self::Columns),
            _ => Err(ArgError::new(v, s).pos("(c)olumns (t)ab (a)uto")),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Tty {
    Auto,
    In,
    Out,
    Inout,
    None,
}
impl ArgParse<String, ()> for Tty {
    fn parse(v: &String, _: (), s: &'static str) -> Result<Self, ArgError> {
        match v.as_str() {
            "a" | "auto" => Ok(Self::Auto),
            "i" | "in" => Ok(Self::In),
            "o" | "out" => Ok(Self::Out),
            "io" | "inout" => Ok(Self::Inout),
            "n" | "none" => Ok(Self::None),
            _ => Err(ArgError::new(v, s).pos("(a)uto (i)n (o)ut (io)nout (n)one")),
        }
    }
}
