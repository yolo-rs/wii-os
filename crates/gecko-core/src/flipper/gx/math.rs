#[derive(Clone, Copy, Debug)]
pub struct Vec3(pub f32, pub f32, pub f32);

impl Vec3 {
    pub fn dot(self, other: Vec3) -> f32 {
        self.0 * other.0 + self.1 * other.1 + self.2 * other.2
    }

    pub fn length(self) -> f32 {
        self.dot(self).sqrt()
    }

    pub fn normalize(self) -> Vec3 {
        let len = self.length();
        if len < 1e-10 {
            Vec3(0.0, 0.0, 0.0)
        } else {
            Vec3(self.0 / len, self.1 / len, self.2 / len)
        }
    }

    pub fn transform(self, m: &[f32; 9]) -> Vec3 {
        Vec3(
            m[0] * self.0 + m[1] * self.1 + m[2] * self.2,
            m[3] * self.0 + m[4] * self.1 + m[5] * self.2,
            m[6] * self.0 + m[7] * self.1 + m[8] * self.2,
        )
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Vec3;

    fn sub(self, rhs: Vec3) -> Vec3 {
        Vec3(self.0 - rhs.0, self.1 - rhs.1, self.2 - rhs.2)
    }
}

impl From<[f32; 3]> for Vec3 {
    fn from(a: [f32; 3]) -> Self {
        Vec3(a[0], a[1], a[2])
    }
}

// Based on Dolphin's SafeDivide
pub fn saturating_div(n: f32, d: f32) -> f32 {
    if d.abs() < 1e-10 { 0.0 } else { n / d }
}

pub fn unpack_rgba(packed: u32) -> [f32; 4] {
    [
        ((packed >> 24) & 0xFF) as f32 / 255.0,
        ((packed >> 16) & 0xFF) as f32 / 255.0,
        ((packed >> 8) & 0xFF) as f32 / 255.0,
        (packed & 0xFF) as f32 / 255.0,
    ]
}
