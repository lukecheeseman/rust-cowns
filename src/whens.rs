// Scheduler
//  - set of cowns
//  - set of pending behaviours (pairs of cowns and lambdas)

use std::thread;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::cell::UnsafeCell;
use std::sync::Mutex;
use std::sync::Arc;

#[derive(PartialEq, Eq, Hash, Clone)]
struct CownAddress ( *const () );
unsafe impl Send for CownAddress {}
unsafe impl Sync for CownAddress {}

type Behaviour = Box<dyn FnOnce() + Send + 'static>;

struct Pending {
    required: HashSet<CownAddress>,
    behaviour: Behaviour,
}
impl Pending {
    fn new(required: &[CownAddress], behaviour: Behaviour) -> Pending {
        let mut addresses = HashSet::new();
        for address in required.iter().cloned() {
            if addresses.contains(&address) {
                panic!("Scheduling behaviour requires aliasing cowns");
            }
            addresses.insert(address);
        }
        Pending {
            required: addresses,
            behaviour: behaviour,
        }
    }
}

struct Scheduler {
    available: HashSet<CownAddress>,
    pending: VecDeque<Pending>,
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

fn schedule(required: &[CownAddress], behaviour: Behaviour) {
    match SCHEDULER.lock() {
        Ok(mut scheduler) => {
            scheduler.pending.push_back(Pending::new(&required, behaviour));
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
                            let behaviour = pending.behaviour;
                            scheduler.handles.push_back(thread::spawn(|| { 
                                behaviour();
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
unsafe impl <T> Send for Cown<T> {}
unsafe impl <T> Sync for Cown<T> {}
impl <T> Clone for Cown<T> {
    fn clone(&self) -> Cown<T> {
        Cown { resource: self.resource.clone() }
    }
}

#[macro_export]
macro_rules! when {
    () => {{ () }};
    ( $c:expr) => {{ ($c.clone()) }};
    ( $( $cs:expr ),* ) => {{ ( ( $( $cs.clone() ),* ) ) }};
}

pub trait Run<F> {
    fn run(&self, function: F);
}
impl <T: Send + 'static, F: FnOnce(&mut T) + Send + 'static>
    Run<F> for Cown<T> {
    fn run(&self, f: F) {
        let resource = self.resource.clone();
        let behaviour = Box::new(move || {
            f(resource.get());
        });
        let address = &[Cown::address(&self)];
        schedule(address, behaviour);
    }
}

impl <T: Send + 'static, U: Send + 'static, F: FnOnce(&mut T, &mut U) + Send + 'static>
    Run<F> for (Cown<T>, Cown<U>) {
    fn run(&self, f: F) {
        let (r1, r2) = (self.0.resource.clone(), self.1.resource.clone());
        let behaviour = Box::new(move || {
            f(r1.get(), r2.get());
        });
        let address = &[Cown::address(&self.0), Cown::address(&self.1)];
        schedule(address, behaviour);
    }
}

impl <T: Send + 'static, U: Send + 'static, V: Send + 'static, W: Send + 'static,
      X: Send + 'static, F: FnOnce(&mut T, &mut U, &mut V, &mut W, &mut X) + Send + 'static>
    Run<F> for (Cown<T>, Cown<U>, Cown<V>, Cown<W>, Cown<X>) {
    fn run(&self, f: F) {
        let (r1, r2, r3, r4, r5) = (self.0.resource.clone(), self.1.resource.clone(),
                                    self.2.resource.clone(), self.3.resource.clone(),
                                    self.4.resource.clone());
        let behaviour = Box::new(move || {
            f(r1.get(), r2.get(), r3.get(), r4.get(), r5.get());
        });
        let address = &[Cown::address(&self.0), Cown::address(&self.1),
                        Cown::address(&self.2), Cown::address(&self.3),
                        Cown::address(&self.4)];
        schedule(address, behaviour);
    }
}
