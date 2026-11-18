//! 2D geometry: vectors in screen space (y down) and the overlap tests the
//! original got from Slick2D's `Shape.intersects` / `Shape.contains`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct V2 {
    pub x: f32,
    pub y: f32,
}

impl V2 {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Unit vector at `deg` degrees, Slick's `new Vector2f(theta)`.
    pub fn from_deg(deg: f64) -> Self {
        let r = deg.to_radians();
        Self::new(r.cos() as f32, r.sin() as f32)
    }

    pub fn add(self, o: V2) -> V2 {
        V2::new(self.x + o.x, self.y + o.y)
    }

    pub fn sub(self, o: V2) -> V2 {
        V2::new(self.x - o.x, self.y - o.y)
    }

    pub fn scale(self, s: f32) -> V2 {
        V2::new(self.x * s, self.y * s)
    }

    pub fn dot(self, o: V2) -> f32 {
        self.x * o.x + self.y * o.y
    }

    pub fn len2(self) -> f32 {
        self.dot(self)
    }
}

/// Normalises an angle to [0, 360), as Slick's `getTheta` returns it.
pub fn norm_deg(d: f64) -> f64 {
    let r = d.rem_euclid(360.0);
    if r >= 360.0 {
        0.0
    } else {
        r
    }
}

/// A `w` x `h` rectangle centred on `c`, rotated by `rot_deg`, as four corners.
pub fn rect_poly(c: V2, w: f32, h: f32, rot_deg: f64) -> [V2; 4] {
    let (s, co) = rot_deg.to_radians().sin_cos();
    let (s, co) = (s as f32, co as f32);
    let hw = w / 2.0;
    let hh = h / 2.0;
    let corners = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)];
    corners.map(|(x, y)| V2::new(c.x + x * co - y * s, c.y + x * s + y * co))
}

/// An axis-aligned rectangle: `x`, `y` is the top-left corner.
#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Aabb {
    pub fn centered(c: V2, w: f32, h: f32) -> Self {
        Self { x: c.x - w / 2.0, y: c.y - h / 2.0, w, h }
    }

    pub fn poly(&self) -> [V2; 4] {
        [
            V2::new(self.x, self.y),
            V2::new(self.x + self.w, self.y),
            V2::new(self.x + self.w, self.y + self.h),
            V2::new(self.x, self.y + self.h),
        ]
    }

    pub fn center(&self) -> V2 {
        V2::new(self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn contains_point(&self, p: V2) -> bool {
        p.x >= self.x && p.x <= self.x + self.w && p.y >= self.y && p.y <= self.y + self.h
    }
}

fn project(poly: &[V2], axis: V2) -> (f32, f32) {
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for p in poly {
        let d = p.dot(axis);
        lo = lo.min(d);
        hi = hi.max(d);
    }
    (lo, hi)
}

/// Convex polygons overlap (touching, crossing or one inside the other).
pub fn poly_poly(a: &[V2], b: &[V2]) -> bool {
    for poly in [a, b] {
        let n = poly.len();
        for i in 0..n {
            let e = poly[(i + 1) % n].sub(poly[i]);
            let axis = V2::new(-e.y, e.x);
            let (a0, a1) = project(a, axis);
            let (b0, b1) = project(b, axis);
            if a1 < b0 || b1 < a0 {
                return false;
            }
        }
    }
    true
}

fn point_in_poly(p: V2, poly: &[V2]) -> bool {
    let n = poly.len();
    let mut sign = 0.0f32;
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let cross = (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
        if cross != 0.0 {
            if sign == 0.0 {
                sign = cross.signum();
            } else if sign != cross.signum() {
                return false;
            }
        }
    }
    true
}

fn seg_dist2(p: V2, a: V2, b: V2) -> f32 {
    let ab = b.sub(a);
    let l2 = ab.len2();
    let t = if l2 == 0.0 { 0.0 } else { (p.sub(a).dot(ab) / l2).clamp(0.0, 1.0) };
    p.sub(a.add(ab.scale(t))).len2()
}

/// A circle and a convex polygon overlap.
pub fn circle_poly(c: V2, r: f32, poly: &[V2]) -> bool {
    if point_in_poly(c, poly) {
        return true;
    }
    let n = poly.len();
    (0..n).any(|i| seg_dist2(c, poly[i], poly[(i + 1) % n]) <= r * r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlap_basics() {
        let a = rect_poly(V2::new(0.0, 0.0), 10.0, 10.0, 0.0);
        let b = rect_poly(V2::new(9.0, 0.0), 10.0, 10.0, 45.0);
        let c = rect_poly(V2::new(30.0, 0.0), 10.0, 10.0, 0.0);
        assert!(poly_poly(&a, &b));
        assert!(!poly_poly(&a, &c));
        assert!(circle_poly(V2::new(12.0, 0.0), 8.0, &a));
        assert!(!circle_poly(V2::new(20.0, 0.0), 8.0, &a));
        assert!(circle_poly(V2::new(0.0, 0.0), 1.0, &a));
    }

    #[test]
    fn angles() {
        assert_eq!(norm_deg(-90.0), 270.0);
        let up = V2::from_deg(270.0);
        assert!(up.x.abs() < 1e-6 && (up.y + 1.0).abs() < 1e-6);
    }
}
