//! Rasterizers: geometry in, the exact list of pixels out. No antialiasing anywhere.

use std::collections::BTreeSet;

use sprite_core::{Point, Rect};

/// Bresenham line, both endpoints included, all octants.
pub fn line(a: Point, b: Point) -> Vec<Point> {
    let (mut x, mut y) = (a.x, a.y);
    let dx = (b.x - a.x).abs();
    let dy = -(b.y - a.y).abs();
    let sx = if a.x < b.x { 1 } else { -1 };
    let sy = if a.y < b.y { 1 } else { -1 };
    let mut err = dx + dy;
    let mut out = Vec::with_capacity((dx.max(-dy) + 1) as usize);
    loop {
        out.push(Point::new(x, y));
        if x == b.x && y == b.y {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
    out
}

/// Connects points with lines, without repeating the shared vertices.
pub fn polyline(points: &[Point], closed: bool) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::new();
    if points.len() == 1 {
        return points.to_vec();
    }
    let mut segments: Vec<(Point, Point)> = points.windows(2).map(|w| (w[0], w[1])).collect();
    if closed && points.len() > 2 {
        segments.push((points[points.len() - 1], points[0]));
    }
    for (a, b) in segments {
        for p in line(a, b) {
            if out.last() != Some(&p) {
                out.push(p);
            }
        }
    }
    if closed && out.len() > 1 && out.first() == out.last() {
        out.pop();
    }
    out
}

/// Removes the corner pixel of every "L" (three pixels where the ends touch diagonally), the
/// classic pixel-perfect stroke cleanup for freehand lines and curves.
pub fn pixel_perfect(points: &[Point]) -> Vec<Point> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut out: Vec<Point> = Vec::with_capacity(points.len());
    let mut i = 0;
    while i < points.len() {
        if i > 0 && i + 1 < points.len() {
            let (prev, cur, next) = (*out.last().unwrap_or(&points[i - 1]), points[i], points[i + 1]);
            let orth = |a: Point, b: Point| (a.x - b.x).abs() + (a.y - b.y).abs() == 1;
            let diag = (prev.x - next.x).abs() == 1 && (prev.y - next.y).abs() == 1;
            if diag && orth(prev, cur) && orth(cur, next) {
                i += 1;
                continue;
            }
        }
        out.push(points[i]);
        i += 1;
    }
    out
}

/// Stamps a square brush of `size` px centered on each point.
pub fn thicken(points: &[Point], size: u32) -> Vec<Point> {
    if size <= 1 {
        return points.to_vec();
    }
    let lo = -((size as i32 - 1) / 2);
    let hi = size as i32 / 2;
    let mut set = BTreeSet::new();
    for p in points {
        for dy in lo..=hi {
            for dx in lo..=hi {
                set.insert((p.y + dy, p.x + dx));
            }
        }
    }
    set.into_iter().map(|(y, x)| Point::new(x, y)).collect()
}

pub fn rect_outline(r: Rect, thickness: u32) -> Vec<Point> {
    if r.is_empty() {
        return Vec::new();
    }
    let t = thickness.max(1) as i32;
    r.points().filter(|p| p.x < r.x + t || p.y < r.y + t || p.x >= r.right() - t || p.y >= r.bottom() - t).collect()
}

pub fn rect_fill(r: Rect) -> Vec<Point> {
    r.points().collect()
}

/// Pixels whose centers fall inside the ellipse inscribed in `r`. Symmetric for odd and even
/// sizes; a 1-px-wide box gives a straight line.
pub fn ellipse_fill(r: Rect) -> Vec<Point> {
    if r.is_empty() {
        return Vec::new();
    }
    let (rx, ry) = (r.w as f64 / 2.0, r.h as f64 / 2.0);
    let (cx, cy) = (r.x as f64 + rx, r.y as f64 + ry);
    let mut out = Vec::new();
    for y in r.y..r.bottom() {
        let ny = (y as f64 + 0.5 - cy) / ry;
        for x in r.x..r.right() {
            let nx = (x as f64 + 0.5 - cx) / rx;
            if nx * nx + ny * ny <= 1.0 + 1e-9 {
                out.push(Point::new(x, y));
            }
        }
    }
    // Very thin boxes may lose every center; keep at least the middle line.
    if out.is_empty() {
        if r.w <= r.h {
            return (r.y..r.bottom()).map(|y| Point::new(r.x + r.w as i32 / 2, y)).collect();
        }
        return (r.x..r.right()).map(|x| Point::new(x, r.y + r.h as i32 / 2)).collect();
    }
    out
}

/// Outline of [`ellipse_fill`]: filled pixels with a 4-neighbor outside. 8-connected, no "L"s.
pub fn ellipse_outline(r: Rect, thickness: u32) -> Vec<Point> {
    let fill = ellipse_fill(r);
    let set: BTreeSet<(i32, i32)> = fill.iter().map(|p| (p.x, p.y)).collect();
    let mut border: BTreeSet<(i32, i32)> = fill
        .iter()
        .filter(|p| [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| !set.contains(&(p.x + dx, p.y + dy))))
        .map(|p| (p.x, p.y))
        .collect();
    for _ in 1..thickness.max(1) {
        let grown: Vec<(i32, i32)> = set
            .iter()
            .filter(|(x, y)| [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dy)| border.contains(&(x + dx, y + dy))))
            .copied()
            .collect();
        border.extend(grown);
    }
    let mut out: Vec<Point> = border.into_iter().map(|(x, y)| Point::new(x, y)).collect();
    out.sort_by_key(|p| (p.y, p.x));
    out
}

