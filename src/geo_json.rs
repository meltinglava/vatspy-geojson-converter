use std::ops::Deref;

use indexmap::IndexMap;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use crate::fir_boundaries::Point;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct GeoJson {
    #[serde(rename = "type")]
    typ: String,
    name: String,
    crs: Crs,
    pub(crate) features: Vec<Feature>,
}

impl<T> From<T> for GeoJson
where
    T: Deref<Target = [crate::fir_boundaries::FIRBoundary]>,
{
    fn from(data: T) -> Self {
        let data = data.deref();
        Self {
            typ: "FeatureCollection".to_string(),
            name: String::new(),
            crs: Crs::default(),
            features: data.iter().map(|v| v.into()).collect(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Crs {
    #[serde(rename = "type")]
    typ: String,
    pub(crate) properties: IndexMap<String, String>,
}

impl Default for Crs {
    fn default() -> Self {
        let mut map = IndexMap::new();
        map.insert(
            "name".to_string(),
            "urn:ogc:def:crs:OGC:1.3:CRS84".to_string(),
        );
        Self {
            typ: "name".to_string(),
            properties: map,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Feature {
    #[serde(rename = "type")]
    typ: String,
    pub(crate) properties: Properties,
    pub(crate) geometry: Geometry,
}

impl From<&crate::fir_boundaries::FIRBoundary> for Feature {
    fn from(fir: &crate::fir_boundaries::FIRBoundary) -> Self {
        Self {
            typ: "Feature".to_string(),
            properties: fir.into(),
            geometry: fir.bondary_corners.as_slice().into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct Properties {
    #[serde(rename = "ICAO")]
    pub(crate) id: usize,
    pub(crate) icao: String,
    pub(crate) is_oceanic: bool,
    pub(crate) is_extension: bool,
    pub(crate) lable: Point,
}

impl From<&crate::fir_boundaries::FIRBoundary> for Properties {
    fn from(fir: &crate::fir_boundaries::FIRBoundary) -> Self {
        Self {
            id: fir.id,
            icao: fir.icao.clone(),
            is_oceanic: fir.is_oseanic,
            is_extension: fir.is_extension,
            lable: fir.lable.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Geometry {
    #[serde(rename = "type")]
    typ: String,
    pub(crate) array: [[Vec<Point>; 1]; 1], // might need to do stuff here when crossing 180 east west
}

impl<T> From<T> for Geometry
where
    T: Deref<Target = [Point]>,
{
    fn from(source: T) -> Self {
        let mut array = source.deref().to_vec();
        if array[0] != array[array.len() - 1] {
            array.push(array[0].clone()); // ref: https://datatracker.ietf.org/doc/html/rfc7946#section-3.1.6 second point
        }
        Self {
            typ: "MultiPolygon".to_string(),
            array: [[array]],
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Fill {
    Polygon,
    Hole,
}

impl Geometry {
    fn polygon_or_hole(&self) -> Fill {
        let arr = &self.array[0][0];
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_geometry() -> Geometry {
        let a = [[1, 1], [0, 2], [1, 3], [2, 2]];
        let arr: Vec<_> = std::array::IntoIter::new(a).map(|v| Point::new(v[1].into(), v[0].into())).collect();
        Geometry {
            typ: "MultiPolygon".to_string(),
            array: [[arr]],
        }
    }

    #[test]
    fn test_polygon_or_hole() {
        let g = make_test_geometry();
        assert_eq!(g.polygon_or_hole(), Fill::Polygon)
    }
}
