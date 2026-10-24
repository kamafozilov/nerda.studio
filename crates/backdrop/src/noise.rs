//! Smooth value noise, layered into fractal noise: soft shapes that vary at
//! every scale, from a hash of the lattice, so the same point always reads
//! the same value.

/// A lattice point's value in `0..1`.
fn hash(x: i32, y: i32) -> f32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    (h & 0x00ff_ffff) as f32 / 16_777_216.
}

/// Noise in `0..1` between lattice points, eased so the lattice never shows.
fn value(x: f32, y: f32) -> f32 {
    let (xf, yf) = (x.floor(), y.floor());
    let (xi, yi) = (xf as i32, yf as i32);
    let (tx, ty) = (x - xf, y - yf);
    let (u, v) = (tx * tx * (3. - 2. * tx), ty * ty * (3. - 2. * ty));
    let a = hash(xi, yi);
    let b = hash(xi + 1, yi);
    let c = hash(xi, yi + 1);
    let d = hash(xi + 1, yi + 1);
    a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v
}

/// Four octaves of value noise, each twice as fine and half as strong, in
/// `0..1` around 0.5.
pub(crate) fn fractal(mut x: f32, mut y: f32) -> f32 {
    let (mut sum, mut amplitude) = (0., 0.5);
    for _ in 0..4 {
        sum += amplitude * value(x, y);
        x *= 2.03;
        y *= 2.03;
        amplitude *= 0.5;
    }
    sum / 0.9375
}

/// 0 below `low`, 1 above `high`, eased between.
pub(crate) fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = ((x - low) / (high - low)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}
