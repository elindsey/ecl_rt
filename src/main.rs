use rand::prelude::*;
use std::{
    f32::consts::PI,
    ops::{Add, AddAssign, Mul, MulAssign, Sub},
    time::Instant,
};

#[derive(Debug, Copy, Clone, PartialEq)]
struct V3(f32, f32, f32);

impl V3 {
    fn dot(self, other: V3) -> f32 {
        self.0 * other.0 + self.1 * other.1 + self.2 * other.2
    }

    fn cross(self, other: V3) -> V3 {
        V3(
            self.1 * other.2 - self.2 * other.1,
            self.2 * other.0 - self.0 * other.2,
            self.0 * other.1 - self.1 * other.0,
        )
    }

    fn normalize(self) -> V3 {
        self * (1.0 / self.len())
    }

    fn reflect(self, normal: V3) -> V3 {
        self - normal * self.dot(normal) * 2.0
    }

    fn len(self) -> f32 {
        self.dot(self).sqrt()
    }

    fn is_unit_vector(self) -> bool {
        // TODO epsilon might be too small?
        (self.dot(self) - 1.0).abs() < f32::EPSILON
    }
}

impl Add for V3 {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0, self.1 + other.1, self.2 + other.2)
    }
}

impl Add<f32> for V3 {
    type Output = Self;

    fn add(self, rhs: f32) -> Self {
        Self(self.0 + rhs, self.1 + rhs, self.2 + rhs)
    }
}

impl AddAssign for V3 {
    fn add_assign(&mut self, other: Self) {
        *self = Self(self.0 + other.0, self.1 + other.1, self.2 + other.2)
    }
}

impl Sub for V3 {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0, self.1 - other.1, self.2 - other.2)
    }
}

impl Sub<f32> for V3 {
    type Output = Self;

    fn sub(self, rhs: f32) -> Self {
        Self(self.0 - rhs, self.1 - rhs, self.2 - rhs)
    }
}

impl Mul for V3 {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        Self(self.0 * other.0, self.1 * other.1, self.2 * other.2)
    }
}

impl Mul<f32> for V3 {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self {
        Self(self.0 * rhs, self.1 * rhs, self.2 * rhs)
    }
}

impl MulAssign<f32> for V3 {
    fn mul_assign(&mut self, rhs: f32) {
        *self = Self(self.0 * rhs, self.1 * rhs, self.2 * rhs)
    }
}

#[derive(Debug)]
struct Camera {
    origin: V3,
    x: V3,
    y: V3,
    z: V3,
    viewport_lower_left: V3,
    viewport_width: f32,
    viewport_height: f32,
}

