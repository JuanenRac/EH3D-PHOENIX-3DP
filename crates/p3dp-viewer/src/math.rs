//! The little vector and matrix arithmetic the viewer needs, in `f64`: vectors of three numbers and matrices of four by four, row by row, applied to a column of numbers.

pub type Vec3 = [f64; 3];
pub type Mat4 = [[f64; 4]; 4];

pub fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub fn scale(a: Vec3, k: f64) -> Vec3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

pub fn dot(a: Vec3, b: Vec3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

pub fn length(a: Vec3) -> f64 {
    dot(a, a).sqrt()
}

/// The vector with length one; `None` for a vector with no length or that is not a number.
pub fn normalized(a: Vec3) -> Option<Vec3> {
    let l = length(a);
    (l.is_finite() && l > 1e-12).then(|| scale(a, 1.0 / l))
}

pub fn identity() -> Mat4 {
    let mut m = [[0.0; 4]; 4];
    for (i, row) in m.iter_mut().enumerate() {
        row[i] = 1.0;
    }
    m
}

/// `a * b`: the transform `b` first and then `a`.
pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut m = [[0.0; 4]; 4];
    for (i, row) in m.iter_mut().enumerate() {
        for (j, cell) in row.iter_mut().enumerate() {
            *cell = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    m
}

/// The matrix of a camera at `eye` looking at `target`, with `up` upwards (right-handed, the camera looks along its own minus z).
pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Option<Mat4> {
    let f = normalized(sub(target, eye))?;
    let r = normalized(cross(f, up))?;
    let u = cross(r, f);
    Some([
        [r[0], r[1], r[2], -dot(r, eye)],
        [u[0], u[1], u[2], -dot(u, eye)],
        [-f[0], -f[1], -f[2], dot(f, eye)],
        [0.0, 0.0, 0.0, 1.0],
    ])
}

/// A perspective projection with the depth from 0 to 1 (as the graphics cards of today take it).
pub fn perspective(fov_y_rad: f64, aspect: f64, near: f64, far: f64) -> Option<Mat4> {
    if !(fov_y_rad.is_finite()
        && aspect.is_finite()
        && near.is_finite()
        && far.is_finite()
        && fov_y_rad > 1e-3
        && fov_y_rad < 3.0
        && aspect > 1e-6
        && near > 0.0
        && far > near)
    {
        return None;
    }
    let f = 1.0 / (fov_y_rad / 2.0).tan();
    Some([
        [f / aspect, 0.0, 0.0, 0.0],
        [0.0, f, 0.0, 0.0],
        [0.0, 0.0, far / (near - far), (far * near) / (near - far)],
        [0.0, 0.0, -1.0, 0.0],
    ])
}

/// A point transformed by a matrix, as the four numbers of clip space.
pub fn transform(m: &Mat4, p: Vec3) -> [f64; 4] {
    let v = [p[0], p[1], p[2], 1.0];
    let mut out = [0.0; 4];
    for (i, o) in out.iter_mut().enumerate() {
        *o = (0..4).map(|k| m[i][k] * v[k]).sum();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vectors_do_what_they_should() {
        assert_eq!(cross([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]), [0.0, 0.0, 1.0]);
        assert_eq!(dot([1.0, 2.0, 3.0], [4.0, 5.0, 6.0]), 32.0);
        assert!((length([3.0, 4.0, 0.0]) - 5.0).abs() < 1e-12);
        assert_eq!(normalized([0.0, 0.0, 0.0]), None);
        assert_eq!(normalized([f64::NAN, 0.0, 0.0]), None);
        assert_eq!(normalized([0.0, 2.0, 0.0]), Some([0.0, 1.0, 0.0]));
        assert_eq!(
            add([1.0, 1.0, 1.0], scale([1.0, 2.0, 3.0], 2.0)),
            [3.0, 5.0, 7.0]
        );
        assert_eq!(sub([3.0, 5.0, 7.0], [1.0, 1.0, 1.0]), [2.0, 4.0, 6.0]);
    }

    #[test]
    fn matrices_multiply_in_the_order_of_the_transforms() {
        let t = [
            [1.0, 0.0, 0.0, 5.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let s = [
            [2.0, 0.0, 0.0, 0.0],
            [0.0, 2.0, 0.0, 0.0],
            [0.0, 0.0, 2.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        // scale first, then move: (1,0,0) -> (2,0,0) -> (7,0,0)
        let m = mul(&t, &s);
        assert_eq!(transform(&m, [1.0, 0.0, 0.0]), [7.0, 0.0, 0.0, 1.0]);
        assert_eq!(mul(&identity(), &m), m);
    }

    #[test]
    fn a_camera_looks_down_its_minus_z_and_a_degenerate_one_is_refused() {
        let v = look_at([0.0, -10.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]).unwrap();
        // the target is straight ahead: at distance 10 along minus z of the camera, on its axis
        let p = transform(&v, [0.0, 0.0, 0.0]);
        assert!(p[0].abs() < 1e-12 && p[1].abs() < 1e-12 && (p[2] + 10.0).abs() < 1e-12);
        assert!(look_at([0.0, 0.0, 0.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]).is_none());
        // looking straight along the up vector has no right-hand side
        assert!(look_at([0.0, 0.0, 5.0], [0.0, 0.0, 0.0], [0.0, 0.0, 1.0]).is_none());
    }

    #[test]
    fn the_projection_keeps_what_is_between_the_planes_and_refuses_nonsense() {
        let p = perspective(1.0, 1.5, 0.1, 100.0).unwrap();
        let near = transform(&p, [0.0, 0.0, -0.1]);
        let far = transform(&p, [0.0, 0.0, -100.0]);
        assert!(
            (near[2] / near[3]).abs() < 1e-9,
            "depth 0 at the near plane"
        );
        assert!(
            (far[2] / far[3] - 1.0).abs() < 1e-9,
            "depth 1 at the far plane"
        );
        for (fov, asp, n, f) in [
            (0.0, 1.0, 0.1, 10.0),
            (4.0, 1.0, 0.1, 10.0),
            (1.0, 0.0, 0.1, 10.0),
            (1.0, 1.0, 0.0, 10.0),
            (1.0, 1.0, 10.0, 1.0),
            (f64::NAN, 1.0, 0.1, 10.0),
        ] {
            assert!(perspective(fov, asp, n, f).is_none());
        }
    }
}
