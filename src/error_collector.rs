use std::{
    error::Error,
    fmt::{self, Display},
};

use color_eyre::eyre::{self, Report};
use itertools::Itertools;

use crate::fir_boundaries::{FIRParsingError, FIRResult};

#[derive(Debug)]
pub struct ErrorCollector {
    errors: Vec<FIRParsingError>,
}

pub type ColResult<T> = Result<T, ErrorCollector>;

impl ErrorCollector {
    pub fn new() -> Self {
        Self { errors: Vec::new() }
    }

    pub fn addresult<T, E>(&mut self, r: Result<T, E>) -> FIRResult<Option<T>>
    where
        E: Into<FIRParsingError>,
    {
        match r {
            Ok(v) => Ok(Some(v)),
            Err(e) => {
                self.adderror(e)?;
                Ok(None)
            }
        }
    }

    pub fn adderror<E>(&mut self, e: E) -> FIRResult<()>
    where
        E: Into<FIRParsingError>,
    {
        Ok(self.errors.push(e.into().recoverable()?))
    }

    pub fn adderrors(&mut self, others: Self) {
        self.errors.extend(others.errors)
    }

    pub fn to_col_result<T>(self, t: T) -> ColResult<T> {
        match self.errors.is_empty() {
            true => Ok(t),
            false => Err(self),
        }
    }
}

impl Display for ErrorCollector {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            self.errors.iter().map(|e| format!("{:#}", e)).format("\n")
        )
    }
}

impl Error for ErrorCollector {}
