use std::{
    error::Error,
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Write},
    path::Path,
    str::FromStr,
};

use indexmap::IndexSet;
use itertools::Itertools;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

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

    pub fn new(lat: Decimal, lon: Decimal) -> Self {
        if Self::validate_range(dec!(90.0), lat) && Self::validate_range(dec!(180.0), lon) {
            Self { lat, lon }
        } else {
            panic!("range outside of scope")
        }
    }

    fn to_fir_dat_str(&self) -> String {
        format!("{}|{}", self.lat, self.lon)
    }
}

impl FromStr for Point {
    type Err = Box<dyn Error>;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let fields: Vec<_> = s.split('|').map(str::trim).collect();
        match fields.len() {
            2 => Ok(Point {
                lat: fields[0].parse()?,
                lon: fields[1].parse()?,
            }),
            n => Err(format!("expected 2 fields, got: {}", n).into()),
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
        .sum::<Decimal>() / dec!(2.0)
    {
        n if n == dec!(0) => panic!("A stait line"),
        s if s.is_sign_negative() => Fill::Polygon,
        s if s.is_sign_positive() => Fill::Hole,
        n => unreachable!("Math is off (are we in imag numbers): {}", n)
    }
}



#[derive(Debug, Eq, PartialEq, Clone)]
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
    pub boundary_corners: IndexSet<Point>,
}

// format:
// ICAO|IsOceanic|IsExtension|PointCount|MinLat|MinLon|MaxLat|MaxLon|CenterLat|CenterLon
// 0000|111111111|22222222222|3333333333|444444|555555|666666|777777|888888888|999999999

impl FIRBoundary {
    fn parse_fields<T: BufRead>(f: &mut T, n: usize) -> Result<Self, Box<dyn Error>> {
        let mut line = String::new();
        f.read_line(&mut line)?;
        let fields: Vec<_> = line.split('|').map(str::trim).collect();
        if fields.len() != 10 {
            return Err(format!(
                "Expected 10 fields, found: {}, values: {:?}",
                fields.len(),
                &fields
            )
            .into());
        }
        let amount: usize = fields[3].parse()?;
        let v = std::iter::repeat_with(|| {
            let mut s = String::new();
            f.read_line(&mut s).unwrap();
            s
        })
        .take(amount)
        .map(|s| Point::from_str(&s))
        .collect::<Result<IndexSet<_>, Box<dyn Error>>>()?;
        let mut fir = Self {
            id: n,
            icao: fields[0].into(),
            is_oseanic: numstr_to_bool(fields[1]),
            is_extension: numstr_to_bool(fields[2]),
            min_lat: fields[4].parse()?,
            min_lon: fields[5].parse()?,
            max_lat: fields[6].parse()?,
            max_lon: fields[7].parse()?,
            lable: Point::new(fields[8].parse()?, fields[9].parse()?),
            boundary_corners: v,
        };
        if fir.polygon_or_hole() == Fill::Hole {
            fir.boundary_corners.reverse()
        }
        Ok(fir)
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
        polygon_or_hole(self.boundary_corners.iter().cloned().collect_vec().as_slice())
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

pub fn read_file<P: AsRef<Path>>(p: P) -> Result<Vec<FIRBoundary>, Box<dyn Error>> {
    let mut f = BufReader::new(File::open(p)?);
    let mut boundaries = Vec::new();
    let mut count = 0;
    while let Ok(b) = FIRBoundary::parse_fields(&mut f, count) {
        boundaries.push(b);
        count += 1;
    }

    Ok(boundaries)
}

pub(crate) fn convert_from_geojson(gj: crate::geo_json::GeoJson) -> Vec<FIRBoundary> {
    let data = gj.features;
    data.iter()
        .flat_map(|fir| {
            fir.geometry.array.iter().map(|n| n.get(0).unwrap()).enumerate().map(|(n, points)|{
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
                if fir.max_lon - fir.min_lon > dec!(180) {
                    std::mem::swap(&mut fir.max_lon, &mut fir.min_lon);
                }
                fir
            })
        })
        .sorted_unstable_by(|a, b| a.id.cmp(&b.id))
        .collect()
}

pub fn write_to_file<P: AsRef<Path>>(firs: &[FIRBoundary], p: P) -> Result<(), Box<dyn Error>> {
    let mut file = BufWriter::new(File::create(p.as_ref())?);
    firs.iter()
        .map(|fir| fir.to_writer(&mut file))
        .collect::<Result<Vec<_>, _>>()
        .map(|_| ())
        .map_err(|e| e.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fir() {
        let data = read_file("FIRBoundaries.dat");
        match data {
            Ok(_) => (),
            Err(e) => panic!("{}", e),
        }
    }
}
