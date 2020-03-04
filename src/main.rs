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

fn end() {
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

struct Cown<T> {
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

pub trait When<F> {
    fn when(&self, f: F);
}

impl <T: Send + 'static, F: FnOnce(&mut T) + Send + 'static>
    When<F> for &Cown<T> {
    fn when(&self, f: F) {
        let resource = self.resource.clone();
        let behaviour = Box::new(move || {
            f(resource.get());
        });
        schedule(&[Cown::address(self)], behaviour);
    }
}

impl <T: Send + 'static, U: Send + 'static, F: FnOnce(&mut T, &mut U) + Send + 'static> 
    When<F> for (&Cown<T>, &Cown<U>) {
    fn when(&self, f: F) {
        let (r1 , r2) = (self.0.resource.clone(), self.1.resource.clone());
        let behaviour = Box::new(move || {
            f(r1.get(), r2.get());
        });
        schedule(&[Cown::address(&self.0), Cown::address(&self.1)], behaviour);
    }
}

impl <T: Send + 'static, U: Send + 'static, V: Send + 'static, W: Send + 'static,
      X: Send + 'static, F: FnOnce(&mut T, &mut U, &mut V, &mut W, &mut X) + Send + 'static> 
    When<F> for (&Cown<T>, &Cown<U>, &Cown<V>, &Cown<W>, &Cown<X>) {
    fn when(&self, f: F) {
        let (r1 , r2, r3, r4, r5) = 
            (self.0.resource.clone(), self.1.resource.clone(), 
             self.2.resource.clone(), self.3.resource.clone(),
             self.4.resource.clone());
        let behaviour = Box::new(move || {
            f(r1.get(), r2.get(), r3.get(), r4.get(), r5.get());
        });
        schedule(&[Cown::address(&self.0), Cown::address(&self.1),
                   Cown::address(&self.2), Cown::address(&self.3),
                   Cown::address(&self.4)], 
                 behaviour);
    }
}

struct Fork {
    count: u64,
}
impl Fork {
    fn new() -> Fork {
        Fork { count: 0 }
    }

    fn eat(&mut self) {
        self.count = self.count + 1;
    }
}

struct Table {
    count: u64,
    f1: Cown<Fork>,
    f2: Cown<Fork>,
    f3: Cown<Fork>,
    f4: Cown<Fork>,
    f5: Cown<Fork>,
}
impl Table {
    fn new(f1: Cown<Fork>, f2: Cown<Fork>, f3: Cown<Fork>,
           f4: Cown<Fork>, f5: Cown<Fork>) -> Table {
        Table { count: 5, f1: f1, f2: f2, f3: f3, f4: f4, f5: f5 }
    }

    fn leave(table: &Cown<Table>) {
        (&table).when(|table| {
            table.count = table.count - 1;
            if table.count == 0 {
                (&table.f1, &table.f2, &table.f3, &table.f4, &table.f5).when(|f1, f2, f3, f4, f5| {
                    println!("f1: {}", f1.count);
                    println!("f2: {}", f2.count);
                    println!("f3: {}", f3.count);
                    println!("f4: {}", f4.count);
                    println!("f5: {}", f5.count);
                })
            }
        });
    }
}

struct Phil {
    id: u64,
    hunger: u64,
    left: Cown<Fork>,
    right: Cown<Fork>,
    table: Cown<Table>,
}
impl Phil {
    fn new(id: u64, left: Cown<Fork>,right: Cown<Fork>, table: Cown<Table>)
            -> Phil {
        Phil { id: id, hunger: 10, left: left, right: right, table: table }
    }

    fn eat(mut self) {
//        when(&self.left, &self.right).do(|left, right| {
//
//        });

        (&self.left.clone(), &self.right.clone()).when(|left, right| {
            println!("Phil {} eats", self.id);
            self.hunger = self.hunger - 1;
            left.eat();
            right.eat();
            if self.hunger > 0 {
                self.eat();
            } else {
                Table::leave(&self.table);
            }
        });
    }
}

fn main() {
    // straight-forward example
    let c1 = Cown::create(1);
    let c2 = Cown::create(2);

    (&c1).when(|c1| {
        *c1 = *c1 + 1;
    });

    (&c1).when(|c1| {
        println!("c1 is: {}", c1);
    });

    (&c2).when(|c2| {
        println!("c2 is: {}", c2);
    });

    (&c1, &c2).when(|c1, c2| {
        println!("c1 + c2 is: {}", *c1 + *c2);
    });

    // broken example
    // (&c1.clone(), &c1).when(|c1, c4| {
    //    println!("c1 is: {}", *c1 + *c4);
    // });

    // Dining Phils (Wadler, Jupitus, Mitchell, Seymour Hoffman, Glass)

    let f1 = Cown::create(Fork::new());
    let f2 = Cown::create(Fork::new());
    let f3 = Cown::create(Fork::new());
    let f4 = Cown::create(Fork::new());
    let f5 = Cown::create(Fork::new());

    let table = Cown::create(Table::new(f1.clone(), f2.clone(), f3.clone(),
                                        f4.clone(), f5.clone()));

    let p1 = Phil::new(1, f1.clone(), f2.clone(), table.clone());
    let p2 = Phil::new(2, f2.clone(), f3.clone(), table.clone());
    let p3 = Phil::new(3, f3.clone(), f4.clone(), table.clone());
    let p4 = Phil::new(4, f4.clone(), f5.clone(), table.clone());
    let p5 = Phil::new(5, f5.clone(), f1.clone(), table.clone());

    p1.eat();
    p2.eat();
    p3.eat();
    p4.eat();
    p5.eat();

    end();
}
