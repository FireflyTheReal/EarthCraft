//! Independent great-circle ruler paths; input and drawing stay in frontends.
use crate::math::haversine_km;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RulerPointId {
    pub path: usize,
    pub point: usize,
}

#[derive(Default)]
pub struct Ruler {
    paths: Vec<Vec<(f64, f64)>>,
}

impl Ruler {
    pub fn add_point(&mut self, position: (f64, f64), start_path: bool) {
        if self
            .paths
            .last()
            .is_none_or(|path| start_path && !path.is_empty())
        {
            self.paths.push(Vec::new());
        }
        self.paths.last_mut().unwrap().push(position);
    }
    pub fn paths(&self) -> impl Iterator<Item = &[(f64, f64)]> {
        self.paths.iter().map(Vec::as_slice)
    }
    pub fn points(&self) -> impl Iterator<Item = &(f64, f64)> {
        self.paths.iter().flatten()
    }
    pub fn point_count(&self) -> usize {
        self.paths.iter().map(Vec::len).sum()
    }
    pub fn point_mut(&mut self, id: RulerPointId) -> Option<&mut (f64, f64)> {
        self.paths.get_mut(id.path)?.get_mut(id.point)
    }
    pub fn remove_point(&mut self, id: RulerPointId) {
        if let Some(path) = self.paths.get_mut(id.path) {
            if id.point < path.len() {
                path.remove(id.point);
                // Retain an empty active path so the next click cannot silently
                // reconnect to an older measurement after deleting its last point.
                if path.is_empty() && id.path + 1 < self.paths.len() {
                    self.paths.remove(id.path);
                }
            }
        }
    }
    pub fn clear(&mut self) {
        self.paths.clear();
    }
    pub fn total_distance_km(&self) -> f64 {
        self.paths()
            .flat_map(|path| path.windows(2))
            .map(|pair| haversine_km(pair[0], pair[1]))
            .sum()
    }
    pub fn last_leg(&self) -> Option<[(f64, f64); 2]> {
        let path = self.paths.last()?;
        let pair = path.windows(2).last()?;
        Some([pair[0], pair[1]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disconnected_paths_exclude_gaps_and_start_without_a_bearing() {
        let mut ruler = Ruler::default();
        ruler.add_point((0.0, 0.0), true);
        ruler.add_point((0.0, 1.0), false);
        let first_distance = ruler.total_distance_km();
        ruler.add_point((40.0, 80.0), true);
        assert!(ruler.last_leg().is_none());
        assert_eq!(ruler.total_distance_km(), first_distance);
        ruler.add_point((41.0, 80.0), false);
        assert_eq!(ruler.last_leg(), Some([(40.0, 80.0), (41.0, 80.0)]));
        assert_eq!(
            ruler.total_distance_km(),
            first_distance + haversine_km((40.0, 80.0), (41.0, 80.0))
        );
        assert_eq!(ruler.point_count(), 4);
        *ruler.point_mut(RulerPointId { path: 1, point: 1 }).unwrap() = (42.0, 80.0);
        assert_eq!(ruler.last_leg(), Some([(40.0, 80.0), (42.0, 80.0)]));
    }
    #[test]
    fn deleting_and_clearing_preserve_path_boundaries() {
        let mut ruler = Ruler::default();
        ruler.add_point((0.0, 0.0), false);
        ruler.add_point((0.0, 1.0), false);
        ruler.add_point((20.0, 30.0), true);
        ruler.add_point((21.0, 30.0), false);
        ruler.remove_point(RulerPointId { path: 1, point: 0 });
        assert_eq!(
            ruler.total_distance_km(),
            haversine_km((0.0, 0.0), (0.0, 1.0))
        );
        ruler.remove_point(RulerPointId { path: 0, point: 0 });
        assert_eq!(ruler.total_distance_km(), 0.0);
        assert_eq!(ruler.paths().count(), 2);
        ruler.remove_point(RulerPointId { path: 1, point: 0 });
        assert_eq!(ruler.paths().count(), 2);
        ruler.add_point((50.0, 50.0), false);
        assert_eq!(ruler.total_distance_km(), 0.0);
        ruler.clear();
        assert_eq!(ruler.point_count(), 0);
        assert!(ruler.last_leg().is_none());
        ruler.add_point((10.0, 20.0), false);
        assert_eq!(ruler.paths().count(), 1);
    }
}
