use std::{
    error::Error,
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Write},
    path::Path,
    str::FromStr,
};

use itertools::Itertools;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Point {
    lat: Decimal,
    lon: Decimal,
}

impl Serialize for Point {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        (self.lat, self.lon).serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Point {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let vals = <(Decimal, Decimal)>::deserialize(deserializer)?;
        Ok(Self {
            lat: vals.0,
            lon: vals.1,
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

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct FIRBoundary {
    pub icao: String,
    pub is_oseanic: bool,
    pub is_extension: bool,
    pub min_lat: Decimal,
    pub min_lon: Decimal,
    pub max_lat: Decimal,
    pub max_lon: Decimal,
    pub center: Point,
    pub bondary_corners: Vec<Point>,
}

// format:
// ICAO|IsOceanic|IsExtension|PointCount|MinLat|MinLon|MaxLat|MaxLon|CenterLat|CenterLon
// 0000|111111111|22222222222|3333333333|444444|555555|666666|777777|888888888|999999999

impl FIRBoundary {
    fn parse_fields<T: BufRead>(f: &mut T) -> Result<Self, Box<dyn Error>> {
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
        .collect::<Result<Vec<_>, Box<dyn Error>>>()?;
        Ok(Self {
            icao: fields[0].into(),
            is_oseanic: numstr_to_bool(fields[1]),
            is_extension: numstr_to_bool(fields[2]),
            min_lat: fields[4].parse()?,
            min_lon: fields[5].parse()?,
            max_lat: fields[6].parse()?,
            max_lon: fields[7].parse()?,
            center: Point::new(fields[8].parse()?, fields[9].parse()?),
            bondary_corners: v,
        })
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
            self.bondary_corners.len(),
            self.min_lat,
            self.min_lon,
            self.max_lat,
            self.max_lon,
            self.center.to_fir_dat_str(),
        )?;
        self.bondary_corners
            .iter()
            .map(|c| writeln!(writer, "{}", c.to_fir_dat_str()))
            .collect::<Result<Vec<_>, io::Error>>()
            .map(|_| ())
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

pub fn read_file() -> Result<Vec<FIRBoundary>, Box<dyn Error>> {
    let file = "FIRBoundaries.dat";
    let mut f = BufReader::new(File::open(file)?);
    let mut boundaries = Vec::new();

    while let Ok(b) = FIRBoundary::parse_fields(&mut f) {
        boundaries.push(b)
    }

    Ok(boundaries)
}

pub(crate) fn convert_from_geojson(gj: crate::geo_json::GeoJson) -> Vec<FIRBoundary> {
    let data = gj.features;
    data.iter()
        .map(|n| {
            let points = &n.geometry.array[0];
            let points = &points[..points.len() - 1];
            let mut fir = FIRBoundary {
                icao: n.properties.icao.clone(),
                is_oseanic: n.properties.is_oceanic,
                is_extension: n.properties.is_extension,
                min_lat: points.iter().map(|n| n.lat).min().unwrap(),
                min_lon: points.iter().map(|n| n.lon).min().unwrap(),
                max_lat: points.iter().map(|n| n.lat).max().unwrap(),
                max_lon: points.iter().map(|n| n.lon).max().unwrap(),
                center: n.properties.center.clone(),
                bondary_corners: points.to_owned(),
            };
            if fir.max_lon - fir.min_lon > dec!(180) {
                std::mem::swap(&mut fir.max_lon, &mut fir.min_lon);
            }
            fir
        })
        .sorted_unstable_by(|a, b| match a.icao.as_str().cmp(b.icao.as_str()) {
            std::cmp::Ordering::Equal => a.is_extension.cmp(&b.is_extension),
            n => n,
        })
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
        let data = read_file();
        match data {
            Ok(_) => (),
            Err(e) => panic!("{}", e),
        }
    }
}
