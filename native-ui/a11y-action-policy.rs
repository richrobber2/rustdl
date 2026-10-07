//! Generation gate for queued Android semantic actions; contains no node or media data.
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
#[derive(Default)]
pub struct Gate { active: AtomicBool, generation: AtomicU64 }
impl Gate {
    pub fn generation(&self) -> u64 {self.generation.load(Ordering::Acquire)}
    pub fn invalidate(&self) {self.generation.fetch_add(1,Ordering::AcqRel);}
    pub fn set_active(&self,active:bool) {
        self.active.store(false,Ordering::Release);self.invalidate();self.active.store(active,Ordering::Release);
    }
    pub fn current(&self,expected:u64)->bool {self.active.load(Ordering::Acquire)&&self.generation()==expected}
}
static GATE: Gate=Gate {active:AtomicBool::new(false),generation:AtomicU64::new(0)};
pub fn generation()->u64 {GATE.generation()}
pub fn invalidate() {GATE.invalidate();}
pub fn set_active(active:bool) {GATE.set_active(active);}
pub fn current(expected:u64)->bool {GATE.current(expected)}
#[test]
fn queued_actions_require_current_active_generation() {
    let gate=Gate::default();assert!(!gate.current(gate.generation()));
    gate.set_active(true);let queued=gate.generation();assert!(gate.current(queued));
    gate.invalidate();assert!(!gate.current(queued));let private=gate.generation();assert!(gate.current(private));
    gate.set_active(false);assert!(!gate.current(private));
    gate.set_active(true);assert!(!gate.current(private));assert!(gate.current(gate.generation()));
}
