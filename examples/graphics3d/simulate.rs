use std::os;
use std::num::{Zero, One};
use extra::time;
use glfw;
use nalgebra::na::{Vec2, Vec3, Translation};
use nalgebra::na;
use kiss3d::window::Window;
use kiss3d::window;
use kiss3d::event;
use ncollide::geom::Geom;
use ncollide::ray;
use ncollide::ray::Ray;
use nphysics::aliases::dim3;
use nphysics::detection::constraint::{RBRB, BallInSocket, Fixed};
use nphysics::detection::joint::fixed::Fixed;
use nphysics::detection::joint::anchor::Anchor;
use nphysics::object::{RigidBody, Dynamic, RB};
use engine::{SceneNode, GraphicsManager};

fn usage(exe_name: &str) {
    println("Usage: " + exe_name);
    println("The following keyboard commands are supported:");
    println("    t      - pause/continue the simulation.");
    println("    s      - pause then execute only one simulation step.");
    println("    r      - show/hide a ray centered on the camera, directed toward the camera front axis.");
    println("    1      - launch a ball.");
    println("    2      - launch a cube.");
    println("    3      - launch a fast cube using continuous collision detection.");
    println("    TAB    - switch camera mode (first-person or arc-ball).");
    println("    CTRL + click + drag - select and drag an object using a ball-in-socket joint.");
    println("    SHIFT + click - remove an object.");
    println("    arrows - move around when in first-person camera mode.");
    println("    space  - switch wireframe mode. When ON, the contacts points and normals are displayed.");
}

