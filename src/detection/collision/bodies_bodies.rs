use std::ptr;
use std::num::{Zero, One};
use std::borrow;
use std::managed;
use nalgebra::na::{
    Translation, Rotate, Rotation, Transform, AbsoluteRotate, Inv,
    Cast, Vec, AlgebraicVecExt, Cross, Dim
};
use nalgebra::na;
use ncollide::geom::AnnotatedPoint;
use ncollide::broad;
use ncollide::bounding_volume::{HasBoundingVolume, AABB};
use ncollide::broad::{InterferencesBroadPhase, BoundingVolumeBroadPhase, RayCastBroadPhase};
use ncollide::narrow::algorithm::johnson_simplex::{RecursionTemplate, JohnsonSimplex};
use ncollide::narrow::{CollisionDetector, GeomGeom};
use ncollide::contact::Contact;
use ncollide::ray::{Ray, RayCastWithTransform};
use object::{Body, RB, SB};
use detection::constraint::{Constraint, RBRB};
use detection::detector::Detector;
use signal::signal::{SignalEmiter, BodyActivationSignalHandler};
use aliases::traits::{NPhysicsScalar, NPhysicsDirection, NPhysicsOrientation, NPhysicsTransform, NPhysicsInertia};

pub enum PairwiseDetector<N, LV, AV, M> {
    GG(GeomGeom<N, LV, AV, M>),
    Unsuported
}

// FIXME: implement CollisionDetector for PairwiseDetector ?
impl<N: Send + Freeze + ApproxEq<N> + Num + Real + Float + Ord + Clone + Algebraic + Cast<f32>,
     LV: 'static + Send + Freeze + AlgebraicVecExt<N> + Cross<AV> + ApproxEq<N> + Translation<LV> +
         Clone + Rotate<LV> + Transform<LV>,
     AV: Vec<N>,
     M:  Send + Freeze + Rotation<AV> + Rotate<LV> + AbsoluteRotate<LV> + Translation<LV> +
         Transform<LV> + Mul<M, M> + Inv + One,
     II>
PairwiseDetector<N, LV, AV, M> {
    fn num_colls(&self) -> uint {
        match *self {
            GG(ref gg) => gg.num_colls(),
            Unsuported => 0
        }
    }
}

struct Dispatcher<N, LV, AV, M, II> {
    simplex: JohnsonSimplex<N, AnnotatedPoint<LV>>
}

impl<N:  Send + Freeze + Clone + Zero + Cast<f32>,
     LV: Send + Freeze + Clone + Zero + Dim,
     AV,
     M:  Send + Freeze,
     II>
Dispatcher<N, LV, AV, M, II> {
    pub fn new() -> Dispatcher<N, LV, AV, M, II> {
        let template = RecursionTemplate::new(na::dim::<LV>());
        let simplex  = JohnsonSimplex::new(template);
        Dispatcher {
            simplex: simplex
        }
    }
}
impl<N:  Clone + NPhysicsScalar,
     LV: Clone + NPhysicsDirection<N, AV>,
     AV: Clone + NPhysicsOrientation<N>,
     M:  Clone + NPhysicsTransform<LV, AV>,
     II: Clone + NPhysicsInertia<N, LV, AV, M>>
     broad::Dispatcher<Body<N, LV, AV, M, II>, PairwiseDetector<N, LV, AV, M>>
for Dispatcher<N, LV, AV, M, II> {
    fn dispatch(&self, a: &Body<N, LV, AV, M, II>, b: &Body<N, LV, AV, M, II>)
        -> PairwiseDetector<N, LV, AV, M> {
        match (a, b) {
            (&RB(ref rb1), &RB(ref rb2)) => {
                GG(GeomGeom::new(rb1.geom(), rb2.geom(), &self.simplex))
            },
            _ => Unsuported
        }
    }

    fn is_valid(&self,
                a: &Body<N, LV, AV, M, II>,
                b: &Body<N, LV, AV, M, II>)
                -> bool {
        if borrow::ref_eq(a, b) {
            return false
        }

        match (a, b) {
            (&RB(ref a), &RB(ref b)) => a.can_move() || b.can_move(),
            _ => true
        }
    }
}


