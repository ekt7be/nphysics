use std::num::One;
use kiss3d::window;
use kiss3d::object::Object;
use nalgebra::na::{Vec3, Transformation, Rotation};
use nalgebra::na;
use nphysics::aliases::dim3;
use engine::SceneNode;

struct Cylinder {
    priv color:      Vec3<f32>,
    priv base_color: Vec3<f32>,
    priv delta:      dim3::Transform3d<f32>,
    priv gfx:        Object,
    priv body:       @mut dim3::Body3d<f32>,
}

impl Cylinder {
    pub fn new(body:   @mut dim3::Body3d<f32>,
               delta:  dim3::Transform3d<f32>,
               r:     f32,
               h:     f32,
               color:  Vec3<f32>,
               window: &mut window::Window) -> Cylinder {
        let mut realign: dim3::Transform3d<f32> = One::one();
        let _frac_pi_2: f32 = Real::frac_pi_2();
        realign.append_rotation(&Vec3::new(0.0f32, 0.0, -_frac_pi_2));

        let mut res = Cylinder {
            color:      color,
            base_color: color,
            delta: delta * realign,
            gfx:   window.add_cylinder(h as f32, r as f32),
            body:  body
        };
        res.gfx.set_color(color.x, color.y, color.z);
        res.update();

        res
    }
}

impl SceneNode for Cylinder {
    fn select(&mut self) {
        self.color = Vec3::x();
    }

    fn unselect(&mut self) {
        self.color = self.base_color;
    }

    fn update(&mut self) {
        let rb = self.body.to_rigid_body_or_fail();
        if rb.is_active() {
            self.gfx.set_transformation(na::transformation(rb) * self.delta);
            self.gfx.set_color(self.color.x, self.color.y, self.color.z);
        }
        else {
            self.gfx.set_color(self.color.x * 0.25, self.color.y * 0.25, self.color.z * 0.25);
        }
    }

    fn object<'r>(&'r self) -> &'r Object {
        &self.gfx
    }
}