pub fn simulate(builder: proc(&mut Window, &mut GraphicsManager) -> dim3::BodyWorld3d<f32>) {
    let args = os::args();

    if args.len() > 1 {
        usage(args[0]);
        os::set_exit_status(1);
        return;
    }

    do Window::spawn("nphysics: 3d demo") |window| {
        let mut running    = Running;
        let mut draw_colls = false;
        let mut graphics   = GraphicsManager::new(window);
        let mut physics    = builder(window, &mut graphics);

        let mut ray_to_draw = None;

        let mut cursor_pos = Vec2::new(0.0f32, 0.0);
        let mut grabbed_object: Option<@mut dim3::Body3d<f32>> = None;
        let mut grabbed_object_joint: Option<@mut dim3::Fixed3d<f32>> = None;
        let mut grabbed_object_plane: (Vec3<f32>, Vec3<f32>) = (Zero::zero(), Zero::zero());


        window.set_framerate_limit(Some(60));
        window.set_light(window::StickToCamera);

        window.render_loop(|w| {
            w.poll_events(|w, event| {
                match *event {
                    event::ButtonPressed(_, modifier) => {
                        if modifier.contains(glfw::Shift) {
                            // XXX: huge and uggly code duplication
                            let (pos, dir) = w.unproject(&cursor_pos);
                            let ray = Ray::new(pos, dir);

                            // cast the ray
                            let mut interferences = ~[];
                            physics.cast_ray(&ray, &mut interferences);

                            let mut mintoi = Bounded::max_value();
                            let mut minb   = None;

                            for (b, toi) in interferences.move_iter() {
                                if toi < mintoi {
                                    mintoi = toi;
                                    minb   = Some(b);
                                }
                            }

                            if minb.is_some() {
                                let b = minb.unwrap();
                                if b.can_move() {
                                    physics.remove_body(b);
                                    graphics.remove(w, b);
                                }
                            }

                            false
                        }
                        else if modifier.contains(glfw::Control) {
                            match grabbed_object {
                                Some(rb) => {
                                    for sn in graphics.body_to_scene_node(rb).unwrap().iter() {
                                        sn.unselect()
                                    }
                                },
                                None => { }
                            }

                            // XXX: huge and uggly code duplication
                            let (pos, dir) = w.unproject(&cursor_pos);
                            let ray = Ray::new(pos, dir);

                            // cast the ray
                            let mut interferences = ~[];
                            physics.cast_ray(&ray, &mut interferences);

                            let mut mintoi = Bounded::max_value();
                            let mut minb   = None;

                            for (b, toi) in interferences.move_iter() {
                                if toi < mintoi {
                                    mintoi = toi;
                                    minb   = Some(b);
                                }
                            }

                            if minb.is_some() {
                                let b = minb.unwrap();
                                if b.can_move() {
                                    grabbed_object = Some(b)
                                }
                            }

                            match grabbed_object {
                                Some(b) => {
                                    for sn in graphics.body_to_scene_node(b).unwrap().iter() {
                                        match grabbed_object_joint {
                                            Some(j) => physics.remove_fixed(j),
                                            None    => { }
                                        }

                                        let rb      = b.to_rigid_body_or_fail();
                                        let _1: dim3::Transform3d<f32> = One::one();
                                        let attach2 = na::append_translation(&_1, &(ray.orig + ray.dir * mintoi));
                                        let attach1 = na::inv(&na::transformation(rb.transform_ref())).unwrap() * attach2;
                                        let anchor1 = Anchor::new(Some(minb.unwrap()), attach1);
                                        let anchor2 = Anchor::new(None, attach2);
                                        let joint   = @mut Fixed::new(anchor1, anchor2);
                                        grabbed_object_joint = Some(joint);
                                        grabbed_object_plane = (na::translation(&attach2), -ray.dir);
                                        physics.add_fixed(joint);
                                        // add a joint
                                        sn.select()
                                    }
                                },
                                None => { }
                            }

                            false
                        }
                        else {
                            true
                        }
                    },
                    event::ButtonReleased(_, _) => {
                        match grabbed_object {
                            Some(b) => {
                                for sn in graphics.body_to_scene_node(b).unwrap().iter() {
                                    sn.unselect()
                                }
                            },
                            None => { }
                        }

                        match grabbed_object_joint {
                            Some(j) => physics.remove_fixed(j),
                            None    => { }
                        }

                        grabbed_object       = None;
                        grabbed_object_joint = None;

                        true
                    },
                    event::CursorPos(x, y) => {
                        cursor_pos.x = x as f32;
                        cursor_pos.y = y as f32;

                        // update the joint
                        match grabbed_object_joint {
                            Some(j) => {
                                let (pos, dir) = w.unproject(&cursor_pos);
                                let (ref ppos, ref pdir) = grabbed_object_plane;

                                match ray::plane_toi_with_ray(ppos, pdir, &Ray::new(pos, dir)) {
                                    Some(inter) => {
                                        let _1: dim3::Transform3d<f32> = One::one();
                                        j.set_local2(na::append_translation(&_1, &(pos + dir * inter)))
                                    },
                                    None => { }
                                }

                            },
                            None => { }
                        }

                        w.glfw_window().get_key(glfw::KeyRightShift)       == glfw::Release &&
                            w.glfw_window().get_key(glfw::KeyLeftShift)    == glfw::Release &&
                            w.glfw_window().get_key(glfw::KeyRightControl) == glfw::Release &&
                            w.glfw_window().get_key(glfw::KeyLeftControl)  == glfw::Release
                    },
                    event::KeyReleased(glfw::KeyTab) => {
                        graphics.switch_cameras(w);

                        true
                    },
                    event::KeyReleased(glfw::KeyT) => {
                        if running == Stop {
                            running = Running;
                        }
                        else {
                            running = Stop;
                        }

                        true
                    },
                    event::KeyReleased(glfw::KeyS) => {
                        running = Step;

                        true
                    },
                    event::KeyReleased(glfw::KeySpace) => {
                        draw_colls = !draw_colls;
                        w.set_wireframe_mode(draw_colls);

                        true
                    },
                    event::KeyPressed(glfw::Key1) => {
                        let geom   = Geom::new_ball(0.5f32);
                        let mut rb = RigidBody::new(geom, 4.0f32, Dynamic, 0.3, 0.6);

                        let cam_transfom = w.camera().view_transform();
                        rb.append_translation(&na::translation(&cam_transfom));

                        let front = na::rotate(&cam_transfom, &Vec3::z());

                        rb.set_lin_vel(front * 40.0f32);

                        let body = @mut RB(rb);
                        physics.add_body(body);
                        graphics.add(w, body);

                        true
                    },
                    event::KeyPressed(glfw::Key2) => {
                        let geom   = Geom::new_box(Vec3::new(0.5f32, 0.5, 0.5));
                        let mut rb = RigidBody::new(geom, 4.0f32, Dynamic, 0.3, 0.6);

                        let cam_transform = w.camera().view_transform();
                        rb.append_translation(&na::translation(&cam_transform));

                        let front = na::rotate(&cam_transform, &Vec3::z());

                        rb.set_lin_vel(front * 40.0f32);

                        let body = @mut RB(rb);
                        physics.add_body(body);
                        graphics.add(w, body);

                        true
                    },
                    event::KeyPressed(glfw::Key3) => {
                        let geom   = Geom::new_box(Vec3::new(0.5f32, 0.5f32, 0.5f32));
                        let mut rb = RigidBody::new(geom, 4.0f32, Dynamic, 0.3, 0.6);

                        let cam_transfom = w.camera().view_transform();
                        rb.append_translation(&na::translation(&cam_transfom));

                        let front = na::rotate(&cam_transfom, &Vec3::z());

                        rb.set_lin_vel(front * 400.0f32);

                        let body = @mut RB(rb);
                        physics.add_body(body);
                        physics.add_ccd_to(body, 0.4, 1.0);
                        graphics.add(w, body);

                        true
                    },
                    event::KeyPressed(glfw::KeyR) => {
                        if ray_to_draw.is_some() {
                            ray_to_draw = None;
                        }
                        else {
                            let cam_transform = w.camera().view_transform();
                            let pos           = na::translation(&cam_transform);
                            let front         = na::rotate(&cam_transform, &Vec3::z());

                            ray_to_draw = Some(Ray::new(pos, front));
                        }

                        true
                    },
                    _ => true
                }
            });

            let before = time::precise_time_s();

            if running != Stop {
                physics.step(0.016);
                graphics.draw();
            }

            if running == Step {
                running = Stop;
            }

            if draw_colls {
                draw_collisions(w, &mut physics);
            }

            match ray_to_draw {
                None          => { },
                Some(ref ray) => {
                    // cast a ray
                    let mut interferences = ~[];
                    physics.cast_ray(ray, &mut interferences);

                    let mut mintoi = Bounded::max_value();

                    for (_, toi) in interferences.move_iter() {
                        if toi < mintoi {
                            mintoi = toi;
                        }
                    }

                    w.draw_line(&ray.orig, &(ray.orig + ray.dir * mintoi), &Vec3::x())
                }
            }

            if running != Stop {
                let dt = (time::precise_time_s() - before);
                println(dt.to_str() + "sec (" + (1.0 / dt).to_str() + " fps)");
            }
        })
    }
}

#[deriving(Eq)]
enum RunMode {
    Running,
    Stop,
    Step
}

fn draw_collisions(window: &mut window::Window, physics: &mut dim3::BodyWorld3d<f32>) {
    let mut collisions = ~[];

    for c in physics.world().detectors().iter() {
        c.interferences(&mut collisions);
    }

    for c in collisions.iter() {
        match *c {
            RBRB(_, _, c) => {
                window.draw_line(&c.world1, &c.world2, &Vec3::x());

                let center = (c.world1 + c.world2) / 2.0f32;
                let end    = center + c.normal * 0.4f32;
                window.draw_line(&center, &end, &Vec3::new(0.0, 1.0, 1.0))
            },
            BallInSocket(bis) => {
                window.draw_line(&bis.anchor1_pos(), &bis.anchor2_pos(), &Vec3::y());
            },
            Fixed(f) => {
                // FIXME: draw the rotation too
                window.draw_line(&na::translation(&f.anchor1_pos()), &na::translation(&f.anchor2_pos()), &Vec3::y());
            }
        }
    }
}
