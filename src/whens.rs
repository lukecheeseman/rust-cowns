// Scheduler
//  - set of cowns
//  - set of pending behaviours (pairs of cowns and lambdas)
//
// Behaviour
//   - Required Cowns
//   - causally related set of behaviours
//
// Causally Constraint <<c
//   - A b1,b2. b2 <<c b1 if b1 in pending b2 in b1.causally-related
//
// Dispatch constrained <<d
//  - A b1,b2. b2 <<d b1 if b2 <<c b1 and b1.required subset b2.required
//
// scheduler
//   findall b1 s.t. b.pending is subset of available
//     schedule first b1 s.t. !E b2

use std::thread;
use std::cell::UnsafeCell;
use std::collections::{HashSet, VecDeque};
use std::sync::{Mutex, Arc};

#[derive(PartialEq, Eq, Hash, Clone)]
struct CownAddress ( *const () );
unsafe impl Send for CownAddress {}
unsafe impl Sync for CownAddress {}

type BehaviourFn = Box<dyn FnOnce() + Send + 'static>;

struct Behaviour {
    required: HashSet<CownAddress>,
    body: BehaviourFn,
}
impl Behaviour {
    fn new(required: &[CownAddress], body: BehaviourFn) -> Behaviour {
        let mut addresses = HashSet::new();
        for address in required.iter().cloned() {
            if addresses.contains(&address) {
                panic!("Scheduling behaviour requires aliasing cowns");
            }
            addresses.insert(address);
        }
        Behaviour {
            required: addresses,
            body: body,
        }
    }
}

struct Scheduler {
    available: HashSet<CownAddress>,
    pending: VecDeque<Behaviour>,
    handles: VecDeque<thread::JoinHandle<()>>,
}
impl Scheduler {
    fn new() -> Scheduler {
        Scheduler {
            available: HashSet::new(),
            pending: VecDeque::new(),
            handles: VecDeque::new(),
        }
    }
}

lazy_static! {
    static ref SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());
}

fn register(cown: CownAddress) {
    match SCHEDULER.lock() {
        Ok(mut scheduler) => {
            scheduler.available.insert(cown);
        }
        Err(_) => panic!("Failed to lock scheduler")
    };
}

fn free(cowns: HashSet<CownAddress>) {
    match SCHEDULER.lock() {
        Ok(mut scheduler) => {
            scheduler.available.extend(cowns);
        }
        Err(_) => panic!("Failed to lock scheduler")
    };
    signal();
}

fn schedule(required: &[CownAddress], behaviour: BehaviourFn) {
    match SCHEDULER.lock() {
        Ok(mut scheduler) => {
            scheduler.pending.push_back(Behaviour::new(&required, behaviour));
        }
        Err(_) => panic!("Failed to lock scheduler")
    };
    signal();
}

fn signal() {
    match SCHEDULER.lock() {
        Ok(mut scheduler) => {
            let index = scheduler.pending.iter().position(|pending| 
                            pending.required.is_subset(&scheduler.available));
            match index {
                Some(index) => {
                    match scheduler.pending.remove(index) {
                        Some(pending) => {
                            for addr in &pending.required {
                                scheduler.available.remove(addr);
                            }
                            let required = pending.required;
                            let body = pending.body;
                            scheduler.handles.push_back(thread::spawn(|| { 
                                body();
                                free(required);
                            }))
                        }
                        None => {}
                    }
                }
                None => {}
            }
        }
        Err(_) => panic!("Failed to lock scheduler")
    };
}

// TODO: remove
pub fn end() {
    loop {
        let handle = match SCHEDULER.lock() {
            Ok(mut scheduler) => {
                scheduler.handles.pop_front()
            }
            Err(_) => panic!("Failed to lock scheduler")
        };
        match handle {
            Some(handle) => { handle.join().unwrap(); }
            None => { return; }
        }
    }
}

struct Resource<T> ( UnsafeCell<T> );
impl <T: Send> Resource<T> {
    fn new(resource: T) -> Resource<T> {
        Resource ( UnsafeCell::new(resource) )
    }

    fn get(&self) -> &mut T {
        unsafe {
            match self.0.get().as_mut() {
                Some(value) => value,
                None => panic!("Empty Resource"),
            }
        }
    }
}
unsafe impl <T> Send for Resource<T> {}
unsafe impl <T> Sync for Resource<T> {}

pub struct Cown<T> {
    resource: Arc<Resource<T>>,
}
impl <T: Send> Cown<T> where {
    pub fn create(resource: T) -> Cown<T> {
        let cown = Cown { resource: Arc::new(Resource::new(resource)) };
        register(Cown::address(&cown));
        cown
    }

    fn address(cown: &Cown<T>) -> CownAddress {
        CownAddress (&*cown.resource as *const Resource<T> as *const ())
    }
}
impl <T> Clone for Cown<T> {
    fn clone(&self) -> Cown<T> {
        Cown { resource: self.resource.clone() }
    }
}

#[macro_export]
macro_rules! when {
    ( $( $cs:expr ),* ) => {{ ( ( $( $cs.clone() ),* ) ) }};
}

pub trait Run<F> {
    fn run(&self, function: F);
}

macro_rules! impl_run_tuple {
    () => (
        impl <Be: FnOnce() + Send + 'static>
            Run<Be> for () {
            fn run(&self, b: Be) {
                let behaviour = Box::new(move || {
                    b();
                });
                schedule(&[], behaviour);
            }
        }
    );

    ( $name:ident ) => (
        impl<$name: Send + 'static, Be: FnOnce( &mut $name ) + Send +'static>
            Run<Be> for Cown<$name> {
            #[allow(non_snake_case)]
            fn run(&self, b: Be) {
                let ref $name = self;
                let addresses = &[ Cown::address( $name ) ];
                let $name = $name.resource.clone();
                let behaviour = Box::new(move || {
                    b($name.get());
                });
                schedule(addresses, behaviour);
            }
        }
    );

    ( $($name:ident)+ ) => (
        impl<$($name: Send + 'static),+, Be: FnOnce( $(&mut $name),+ ) + Send +'static>
            Run<Be> for ($(Cown<$name>,)+) {
            #[allow(non_snake_case)]
            fn run(&self, b: Be) {
                let ( $(ref $name,)+ ) = self;
                let addresses = &[ $( Cown::address( $name ) , )+ ];
                let ( $($name,)+ ) = ( $($name.resource.clone(),)+ );
                let behaviour = Box::new(move || {
                    b($($name.get(),)+ );
                });
                schedule(addresses, behaviour);
            }
        }
    );
}

impl_run_tuple! {}
impl_run_tuple! { A }
impl_run_tuple! { A B }
impl_run_tuple! { A B C }
impl_run_tuple! { A B C D }
impl_run_tuple! { A B C D E }
impl_run_tuple! { A B C D E F }
impl_run_tuple! { A B C D E F G }
impl_run_tuple! { A B C D E F G H }
impl_run_tuple! { A B C D E F G H I }
impl_run_tuple! { A B C D E F G H I J }
impl_run_tuple! { A B C D E F G H I J K }
impl_run_tuple! { A B C D E F G H I J K L }