pub struct BodiesBodies<N, LV, AV, M, II, BF> {
    contacts_collector:    ~[Contact<N, LV>],
    // FIXME: this is an useless buffer which accumulate the result of bodies activation.
    // This must exist since there is no way to send an activation message without an accumulation
    // list…
    constraints_collector: ~[Constraint<N, LV, AV, M, II>],
    signals:     @mut SignalEmiter<N, Body<N, LV, AV, M, II>, Constraint<N, LV, AV, M, II>>,
    broad_phase: @mut BF,
    update_bf:   bool
}

impl<N:  'static + Clone + NPhysicsScalar,
     LV: 'static + Clone + NPhysicsDirection<N, AV>,
     AV: 'static + Clone + NPhysicsOrientation<N>,
     M:  'static + NPhysicsTransform<LV, AV>,
     II: 'static + Clone + NPhysicsInertia<N, LV, AV, M>,
     BF: 'static + InterferencesBroadPhase<Body<N, LV, AV, M, II>, PairwiseDetector<N, LV, AV, M>>>
BodiesBodies<N, LV, AV, M, II, BF> {
    pub fn new(events:    @mut SignalEmiter<N, Body<N, LV, AV, M, II>, Constraint<N, LV, AV, M, II>>,
               bf:        @mut BF,
               update_bf: bool) -> @mut BodiesBodies<N, LV, AV, M, II, BF> {
        let res = @mut BodiesBodies {
            contacts_collector:    ~[],
            constraints_collector: ~[],
            signals:               events,
            broad_phase:           bf,
            update_bf:             update_bf
        };

        events.add_body_activation_handler(
            ptr::to_mut_unsafe_ptr(res) as uint,
            res as @mut BodyActivationSignalHandler<Body<N, LV, AV, M, II>, Constraint<N, LV, AV, M, II>>
        );

        res
    }

    fn activate(&mut self,
                body: @mut Body<N, LV, AV, M, II>,
                out:  &mut ~[Constraint<N, LV, AV, M, II>]) {
        self.broad_phase.activate(body, |b1, b2, cd| {
            match *cd {
                GG(ref mut d) => {
                    let rb1 = b1.to_rigid_body_or_fail();
                    let rb2 = b2.to_rigid_body_or_fail();

                    // FIXME: is the update needed? Or do we have enough guarantees to avoid it?
                    d.update(rb1.transform_ref(), rb1.geom(), rb2.transform_ref(), rb2.geom());

                    d.colls(&mut self.contacts_collector);

                    for c in self.contacts_collector.iter() {
                        out.push(RBRB(b1, b2, c.clone()))
                    }

                    self.contacts_collector.clear()
                },
                Unsuported => { }
            }
        })
    }

    fn deactivate(&mut self, body: @mut Body<N, LV, AV, M, II>) {
        self.broad_phase.deactivate(body)
    }
}

impl<N:  'static + Clone + NPhysicsScalar,
     LV: 'static + Clone + NPhysicsDirection<N, AV>,
     AV: 'static + Clone + NPhysicsOrientation<N>,
     M:  'static + NPhysicsTransform<LV, AV>,
     II: 'static + Clone + NPhysicsInertia<N, LV, AV, M>,
     BF: RayCastBroadPhase<LV, Body<N, LV, AV, M, II>>>
BodiesBodies<N, LV, AV, M, II, BF> {
    pub fn interferences_with_ray(&mut self,
                                  ray: &Ray<LV>,
                                  out: &mut ~[(@mut Body<N, LV, AV, M, II>, N)]) {
        let mut bodies = ~[];

        self.broad_phase.interferences_with_ray(ray, &mut bodies);

        for b in bodies.iter() {
            match **b {
                RB(ref rb) => {
                    match rb.geom().toi_with_transform_and_ray(rb.transform_ref(), ray) {
                        None    => { },
                        Some(t) => out.push((*b, t))
                    }
                },
                SB(_) => fail!("Not yet implemented.")
            }
        }
    }
}