impl Camera {
    fn new(look_from: V3, look_at: V3, aspect_ratio: f32) -> Camera {
        assert!(aspect_ratio > 1.0, "width > height only");

        let origin = look_from - look_at;
        let z = origin.normalize();
        let x = V3(0.0, 0.0, 1.0).cross(z).normalize();
        let y = z.cross(x).normalize();

        let viewport_height = 1.0;
        let viewport_width = viewport_height * aspect_ratio;
        let viewport_lower_left = origin - z - y * 0.5 * viewport_height - x * 0.5 * viewport_width;

        Camera {
            origin,
            x,
            y,
            z,
            viewport_lower_left,
            viewport_width,
            viewport_height,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum MaterialType {
    Diffuse,
    Specular,
}

#[derive(Debug, Clone, PartialEq)]
struct Material {
    emit_color: V3,
    reflect_color: V3,
    t: MaterialType,
}

struct Sphere {
    p: V3,
    r: f32,
    inv_r: f32,
    m: Material,
}

impl Sphere {
    fn new(p: V3, r: f32, m: Material) -> Sphere {
        Sphere {
            p,
            r,
            inv_r: 1.0 / r,
            m,
        }
    }
}

// https://entropymine.com/imageworsener/srgbformula/
fn linear_to_srgb(x: f32) -> f32 {
    if x < 0.0 {
        0.0
    } else if x > 1.0 {
        1.0
    } else if x > 0.0031308 {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    } else {
        x * 12.92
    }
}

// Algorithm "xor" from p. 4 of Marsaglia, "Xorshift RNGs"
fn xorshift(state: &mut u32) -> u32 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    *state = x;
    x
}

fn randf01(state: &mut u32) -> f32 {
    let randu = (xorshift(state) >> 9) | 0x3f800000;
    let randf = unsafe { std::mem::transmute::<u32, f32>(randu) } - 1.0;
    randf
}

fn randf_range(state: &mut u32, min: f32, max: f32) -> f32 {
    min + (max - min) * randf01(state)
}

fn cast(
    bg: &Material,
    spheres: &Vec<Sphere>,
    origin: V3,
    dir: V3,
    bounces: u32,
    rng_state: &mut u32,
) -> V3 {
    //assert!(dir.is_unit_vector());
    let mut hit_dist = f32::MAX;
    let mut hit_material = bg;
    let mut hit_p = V3(0.0, 0.0, 0.0);
    let mut hit_normal = V3(0.0, 0.0, 0.0);
    let tolerance = 0.0001;

    for s in spheres {
        let sphere_relative_origin = origin - s.p;
        let b = dir.dot(sphere_relative_origin);
        let c = sphere_relative_origin.dot(sphere_relative_origin) - s.r * s.r;
        let discr = b * b - c;

        if discr > 0.0 {
            // at least one real root, meaning we've hit the sphere
            let root_term = discr.sqrt();
            if root_term > tolerance {
                // Order here matters. root_term is positive; b may be positive or negative
                //
                // If b is negative, -b is positive, so -b + root_term is _more_ positive than -b - root_term
                // Thus we check -b - root_term first; if it's negative, we check -b + root_term. This is why -b - root_term
                // must be first.
                //
                // Second case is less interesting
                // If b is positive, -b is negative, so -b - root_term is more negative and we will then check -b + root_term
                let t = -b - root_term; // -b minus pos
                if t > tolerance && t < hit_dist {
                    hit_dist = t;
                    hit_material = &s.m;
                    hit_p = origin + dir * hit_dist;
                    // normalize with mulf by 1/s->r, b/c length of that vector is the radius
                    hit_normal = (hit_p - s.p) * s.inv_r;
                    continue;
                }
                let t = -b + root_term; // -b plus pos
                if t > tolerance && t < hit_dist {
                    hit_dist = t;
                    hit_material = &s.m;
                    hit_p = origin + dir * hit_dist;
                    // normalize with mulf by 1/s->r, b/c length of that vector is the radius
                    hit_normal = (hit_p - s.p) * s.inv_r;
                    continue;
                }
            }
        }
    }

    if hit_material != bg {
        if bounces > 0 {
            let new_dir = match hit_material.t {
                MaterialType::Specular => dir.reflect(hit_normal),
                MaterialType::Diffuse => {
                    let a = randf_range(rng_state, 0.0, 2.0 * PI);
                    let z = randf_range(rng_state, -1.0, 1.0f32); // technically should be [-1, 1], but close enough
                    let r = (1.0 - z * z).sqrt();
                    V3(r * a.cos(), r * a.sin(), z)
                }
            };

            hit_material.emit_color
                + hit_material.reflect_color
                    * cast(bg, spheres, hit_p, new_dir, bounces - 1, rng_state)
        } else {
            hit_material.emit_color
        }
    } else {
        bg.emit_color
    }
}

fn main() {
    // Materials
    let bg = Material {
        emit_color: V3(0.3, 0.4, 0.8),
        reflect_color: V3(0.0, 0.0, 0.0),
        t: MaterialType::Specular,
    };
    let ground = Material {
        emit_color: V3(0.0, 0.0, 0.0),
        reflect_color: V3(0.5, 0.5, 0.5),
        t: MaterialType::Diffuse,
    };
    let left = Material {
        emit_color: V3(0.0, 0.0, 0.0),
        reflect_color: V3(1.0, 0.0, 0.0),
        t: MaterialType::Specular,
    };
    let center = Material {
        emit_color: V3(0.4, 0.8, 0.9),
        reflect_color: V3(0.8, 0.8, 0.8),
        t: MaterialType::Specular,
    };
    let right = Material {
        emit_color: V3(0.0, 0.0, 0.0),
        reflect_color: V3(0.95, 0.95, 0.95),
        t: MaterialType::Specular,
    };

    let spheres = vec![
        Sphere::new(V3(0.0, 0.0, -100.0), 100.0, ground),
        Sphere::new(V3(0.0, 0.0, 1.0), 1.0, center),
        Sphere::new(V3(-2.0, -3.0, 1.5), 0.3, right.clone()),
        Sphere::new(V3(-3.0, -6.0, 0.0), 0.3, right.clone()),
        Sphere::new(V3(-3.0, -5.0, 2.0), 0.5, left),
        Sphere::new(V3(3.0, -3.0, 0.8), 1.0, right),
    ];

    let width = 1920;
    let height = 1080;
    let rays_per_pixel = 100;
    let inv_rays_per_pixels = 1.0 / rays_per_pixel as f32;
    let pixel_width = 3;
    let mut pixels: Vec<u8> = vec![0; width * height * pixel_width];
    let cam = Camera::new(
        V3(0.0, -10.0, 1.0),
        V3(0.0, 0.0, 0.0),
        width as f32 / height as f32,
    );

    // TODO: test this as an iteration over pixels, may elide bounds checking
    let start = Instant::now();
    let mut rng_state = rand::thread_rng().next_u32();
    for image_y in 0..height {
        for image_x in 0..width {
            let inv_height = 1.0 / (height as f32 - 1.0);
            let inv_width = 1.0 / (width as f32 - 1.0);

            let mut color = V3(0.0, 0.0, 0.0);
            for _ in 0..rays_per_pixel {
                // calculate ratio we've moved along the image (y/height), step proportionally within the viewport
                let rand_x: f32 = randf01(&mut rng_state);
                let rand_y: f32 = randf01(&mut rng_state);
                let viewport_y =
                    cam.y * cam.viewport_height * (image_y as f32 + rand_y) * inv_height;
                let viewport_x = cam.x * cam.viewport_width * (image_x as f32 + rand_x) * inv_width;
                let viewport_p = cam.viewport_lower_left + viewport_x + viewport_y;

                // remember that a pixel in float-space is a _range_. We want to send multiple rays within that range
                // to do this we take the start of that range (what we calculated as the image projecting onto our viewport),
                // then add a random [0,1) float
                let ray_p = cam.origin;
                let ray_dir = (viewport_p - cam.origin).normalize();
                color += cast(&bg, &spheres, ray_p, ray_dir, 8, &mut rng_state);
            }

            color *= inv_rays_per_pixels;

            pixels[image_y * width * pixel_width + image_x * pixel_width + 0] =
                (255.0 * linear_to_srgb(color.0)) as u8;
            pixels[image_y * width * pixel_width + image_x * pixel_width + 1] =
                (255.0 * linear_to_srgb(color.1)) as u8;
            pixels[image_y * width * pixel_width + image_x * pixel_width + 2] =
                (255.0 * linear_to_srgb(color.2)) as u8;
        }
        //println!("height {}", image_y);
    }
    println!("computation took {}ms", start.elapsed().as_millis());

    // TODO: bug, image is upside down
    let start = Instant::now();
    image::save_buffer(
        "out.png",
        &pixels,
        width as u32,
        height as u32,
        image::ColorType::Rgb8,
    )
    .unwrap();
    println!("writing file took {}ms", start.elapsed().as_millis());

    println!("Fin.");
}