/// Circle as an ellipse in the box of side `2r+1` centered on `center`.
pub fn circle_box(center: Point, radius: u32) -> Rect {
    let r = radius as i32;
    Rect::new(center.x - r, center.y - r, (2 * r + 1) as u32, (2 * r + 1) as u32)
}

/// Even-odd scanline fill sampled at pixel centers, plus the polygon edges so thin shapes and
/// vertices are always covered.
pub fn polygon_fill(points: &[Point]) -> Vec<Point> {
    if points.len() < 3 {
        return polyline(points, false);
    }
    let mut set: BTreeSet<(i32, i32)> = BTreeSet::new();
    let min_y = points.iter().map(|p| p.y).min().unwrap_or(0);
    let max_y = points.iter().map(|p| p.y).max().unwrap_or(0);
    let n = points.len();
    for y in min_y..=max_y {
        let sy = y as f64 + 0.5;
        let mut xs: Vec<f64> = Vec::new();
        for i in 0..n {
            let (a, b) = (points[i], points[(i + 1) % n]);
            let (ay, by) = (a.y as f64 + 0.5, b.y as f64 + 0.5);
            if (ay <= sy && by > sy) || (by <= sy && ay > sy) {
                let t = (sy - ay) / (by - ay);
                xs.push(a.x as f64 + 0.5 + t * (b.x - a.x) as f64);
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        for pair in xs.chunks(2) {
            if let [x0, x1] = pair {
                let start = (x0 - 0.5).ceil() as i32;
                let end = (x1 - 0.5).floor() as i32;
                for x in start..=end {
                    set.insert((y, x));
                }
            }
        }
    }
    for p in polyline(points, true) {
        set.insert((p.y, p.x));
    }
    set.into_iter().map(|(y, x)| Point::new(x, y)).collect()
}

/// Curve through control points: 2 points = line, 3 = quadratic Bézier, 4 = cubic Bézier,
/// more = Catmull-Rom spline passing through every point. Cleaned to be pixel-perfect.
pub fn curve(points: &[Point]) -> Vec<Point> {
    let samples: Vec<(f64, f64)> = match points.len() {
        0 => return Vec::new(),
        1 => return points.to_vec(),
        2 => return line(points[0], points[1]),
        3 => {
            let [p0, p1, p2] = [points[0], points[1], points[2]].map(|p| (p.x as f64, p.y as f64));
            sample(estimate_len(points), |t| {
                let u = 1.0 - t;
                (u * u * p0.0 + 2.0 * u * t * p1.0 + t * t * p2.0, u * u * p0.1 + 2.0 * u * t * p1.1 + t * t * p2.1)
            })
        }
        4 => {
            let [p0, p1, p2, p3] = [points[0], points[1], points[2], points[3]].map(|p| (p.x as f64, p.y as f64));
            sample(estimate_len(points), |t| {
                let u = 1.0 - t;
                let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
                (a * p0.0 + b * p1.0 + c * p2.0 + d * p3.0, a * p0.1 + b * p1.1 + c * p2.1 + d * p3.1)
            })
        }
        _ => {
            let pts: Vec<(f64, f64)> = points.iter().map(|p| (p.x as f64, p.y as f64)).collect();
            let mut all = Vec::new();
            for i in 0..pts.len() - 1 {
                let p0 = if i == 0 { pts[0] } else { pts[i - 1] };
                let (p1, p2) = (pts[i], pts[i + 1]);
                let p3 = if i + 2 < pts.len() { pts[i + 2] } else { pts[i + 1] };
                let seg_len = estimate_len(&[points[i], points[i + 1]]);
                all.extend(sample(seg_len, |t| {
                    let (t2, t3) = (t * t, t * t * t);
                    let f = |a: f64, b: f64, c: f64, d: f64| {
                        0.5 * (2.0 * b + (-a + c) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (-a + 3.0 * b - 3.0 * c + d) * t3)
                    };
                    (f(p0.0, p1.0, p2.0, p3.0), f(p0.1, p1.1, p2.1, p3.1))
                }));
            }
            all
        }
    };
    let rounded: Vec<Point> = samples.iter().map(|(x, y)| Point::new(x.round() as i32, y.round() as i32)).collect();
    let mut connected: Vec<Point> = Vec::new();
    for p in rounded {
        match connected.last().copied() {
            Some(last) if last == p => {}
            Some(last) => {
                for q in line(last, p).into_iter().skip(1) {
                    connected.push(q);
                }
            }
            None => connected.push(p),
        }
    }
    pixel_perfect(&connected)
}

fn estimate_len(points: &[Point]) -> usize {
    let len: f64 = points.windows(2).map(|w| (((w[1].x - w[0].x).pow(2) + (w[1].y - w[0].y).pow(2)) as f64).sqrt()).sum();
    (len * 3.0).ceil().clamp(8.0, 20_000.0) as usize
}

fn sample(n: usize, f: impl Fn(f64) -> (f64, f64)) -> Vec<(f64, f64)> {
    (0..=n).map(|i| f(i as f64 / n as f64)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pts(list: &[(i32, i32)]) -> Vec<Point> {
        list.iter().map(|(x, y)| Point::new(*x, *y)).collect()
    }

    #[test]
    fn line_octants_and_endpoints() {
        assert_eq!(line(Point::new(0, 0), Point::new(3, 0)).len(), 4);
        assert_eq!(line(Point::new(0, 0), Point::new(0, -3)).len(), 4);
        let diag = line(Point::new(0, 0), Point::new(3, 3));
        assert_eq!(diag, pts(&[(0, 0), (1, 1), (2, 2), (3, 3)]));
        let steep = line(Point::new(2, 5), Point::new(0, 0));
        assert_eq!(steep.first(), Some(&Point::new(2, 5)));
        assert_eq!(steep.last(), Some(&Point::new(0, 0)));
        assert_eq!(steep.len(), 6);
        // Symmetric: reversing endpoints covers the same pixels count.
        assert_eq!(line(Point::new(0, 0), Point::new(7, 3)).len(), line(Point::new(7, 3), Point::new(0, 0)).len());
    }

    #[test]
    fn pixel_perfect_removes_l_corners() {
        // A jagged staircase becomes a clean diagonal.
        let stroke = pts(&[(0, 0), (1, 0), (1, 1), (2, 1), (2, 2)]);
        assert_eq!(pixel_perfect(&stroke), pts(&[(0, 0), (1, 1), (2, 2)]));
        let straight = pts(&[(0, 0), (1, 0), (2, 0)]);
        assert_eq!(pixel_perfect(&straight), straight);
    }

    #[test]
    fn ellipse_is_symmetric() {
        for (w, h) in [(5, 5), (6, 4), (8, 8), (7, 3), (1, 6), (16, 9)] {
            let fill = ellipse_fill(Rect::new(0, 0, w, h));
            let set: BTreeSet<(i32, i32)> = fill.iter().map(|p| (p.x, p.y)).collect();
            for (x, y) in &set {
                assert!(set.contains(&(w as i32 - 1 - x, *y)), "{w}x{h} mirror x");
                assert!(set.contains(&(*x, h as i32 - 1 - y)), "{w}x{h} mirror y");
                assert!(*x >= 0 && *y >= 0 && *x < w as i32 && *y < h as i32);
            }
            let outline = ellipse_outline(Rect::new(0, 0, w, h), 1);
            assert!(outline.len() <= fill.len());
            assert!(!outline.is_empty());
        }
        // 5x5 circle: the classic shape.
        let circle: BTreeSet<(i32, i32)> = ellipse_fill(Rect::new(0, 0, 5, 5)).iter().map(|p| (p.x, p.y)).collect();
        assert!(!circle.contains(&(0, 0)) && circle.contains(&(2, 0)) && circle.contains(&(2, 2)));
    }

    #[test]
    fn rect_shapes() {
        assert_eq!(rect_outline(Rect::new(0, 0, 4, 3), 1).len(), 10);
        assert_eq!(rect_fill(Rect::new(1, 1, 4, 3)).len(), 12);
        assert_eq!(rect_outline(Rect::new(0, 0, 4, 4), 2).len(), 16);
    }

    #[test]
    fn polygon_covers_triangle() {
        let tri = polygon_fill(&pts(&[(0, 0), (6, 0), (0, 6)]));
        let set: BTreeSet<(i32, i32)> = tri.iter().map(|p| (p.x, p.y)).collect();
        assert!(set.contains(&(0, 0)) && set.contains(&(6, 0)) && set.contains(&(0, 6)));
        assert!(set.contains(&(2, 2)));
        assert!(!set.contains(&(5, 5)));
        // Square polygon equals the filled rectangle.
        let sq = polygon_fill(&pts(&[(0, 0), (3, 0), (3, 3), (0, 3)]));
        assert_eq!(sq.len(), 16);
    }

    #[test]
    fn curves_are_connected_and_hit_endpoints() {
        for ctrl in
            [pts(&[(0, 0), (8, 0), (8, 8)]), pts(&[(0, 8), (2, 0), (10, 0), (12, 8)]), pts(&[(0, 0), (5, 5), (10, 0), (15, 5), (20, 0)])]
        {
            let c = curve(&ctrl);
            assert_eq!(c.first(), ctrl.first());
            assert_eq!(c.last(), ctrl.last());
            for w in c.windows(2) {
                assert!((w[0].x - w[1].x).abs() <= 1 && (w[0].y - w[1].y).abs() <= 1, "gap in curve");
            }
        }
    }

    #[test]
    fn thick_brush() {
        assert_eq!(thicken(&[Point::new(5, 5)], 3).len(), 9);
        assert_eq!(thicken(&[Point::new(5, 5)], 2).len(), 4);
    }
}
