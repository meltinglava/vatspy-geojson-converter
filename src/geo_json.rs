use std::ops::Deref;

use indexmap::{IndexMap, IndexSet};
use itertools::Itertools;
use rust_decimal::Decimal;
use rust_decimal_macros::dec;
use serde::{Deserialize, Serialize};

use crate::fir_boundaries::{polygon_or_hole, Fill, Point};

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
        Self {
            typ: "FeatureCollection".to_string(),
            name: String::new(),
            crs: Crs::default(),
            features: generate_features(data),
        }
    }
}

fn generate_features<T>(data: T) -> Vec<Feature>
where
    T: Deref<Target = [crate::fir_boundaries::FIRBoundary]>,
{
    let data = data.deref();
    let mut features = IndexSet::new();
    let mut extensions = Vec::new();
    for fir in data {
        if !fir.is_extension {
            assert!(features.insert(fir.into()));
        } else {
            extensions.push(fir.into());
        }
    }
    let mut features = features.into_iter().collect_vec();
    extensions.into_iter().for_each(|e: Feature| {
        match features
            .iter_mut()
            .find(|fir: &&mut Feature| fir.properties.icao == e.properties.icao)
        {
            Some(fir) => fir.geometry.array.push([e.geometry.array[0][0].clone()]),
            None => panic!("Extention FIR without Owning FIR"),
        }
    });
    features
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

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
            geometry: fir.boundary_corners.as_slice().into(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub(crate) struct Properties {
    #[serde(rename = "ICAO")]
    pub(crate) icao: String,
    pub(crate) is_oceanic: bool,
    pub(crate) lable: Point,
}

impl From<&crate::fir_boundaries::FIRBoundary> for Properties {
    fn from(fir: &crate::fir_boundaries::FIRBoundary) -> Self {
        Self {
            icao: fir.icao.clone(),
            is_oceanic: fir.is_oseanic,
            lable: fir.lable.clone(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub(crate) struct Geometry {
    #[serde(rename = "type")]
    typ: String,
    pub(crate) array: Vec<[Vec<Point>; 1]>, // we do not support holes yet.
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
            array: vec![[array]],
        }
    }
}

/* Commented out due to trait rules: Compiler Error [E0119]
impl From<&IndexSet<Point>> for Geometry {
    fn from(source: &IndexSet<Point>) -> Self {
        let mut array = source.iter().cloned().collect_vec();
        array.push(array.first().unwrap().clone());  // ref: https://datatracker.ietf.org/doc/html/rfc7946#section-3.1.6 second point
        Self {
            typ: "MultiPolygon".to_string(),
            array: vec![[array]],
        }
    }
}
 */

impl Geometry {
    fn polygon_or_hole(&self) -> Vec<Fill> {
        self.array[0].iter().map(|s| polygon_or_hole(s)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_geometry() -> Geometry {
        let a = [[1, 1], [0, 2], [1, 3], [2, 2]];
        let arr: Vec<_> = std::array::IntoIter::new(a)
            .map(|v| Point::new(v[1].into(), v[0].into()))
            .collect();
        Geometry {
            typ: "MultiPolygon".to_string(),
            array: [vec![arr]],
        }
    }

    #[test]
    fn test_polygon_or_hole() {
        let g = make_test_geometry();
        assert_eq!(g.polygon_or_hole(), Fill::Polygon)
    }
}
