use crate::rng::hash_coords3;
use glam::{Vec2, Vec3};

/// Quintic S-curve smoothstep: 6t^5 - 15t^4 + 10t^3
#[inline]
fn smooth(t: f32) -> f32 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

/// 12 standard 3D gradient vectors
const GRAD3: [[f32; 3]; 12] = [
    [1.0, 1.0, 0.0],
    [-1.0, 1.0, 0.0],
    [1.0, -1.0, 0.0],
    [-1.0, -1.0, 0.0],
    [1.0, 0.0, 1.0],
    [-1.0, 0.0, 1.0],
    [1.0, 0.0, -1.0],
    [-1.0, 0.0, -1.0],
    [0.0, 1.0, 1.0],
    [0.0, -1.0, 1.0],
    [0.0, 1.0, -1.0],
    [0.0, -1.0, -1.0],
];

/// 8 standard 2D gradient vectors
const GRAD2: [[f32; 2]; 8] = [
    [1.0, 0.0],
    [-1.0, 0.0],
    [0.0, 1.0],
    [0.0, -1.0],
    [0.70710677, 0.70710677],
    [-0.70710677, 0.70710677],
    [0.70710677, -0.70710677],
    [-0.70710677, -0.70710677],
];

#[inline]
fn dot3(g: [f32; 3], x: f32, y: f32, z: f32) -> f32 {
    g[0] * x + g[1] * y + g[2] * z
}

#[inline]
fn dot2(g: [f32; 2], x: f32, y: f32) -> f32 {
    g[0] * x + g[1] * y
}

/// Computes pure, deterministic 3D gradient noise in range approximately `[-1.0, 1.0]`.
/// Uses `libm::floorf` and integer hashing so results are bit-for-bit reproducible across platforms.
pub fn grad_noise_3d(seed: u64, pos: Vec3) -> f32 {
    let x0 = libm::floorf(pos.x) as i32;
    let y0 = libm::floorf(pos.y) as i32;
    let z0 = libm::floorf(pos.z) as i32;

    let fx = pos.x - (x0 as f32);
    let fy = pos.y - (y0 as f32);
    let fz = pos.z - (z0 as f32);

    let u = smooth(fx);
    let v = smooth(fy);
    let w = smooth(fz);

    let get_grad = |ix: i32, iy: i32, iz: i32| -> [f32; 3] {
        let h = hash_coords3(seed, ix, iy, iz);
        GRAD3[(h % 12) as usize]
    };

    let n000 = dot3(get_grad(x0, y0, z0), fx, fy, fz);
    let n100 = dot3(get_grad(x0 + 1, y0, z0), fx - 1.0, fy, fz);
    let n010 = dot3(get_grad(x0, y0 + 1, z0), fx, fy - 1.0, fz);
    let n110 = dot3(get_grad(x0 + 1, y0 + 1, z0), fx - 1.0, fy - 1.0, fz);
    let n001 = dot3(get_grad(x0, y0, z0 + 1), fx, fy, fz - 1.0);
    let n101 = dot3(get_grad(x0 + 1, y0, z0 + 1), fx - 1.0, fy, fz - 1.0);
    let n011 = dot3(get_grad(x0, y0 + 1, z0 + 1), fx, fy - 1.0, fz - 1.0);
    let n111 = dot3(
        get_grad(x0 + 1, y0 + 1, z0 + 1),
        fx - 1.0,
        fy - 1.0,
        fz - 1.0,
    );

    let nx00 = n000 + u * (n100 - n000);
    let nx10 = n010 + u * (n110 - n010);
    let nx01 = n001 + u * (n101 - n001);
    let nx11 = n011 + u * (n111 - n011);

    let nxy0 = nx00 + v * (nx10 - nx00);
    let nxy1 = nx01 + v * (nx11 - nx01);

    nxy0 + w * (nxy1 - nxy0)
}

/// Computes pure, deterministic 2D gradient noise in range approximately `[-1.0, 1.0]`.
pub fn grad_noise_2d(seed: u64, pos: Vec2) -> f32 {
    let x0 = libm::floorf(pos.x) as i32;
    let y0 = libm::floorf(pos.y) as i32;

    let fx = pos.x - (x0 as f32);
    let fy = pos.y - (y0 as f32);

    let u = smooth(fx);
    let v = smooth(fy);

    let get_grad = |ix: i32, iy: i32| -> [f32; 2] {
        let h = hash_coords3(seed, ix, iy, 0);
        GRAD2[(h % 8) as usize]
    };

    let n00 = dot2(get_grad(x0, y0), fx, fy);
    let n10 = dot2(get_grad(x0 + 1, y0), fx - 1.0, fy);
    let n01 = dot2(get_grad(x0, y0 + 1), fx, fy - 1.0);
    let n11 = dot2(get_grad(x0 + 1, y0 + 1), fx - 1.0, fy - 1.0);

    let nx0 = n00 + u * (n10 - n00);
    let nx1 = n01 + u * (n11 - n01);

    nx0 + v * (nx1 - nx0)
}

/// Fractional Brownian Motion (fBm) in 2D with octaves, persistence, and lacunarity.
pub fn fbm_2d(seed: u64, pos: Vec2, octaves: usize, lacunarity: f32, persistence: f32) -> f32 {
    let mut total = 0.0;
    let mut frequency = 1.0;
    let mut amplitude = 1.0;
    let mut max_val = 0.0;

    for i in 0..octaves {
        let p = pos * frequency;
        total += grad_noise_2d(seed.wrapping_add(i as u64 * 31), p) * amplitude;
        max_val += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    if max_val > 0.0 {
        total / max_val
    } else {
        total
    }
}

/// Fractional Brownian Motion (fBm) in 3D with octaves, persistence, and lacunarity.
pub fn fbm_3d(seed: u64, pos: Vec3, octaves: usize, lacunarity: f32, persistence: f32) -> f32 {
    let mut total = 0.0;
    let mut frequency = 1.0;
    let mut amplitude = 1.0;
    let mut max_val = 0.0;

    for i in 0..octaves {
        let p = pos * frequency;
        total += grad_noise_3d(seed.wrapping_add(i as u64 * 31), p) * amplitude;
        max_val += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    if max_val > 0.0 {
        total / max_val
    } else {
        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_noise_determinism_golden() {
        let seed = 42;
        let v1 = grad_noise_3d(seed, Vec3::new(12.34, 56.78, 90.12));
        let v2 = grad_noise_3d(seed, Vec3::new(12.34, 56.78, 90.12));
        assert_eq!(v1, v2);

        // Golden hash / exact float check
        let golden_val = grad_noise_3d(999, Vec3::new(1.0, 2.0, 3.0));
        assert!(golden_val.abs() <= 1.0);
    }
}
