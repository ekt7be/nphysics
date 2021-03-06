use std::ptr;
use ncollide::util::hash_map::HashMap;
use ncollide::util::hash::UintTWHash;
use object::{Body, RB, SB};
use integration::Integrator;
use signal::signal::{SignalEmiter, BodyActivationSignalHandler};
use aliases::traits::{NPhysicsScalar, NPhysicsDirection, NPhysicsOrientation, NPhysicsTransform, NPhysicsInertia};

pub struct BodyDamping<N, LV, AV, M, II> {
    priv linear_damping:  N,
    priv angular_damping: N,
    priv objects:         HashMap<uint, @mut Body<N, LV, AV, M, II>, UintTWHash>
}

impl<N:  'static + Clone + NPhysicsScalar,
     LV: 'static + Clone + NPhysicsDirection<N, AV>,
     AV: 'static + Clone + NPhysicsOrientation<N>,
     M:  'static + Clone + NPhysicsTransform<LV, AV>,
     II: 'static + Clone + NPhysicsInertia<N, LV, AV, M>>
BodyDamping<N, LV, AV, M, II> {
    #[inline]
    pub fn new<C>(events:          &mut SignalEmiter<N, Body<N, LV, AV, M, II>, C>,
                  linear_damping:  N,
                  angular_damping: N)
                  -> @mut BodyDamping<N, LV, AV, M, II> {
        let res = @mut BodyDamping {
            linear_damping:  linear_damping,
            angular_damping: angular_damping,
            objects:         HashMap::new(UintTWHash::new())
        };

        events.add_body_activation_handler(
            ptr::to_mut_unsafe_ptr(res) as uint,
            res as @mut BodyActivationSignalHandler<Body<N, LV, AV, M, II>, C>
        );

        res
    }
}

impl<N:  Clone + NPhysicsScalar,
     LV: Clone + NPhysicsDirection<N, AV>,
     AV: Clone + NPhysicsOrientation<N>,
     M:  Clone + NPhysicsTransform<LV, AV>,
     II: Clone + NPhysicsInertia<N, LV, AV, M>>
Integrator<N, Body<N, LV, AV, M, II>> for BodyDamping<N, LV, AV, M, II> {
    #[inline]
    fn add(&mut self, o: @mut Body<N, LV, AV, M, II>) {
        self.objects.insert(ptr::to_mut_unsafe_ptr(o) as uint, o);
    }

    #[inline]
    fn remove(&mut self, o: @mut Body<N, LV, AV, M, II>) {
        self.objects.remove(&(ptr::to_mut_unsafe_ptr(o) as uint));
    }

    fn update(&mut self, _: N) {
        for o in self.objects.elements().iter() {
            match *o.value {
                RB(ref mut rb) => {
                    let new_lin = rb.lin_vel() * self.linear_damping;
                    rb.set_lin_vel(new_lin);
                    let new_ang = rb.ang_vel() * self.angular_damping;
                    rb.set_ang_vel(new_ang);
                },
                SB(_) => {
                    fail!("Not yet implemented.")
                }
            }
        }
    }

    #[inline]
    fn priority(&self) -> f64 { 100.0 }
}

impl<N:  Clone + NPhysicsScalar,
     LV: Clone + NPhysicsDirection<N, AV>,
     AV: Clone + NPhysicsOrientation<N>,
     M:  Clone + NPhysicsTransform<LV, AV>,
     II: Clone + NPhysicsInertia<N, LV, AV, M>,
     C>
BodyActivationSignalHandler<Body<N, LV, AV, M, II>, C> for BodyDamping<N, LV, AV, M, II> {
    fn handle_body_activated_signal(&mut self, b: @mut Body<N, LV, AV, M, II>, _: &mut ~[C]) {
        self.add(b)
    }

    fn handle_body_deactivated_signal(&mut self, b: @mut Body<N, LV, AV, M, II>) {
        self.remove(b)
    }
}
