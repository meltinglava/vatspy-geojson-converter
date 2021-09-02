use std::{
    error::Error,
    fs::File,
    io::{self, BufRead, BufReader, BufWriter, Write},
    str::FromStr,
};

use rust_decimal::Decimal;
use rust_decimal_macros::dec;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Point {
    lat: Decimal,
    lon: Decimal,
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

#[derive(Debug)]
pub struct FIRBoundary {
    pub icao: String,
    pub is_oseanic: bool,
    pub is_extension: bool,
    pub min_lat: Decimal,
    pub min_lon: Decimal,
    pub max_lat: Decimal,
    pub max_lon: Decimal,
    pub center: Point, //think center chagnes where the label are placed.
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

pub fn write_to_file(firs: &[FIRBoundary]) -> Result<(), Box<dyn Error>> {
    let mut file = BufWriter::new(File::create("test_FIRBoundary.dat")?);
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