impl<N:  'static + Clone + NPhysicsScalar,
     LV: 'static + Clone + NPhysicsDirection<N, AV>,
     AV: 'static + Clone + NPhysicsOrientation<N>,
     M:  'static + NPhysicsTransform<LV, AV>,
     II: 'static + Clone + NPhysicsInertia<N, LV, AV, M>,
     BF: InterferencesBroadPhase<Body<N, LV, AV, M, II>, PairwiseDetector<N, LV, AV, M>> +
         BoundingVolumeBroadPhase<Body<N, LV, AV, M, II>, AABB<N, LV>>>
Detector<N, Body<N, LV, AV, M, II>, Constraint<N, LV, AV, M, II>>
for BodiesBodies<N, LV, AV, M, II, BF> {
    fn add(&mut self, o: @mut Body<N, LV, AV, M, II>) {
        if self.update_bf {
            self.broad_phase.add(o);
        }
    }

    fn remove(&mut self, o: @mut Body<N, LV, AV, M, II>) {
        if !o.is_active() {
            // wake up everybody in contact
            let aabb              = o.bounding_volume();
            let mut interferences = ~[];

            // FIXME: change that to a collision lost event
            self.broad_phase.interferences_with_bounding_volume(&aabb, &mut interferences);

            for i in interferences.iter() {
                if !managed::mut_ptr_eq(o, *i) && !i.is_active() && i.can_move() {
                    self.signals.request_body_activation(*i);
                }
            }
        }

        // remove
        if self.update_bf {
            self.broad_phase.remove(o);
        }
    }

    fn update(&mut self) {
        if self.update_bf {
            self.broad_phase.update();
        }

        self.broad_phase.for_each_pair_mut(|b1, b2, cd| {
            match *cd {
                GG(ref mut d) => {
                    let rb1 = b1.to_rigid_body_or_fail();
                    let rb2 = b2.to_rigid_body_or_fail();

                    let ncols = d.num_colls();

                    d.update(rb1.transform_ref(), rb1.geom(), rb2.transform_ref(), rb2.geom());

                    let new_ncols = d.num_colls();

                    if ncols == 0 && new_ncols != 0 {
                        // collision lost
                        self.signals.emit_collision_started(b1, b2);
                    }
                    else if ncols != 0 && new_ncols == 0 {
                        // collision created
                        self.signals.emit_collision_ended(b1, b2);
                    }
                },
                Unsuported => { }
            }
        })
    }

    fn interferences(&mut self, out: &mut ~[Constraint<N, LV, AV, M, II>]) {
        self.broad_phase.for_each_pair_mut(|b1, b2, cd| {
            match *cd {
                GG(ref mut d) => {
                    d.colls(&mut self.contacts_collector);

                    for c in self.contacts_collector.iter() {
                        out.push(RBRB(b1, b2, c.clone()))
                    }

                    self.contacts_collector.clear()
                },
                Unsuported => { }
            }
        })
    }

    #[inline]
    fn priority(&self) -> f64 { 50.0 }
}

impl<N:  'static + Clone + NPhysicsScalar,
     LV: 'static + Clone + NPhysicsDirection<N, AV>,
     AV: 'static + Clone + NPhysicsOrientation<N>,
     M:  'static + NPhysicsTransform<LV, AV>,
     II: 'static + Clone + NPhysicsInertia<N, LV, AV, M>,
     BF: 'static + InterferencesBroadPhase<Body<N, LV, AV, M, II>, PairwiseDetector<N, LV, AV, M>>>
BodyActivationSignalHandler<Body<N, LV, AV, M, II>, Constraint<N, LV, AV, M, II>> for BodiesBodies<N, LV, AV, M, II, BF> {
    fn handle_body_activated_signal(&mut self,
                                    b:   @mut Body<N, LV, AV, M, II>,
                                    out: &mut ~[Constraint<N, LV, AV, M, II>]) {
        self.activate(b, out)
    }

    fn handle_body_deactivated_signal(&mut self, b: @mut Body<N, LV, AV, M, II>) {
        self.deactivate(b)
    }
}
