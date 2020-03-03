// use std::collections::HashSet;

// Scheduler
//  - set of cowns
//  - set of pending behaviours (pairs of cowns and lambdas)

#[macro_use]
extern crate lazy_static;

use std::thread;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::iter::FromIterator;

use std::cell::RefCell;

use std::sync::Mutex;

use std::sync::Arc;

use std::cell::UnsafeCell;

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
        Pending {
            required: HashSet::from_iter(required.iter().cloned()),
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

fn end() {
    while true {
        let handle = match SCHEDULER.lock() {
            Ok(mut scheduler) => {
                scheduler.handles.pop_front()
            }
            Err(_) => panic!("Failed to lock scheduler")
        };
        match handle {
            Some(handle) => { handle.join(); }
            None => { return; }
        }
    }
}

struct Cown<T> {
    data: RefCell<T>, // UnsafeCell
}
impl <T: Send> Cown<T> where {
    pub fn create(data: T) -> Arc<Cown<T>> {
        let cown = Arc::new(Cown { data: RefCell::new(data) });
        register(Cown::address(&cown));
        cown
    }

    fn address(cown: &Arc<Cown<T>>) -> CownAddress {
        CownAddress (&**cown as *const Cown<T> as *const ())
    }
}
unsafe impl <T> Send for Cown<T> {}
unsafe impl <T> Sync for Cown<T> {}

pub trait When<F> {
    fn when(&self, f: F);
}

impl <T: Send + 'static, F: FnOnce(&mut T) + Send + 'static> When<F> for Arc<Cown<T>> {
    fn when(&self, f: F) {
        let cown = Arc::clone(&self);
        let behaviour = Box::new(move || { f(&mut cown.data.borrow_mut()); });
        schedule(&[Cown::address(self)], behaviour);
    }
}

impl <T: Send + 'static, U: Send + 'static, F: FnOnce(&mut T, &mut U) + Send + 'static> 
    When<F> for (Arc<Cown<T>>, Arc<Cown<U>>) {
    fn when(&self, f: F) {
        let (c1 , c2) = (Arc::clone(&self.0), Arc::clone(&self.1));
        let behaviour = Box::new(move ||
            { f(&mut c1.data.borrow_mut(), &mut c2.data.borrow_mut()); });
        schedule(&[Cown::address(&self.0), Cown::address(&self.1)], behaviour);
    }
}

fn main() {
    let c1 = Cown::create(1);
    let c2 = Cown::create(2);

    (c1).when(|c1| {
        *c1 = *c1 + 1;
    });

    (c1).when(|c1| {
        println!("c1 is: {}", c1);
    });

    (c2).when(|c2| {
        println!("c2 is: {}", c2);
    });

    (c1, c2).when(|c1, c2| {
        println!("c1 + c2 is: {}", *c1 + *c2);
    });

    end();
}
