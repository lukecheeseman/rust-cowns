// use std::collections::HashSet;

// Scheduler
//  - set of cowns
//  - set of pending behaviours (pairs of cowns and lambdas)

#[macro_use]
extern crate lazy_static;
extern crate uuid;

use uuid::Uuid;

use std::thread;
use std::collections::HashMap;
use std::collections::VecDeque;

use std::sync::Mutex;

use std::hash::{Hash, Hasher};

use std::marker::PhantomData;

/*
struct Scheduler {
    cowns: HashMap<Cown, i64>,
//    behaviours: VecDeque<Pending>,
}
impl Scheduler {
    pub fn new() -> Scheduler {
        Scheduler {
            cowns: HashMap::new(),
//            behaviours: VecDeque::new(),
        }
    }
}

lazy_static! {
    static ref SCHEDULER: Mutex<Scheduler> = Mutex::new(Scheduler::new());
}

pub fn when<F: 'static>(cown: &Cown, f: F) where F: FnOnce(&mut i64) + Send {
    let thread = match SCHEDULER.lock() {
        Ok(mut scheduler) => {
            let mut resource = match scheduler.cowns.remove(&cown) {
                Some(resource) => { resource }
                None => { panic!("Failed to get resource") }
            };
            let clone = Cown::clone(&cown);
            thread::spawn(move || 
                {
                    f(&mut resource);
                    match SCHEDULER.lock() {
                        Ok(mut scheduler) => {
                            scheduler.cowns.insert(clone, resource);
                        }
                        Err(_) => panic!("Failed to return resource")
                    }
                }
            )
        }
        Err(_) => panic!("Failed to schedule behaviour")
    };
    thread.join().unwrap();
}

pub struct Cown {
    identifier: uuid::Uuid,
}
impl Cown {
    pub fn create(resource: i64) -> Cown {
        let uuid = Uuid::new_v4();
        let cown = Cown { identifier: uuid };
        match SCHEDULER.lock() {
            Ok(mut scheduler) => {
                scheduler.cowns.insert(Cown::clone(&cown), resource);
                cown
            }
            Err(_) => panic!("Failed to allocate cown")
        }
    }
}
impl PartialEq for Cown {
    fn eq(&self, other: &Cown) -> bool { self.identifier == other.identifier }
}
impl Eq for Cown{}
impl Hash for Cown {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.identifier.hash(state);
    }
}
impl Clone for Cown {
    fn clone(&self) -> Cown {
        Cown { identifier: self.identifier }
    }
}
*/

struct Cown<T> {
    identifier: uuid::Uuid,
    phantom: PhantomData<T>,
}
impl <T> Cown<T> {
    pub fn create(resource: T) -> Cown<T> {
        let uuid = Uuid::new_v4();
        let cown = Cown { identifier: uuid };
    //    match SCHEDULER.lock() {
    //        Ok(mut scheduler) => {
    //            scheduler.cowns.insert(Cown::clone(&cown), resource);
    //            cown
    //        }
    //        Err(_) => panic!("Failed to allocate cown")
    //    }
    }
}

pub trait When<F> {
    fn when(&self, f: F);
}

impl <T, F> When<F> for (Cown<T>) where F: FnOnce(&mut T) + Send {
    fn when(&self, f: F) {

    }
}

impl <T, U, F> When<F> for (Cown<T>, Cown<U>) where F: FnOnce(&mut T, &mut U) + Send {
    fn when(&self, f: F) {

    }
}

fn main() {
    let c1 = Cown::create(1);
    let c2 = Cown::create(2);

    (c1, c2).when(|c1, c2| {

   });

    //when(&f1, |x| *x = *x + 1);


//    when(&f1, |x| println!("x is: {}", x));
}
