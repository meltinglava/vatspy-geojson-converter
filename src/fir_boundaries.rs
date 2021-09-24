use std::{
    fmt::{self, Display},
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Write},
    num::ParseIntError,
    path::Path,
    str::FromStr,
};

use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    error_collector::{ColResult, ErrorCollector},
    Mode,
};

pub type FIRResult<T> = Result<T, FIRParsingError>;

#[derive(Error, Debug)]
pub enum FIRParsingError {
    #[error("Error parsing FIRBoundary.dat structure: {0}.")]
    FIRParsing(String),
    #[error("Point out of range: {0} is out of range for coordinates on the earth.")]
    PointOutOfRange(Point),
    #[error("Duplicates: FIR: {owner}, has duplicate points: {}.", .points.iter().join(", "))]
    DuplicatePointError {
        points: IndexSet<Point>,
        owner: String,
    },
    #[error("Airspace draw direction: FIR: {0} is drawn clockwise, all airspaces need to be drawn counterclockwise.")]
    AirspaceDrawDirection(String),
    #[error("Extention not after FIR: The following FIRs has atleast one extention that is not just after it in the file: {}.", .0.iter().join(", "))]
    ExtentionNotAfterFir(IndexSet<String>),
    #[error("FIRs defined multiple times: {}.", .0.iter().map(|(fir, n)| format!("{}: {}", fir, n)).join(", "))]
    MultipleFirs(IndexMap<String, usize>),
    #[error("Wrong min/max for sector: {1}: {}.", .0.iter().map(|(stated, actual, typ)| format!("stated {}: {}, actual: {}", typ, stated, actual)).join(", "))]
    WrongMinMax(Vec<(Decimal, Decimal, &'static str)>, String),
    #[error(transparent)]
    ParseDecimalError(#[from] rust_decimal::Error),
    #[error(transparent)]
    ParseIntError(#[from] ParseIntError),
    #[error(transparent)]
    IoError(#[from] io::Error),
    #[error("No more file to read")]
    EOFError,
}

impl FIRParsingError {
    /// This function is ment to use to see if we can continue finding errors or if we should return error imidiatly,
    /// because we assume that the error will cause other errors or make more confutions afterwards.
    pub fn recoverable(self) -> Result<Self, Self> {
        match self {
            FIRParsingError::FIRParsing(e) => Err(FIRParsingError::FIRParsing(e)),
            FIRParsingError::PointOutOfRange(e) => Ok(FIRParsingError::PointOutOfRange(e)),
            FIRParsingError::DuplicatePointError { points, owner } => {
                Ok(FIRParsingError::DuplicatePointError { points, owner })
            }
            FIRParsingError::AirspaceDrawDirection(e) => {
                Ok(FIRParsingError::AirspaceDrawDirection(e))
            }
            FIRParsingError::ExtentionNotAfterFir(e) => {
                Ok(FIRParsingError::ExtentionNotAfterFir(e))
            }
            FIRParsingError::MultipleFirs(e) => Err(FIRParsingError::MultipleFirs(e)),
            FIRParsingError::WrongMinMax(d, f) => Ok(FIRParsingError::WrongMinMax(d, f)),
            FIRParsingError::ParseDecimalError(e) => Err(FIRParsingError::ParseDecimalError(e)),
            FIRParsingError::ParseIntError(e) => Err(FIRParsingError::ParseIntError(e)),
            FIRParsingError::IoError(e) => Err(FIRParsingError::IoError(e)),
            FIRParsingError::EOFError => Ok(FIRParsingError::EOFError),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Point {
    pub lat: Decimal,
    pub lon: Decimal,
}

impl Serialize for Point {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        (self.lon, self.lat).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Point {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let vals = <(Decimal, Decimal)>::deserialize(deserializer)?;
        Ok(Self {
            lat: vals.1,
            lon: vals.0,
        })
    }
}

impl Point {
    fn validate_range(rng: Decimal, check: Decimal) -> bool {
        (-rng..=rng).contains(&check)
    }

    pub fn new(lat: Decimal, lon: Decimal) -> FIRResult<Self> {
        if Self::validate_range(dec!(90.0), lat) && Self::validate_range(dec!(180.0), lon) {
            Ok(Self { lat, lon })
        } else {
            Err(FIRParsingError::PointOutOfRange(Self { lat, lon }))
        }
    }

    fn to_fir_dat_str(&self) -> String {
        format!("{}|{}", self.lat, self.lon)
    }
}

impl Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}|{}", self.lat, self.lon)
    }
}

impl FromStr for Point {
    type Err = FIRParsingError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let fields: Vec<_> = s.split('|').map(str::trim).collect();
        match fields.len() {
            2 => Ok(Point {
                lat: fields[0].parse()?,
                lon: fields[1].parse()?,
            }),
            n => Err(Self::Err::FIRParsing(format!(
                "A point expects 2 fields (lat|lon), got: {}",
                n
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Fill {
    Polygon,
    Hole,
}

pub(crate) fn polygon_or_hole(arr: &[Point]) -> Fill {
    match arr
        .windows(2)
        .map(|v| v[0].lon * v[1].lat - v[0].lat * v[1].lon)
        .sum::<Decimal>()
        // / dec!(2.0) //not needed as we only look for zero point
    {
        n if n == dec!(0) => panic!("A stait line"),
        s if s.is_sign_negative() => Fill::Polygon,
        s if s.is_sign_positive() => Fill::Hole,
        n => unreachable!("Math is off (are we in imag numbers): {}", n),
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Hash)]
pub struct FIRBoundary {
    pub(crate) id: usize,
    pub icao: String,
    pub is_oseanic: bool,
    pub is_extension: bool,
    pub min_lat: Decimal,
    pub min_lon: Decimal,
    pub max_lat: Decimal,
    pub max_lon: Decimal,
    pub lable: Point,
    pub boundary_corners: Vec<Point>,
}

// format:
// ICAO|IsOceanic|IsExtension|PointCount|MinLat|MinLon|MaxLat|MaxLon|LableLat|LableLon
// 0000|111111111|22222222222|3333333333|444444|555555|666666|777777|88888888|99999999

impl FIRBoundary {
    fn parse_fields<T: BufRead>(
        f: &mut T,
        count: &mut usize,
        mode: Mode,
        linenr: &mut usize,
    ) -> FIRResult<ColResult<Self>> {
        let mut errors = ErrorCollector::new();
        let mut line = String::new();
        f.read_line(&mut line)?;
        *linenr += 1;
        let fields: Vec<_> = line.split('|').map(str::trim).collect();
        if fields.len() != 10 {
            return if f.fill_buf()?.len() == 0 {
                Err(FIRParsingError::EOFError)
            } else {
                Err(FIRParsingError::FIRParsing(format!(
                    "Expected 10 fields, found: {}, values: {:?}",
                    fields.len(),
                    &fields
                )))
            };
        }
        let amount: usize = fields[3].parse()?;
        let v = std::iter::repeat_with(|| -> FIRResult<String> {
            let mut s = String::new();
            f.read_line(&mut s)?;
            Ok(s)
        })
        .take(amount)
        .map(|r| r.and_then(|s| Point::from_str(&s)))
        .collect::<Result<Vec<_>, _>>()?;
        let mut fir = Self {
            id: *count,
            icao: fields[0].into(),
            is_oseanic: numstr_to_bool(fields[1]),
            is_extension: numstr_to_bool(fields[2]),
            min_lat: fields[4].parse()?,
            min_lon: fields[5].parse()?,
            max_lat: fields[6].parse()?,
            max_lon: fields[7].parse()?,
            lable: Point::new(fields[8].parse()?, fields[9].parse()?)?,
            boundary_corners: v,
        };
        *count += 1;
        match mode {
            Mode::Strict => {
                if fir.polygon_or_hole() == Fill::Hole {
                    fir.icao.as_str();
                    errors.adderror(FIRParsingError::AirspaceDrawDirection(fir.icao.clone()))?
                }
                let mut boundaries = IndexSet::new();
                let mut duplicates = IndexSet::new();
                for point in &fir.boundary_corners {
                    if !boundaries.insert(point.clone()) {
                        duplicates.insert(point.clone());
                    }
                }
                if duplicates.len() != 0 {
                    errors.adderror(FIRParsingError::DuplicatePointError {
                        points: duplicates,
                        owner: fir.icao.clone(),
                    })?
                }
            }
            Mode::Fix => {
                fir.boundary_corners = fir
                    .boundary_corners
                    .iter()
                    .collect::<IndexSet<_>>()
                    .into_iter()
                    .cloned()
                    .collect_vec();
                if fir.polygon_or_hole() == Fill::Hole {
                    fir.boundary_corners.reverse();
                    fir.icao.as_str();
                    assert!(fir.polygon_or_hole() == Fill::Polygon);
                }
            }
        }
        let (min_lat, max_lat) = fir
            .boundary_corners
            .iter()
            .map(|n| n.lat)
            .minmax()
            .into_option()
            .unwrap();
        let (mut min_lon, mut max_lon) = fir
            .boundary_corners
            .iter()
            .map(|n| n.lon)
            .minmax()
            .into_option()
            .unwrap();
        fix_min_max_lon(&mut min_lon, &mut max_lon);
        match mode {
            Mode::Strict => {
                let wrong = vec![
                    (fir.min_lat, min_lat, "minimum latitude"),
                    (fir.min_lon, min_lon, "minimum longitude"),
                    (fir.max_lat, max_lat, "maximum latitude"),
                    (fir.max_lon, max_lon, "maximum longitude"),
                ]
                .into_iter()
                .filter(|(f, c, _)| f != c)
                .collect_vec();
                if wrong.len() != 0 {
                    errors.adderror(FIRParsingError::WrongMinMax(wrong, fir.icao.clone()))?;
                }
            }
            Mode::Fix => {
                fir.min_lat = min_lat;
                fir.min_lon = min_lon;
                fir.max_lat = max_lat;
                fir.max_lon = max_lon;
            }
        }
        Ok(errors.to_col_result(fir))
    }

    fn to_writer<W>(&self, writer: &mut BufWriter<W>) -> io::Result<()>
    where
        W: Write,
    {
        writeln!(
            writer,
            "{}|{}|{}|{}|{}|{}|{}|{}|{}",
            &self.icao,
            bool_to_num(self.is_oseanic),
            bool_to_num(self.is_extension),
            self.boundary_corners.len(),
            self.min_lat,
            self.min_lon,
            self.max_lat,
            self.max_lon,
            self.lable.to_fir_dat_str(),
        )?;
        self.boundary_corners
            .iter()
            .map(|c| writeln!(writer, "{}", c.to_fir_dat_str()))
            .collect::<Result<Vec<_>, io::Error>>()
            .map(|_| ())
    }

    pub fn polygon_or_hole(&self) -> Fill {
        polygon_or_hole(
            self.boundary_corners
                .iter()
                .cloned()
                .collect_vec()
                .as_slice(),
        )
    }
}

fn numstr_to_bool(a: &str) -> bool {
    match a {
        "0" => false,
        "1" => true,
        _ => panic!("only supports '0' or '1' as values, found: {}", a),
    }
}

fn bool_to_num(b: bool) -> u8 {
    match b {
        true => 1,
        false => 0,
    }
}

pub fn read_file<P: AsRef<Path>>(p: P, mode: Mode) -> FIRResult<ColResult<Vec<FIRBoundary>>> {
    let mut f = BufReader::new(File::open(p)?);
    let mut boundaries = IndexMap::new();
    let mut extentions = IndexMap::new();
    let mut duplicate_firs = IndexMap::new();
    let mut count = 0;
    let mut errors = ErrorCollector::new();
    let mut linenr = 0;
    for b in
        std::iter::repeat_with(move || FIRBoundary::parse_fields(&mut f, &mut count, mode, &mut linenr))
    {
        if let Err(FIRParsingError::EOFError) = b {
            break;
        }
        let b = match b? {
            Ok(v) => v,
            Err(e) => {
                errors.adderrors(e);
                continue;
            }
        };
        match b.is_extension {
            true => {
                extentions
                    .entry(b.icao.clone())
                    .or_insert_with(|| Vec::new())
                    .push(b);
            }
            false => match boundaries.entry((b.icao.clone(), b.is_oseanic)) {
                indexmap::map::Entry::Occupied(_) => {
                    *duplicate_firs
                        .entry((b.icao.clone(), b.is_oseanic))
                        .or_insert(1usize) += 1
                }
                indexmap::map::Entry::Vacant(v) => {
                    v.insert(b);
                }
            },
        }
    }
    if duplicate_firs.len() != 0 {
        errors.adderror(FIRParsingError::MultipleFirs(
            duplicate_firs
                .into_iter()
                .map(|((s, _), v)| (s, v))
                .collect(),
        ))?;
    }
    let mut all = Vec::with_capacity(boundaries.len() + extentions.len());
    for (_, fir) in boundaries {
        all.push(fir);
        let fir = all.last().unwrap();
        match extentions.remove(fir.icao.as_str()) {
            Some(s) => s.into_iter().for_each(|v| all.push(v)),
            None => (),
        }
    }

    if mode == Mode::Strict {
        let wrong_orders: IndexSet<_> = all
            .iter()
            .enumerate()
            .filter(|(_, fir)| fir.is_extension)
            .filter(|(n, fir)| fir.id != *n)
            //.inspect(|(n, fir)| {dbg!(n, fir.id);})
            .map(|(_, fir)| fir.icao.clone())
            .collect();
        if wrong_orders.len() != 0 {
            errors.adderror(FIRParsingError::ExtentionNotAfterFir(wrong_orders))?;
        }
    }

    Ok(errors.to_col_result(all))
}

pub(crate) fn convert_from_geojson(gj: crate::geo_json::GeoJson) -> Vec<FIRBoundary> {
    let data = gj.features;
    data.iter()
        .flat_map(|fir| {
            fir.geometry
                .array
                .iter()
                .map(|n| n.get(0).unwrap())
                .enumerate()
                .map(move |(n, points)| {
                    let mut fir = FIRBoundary {
                        id: fir.properties.id,
                        icao: fir.properties.icao.clone(),
                        is_oseanic: fir.properties.is_oceanic,
                        is_extension: n != 0,
                        min_lat: points.iter().map(|n| n.lat).min().unwrap(),
                        min_lon: points.iter().map(|n| n.lon).min().unwrap(),
                        max_lat: points.iter().map(|n| n.lat).max().unwrap(),
                        max_lon: points.iter().map(|n| n.lon).max().unwrap(),
                        lable: fir.properties.lable.clone(),
                        boundary_corners: points.into_iter().cloned().collect(),
                    };
                    fix_min_max_lon(&mut fir.min_lon, &mut fir.max_lon);
                    fir
                })
        })
        .sorted_unstable_by(|a, b| a.id.cmp(&b.id))
        .collect()
}

fn fix_min_max_lon(min_lon: &mut Decimal, max_lon: &mut Decimal) {
    if *max_lon - *min_lon > dec!(180) {
        std::mem::swap(max_lon, min_lon);
    }
}

pub fn write_to_file<P: AsRef<Path>>(firs: &[FIRBoundary], p: P) -> io::Result<()> {
    let mut file = BufWriter::new(File::create(p.as_ref())?);
    firs.iter()
        .map(|fir| fir.to_writer(&mut file))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fir() {
        let data = read_file("FIRBoundaries.dat", Mode::Fix);
        match data {
            Ok(_) => (),
            Err(e) => panic!("{}", e),
        }
    }
}
