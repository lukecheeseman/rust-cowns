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

impl <T: Send + 'static, F: FnOnce(&mut T) + Send + 'static> When<F> for &Arc<Cown<T>> {
    fn when(&self, f: F) {
        let cown = Arc::clone(self);
        let behaviour = Box::new(move || { f(&mut cown.data.borrow_mut()); });
        schedule(&[Cown::address(self)], behaviour);
    }
}

impl <T: Send + 'static, U: Send + 'static, F: FnOnce(&mut T, &mut U) + Send + 'static> 
    When<F> for (&Arc<Cown<T>>, &Arc<Cown<U>>) {
    fn when(&self, f: F) {
        let (c1 , c2) = (Arc::clone(self.0), Arc::clone(self.1));
        let behaviour = Box::new(move ||
            { f(&mut c1.data.borrow_mut(), &mut c2.data.borrow_mut()); });
        schedule(&[Cown::address(&self.0), Cown::address(&self.1)], behaviour);
    }
}

impl <T: Send + 'static, U: Send + 'static, V: Send + 'static,
     F: FnOnce(&mut T, &mut U, &mut V) + Send + 'static> 
    When<F> for (&Arc<Cown<T>>, &Arc<Cown<U>>, &Arc<Cown<V>>) {
    fn when(&self, f: F) {
        let (c1 , c2, c3) = (Arc::clone(&self.0), Arc::clone(&self.1), Arc::clone(&self.2));
        let behaviour = Box::new(move ||
            { f(&mut c1.data.borrow_mut(), 
                &mut c2.data.borrow_mut(),
                &mut c3.data.borrow_mut()); });
        schedule(&[Cown::address(&self.0), Cown::address(&self.1), Cown::address(&self.2)], 
                 behaviour);
    }
}

impl <T: Send + 'static, U: Send + 'static, V: Send + 'static, W: Send + 'static,
     F: FnOnce(&mut T, &mut U, &mut V, &mut W) + Send + 'static> 
    When<F> for (&Arc<Cown<T>>, &Arc<Cown<U>>, &Arc<Cown<V>>, &Arc<Cown<W>>) {
    fn when(&self, f: F) {
        let (c1 , c2, c3, c4) = (Arc::clone(&self.0), Arc::clone(&self.1),
                                 Arc::clone(&self.2), Arc::clone(&self.3));
        let behaviour = Box::new(move ||
            { f(&mut c1.data.borrow_mut(), 
                &mut c2.data.borrow_mut(),
                &mut c3.data.borrow_mut(),
                &mut c4.data.borrow_mut()); });
        schedule(&[Cown::address(&self.0), Cown::address(&self.1),
                   Cown::address(&self.2), Cown::address(&self.3)], 
                 behaviour);
    }
}

impl <T: Send + 'static, U: Send + 'static, V: Send + 'static, W: Send + 'static,
      X: Send + 'static, F: FnOnce(&mut T, &mut U, &mut V, &mut W, &mut X) + Send + 'static> 
    When<F> for (&Arc<Cown<T>>, &Arc<Cown<U>>, &Arc<Cown<V>>, &Arc<Cown<W>>, &Arc<Cown<X>>) {
    fn when(&self, f: F) {
        let (c1 , c2, c3, c4, c5) = (Arc::clone(self.0), Arc::clone(self.1),
                                 Arc::clone(self.2), Arc::clone(self.3),
                                 Arc::clone(self.4));
        let behaviour = Box::new(move ||
            { f(&mut c1.data.borrow_mut(), 
                &mut c2.data.borrow_mut(),
                &mut c3.data.borrow_mut(),
                &mut c4.data.borrow_mut(),
                &mut c5.data.borrow_mut()); });
        schedule(&[Cown::address(self.0), Cown::address(self.1),
                   Cown::address(self.2), Cown::address(self.3),
                   Cown::address(self.4)], 
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
    f1: Arc<Cown<Fork>>,
    f2: Arc<Cown<Fork>>,
    f3: Arc<Cown<Fork>>,
    f4: Arc<Cown<Fork>>,
    f5: Arc<Cown<Fork>>,
}
impl Table {
    fn new(f1: &Arc<Cown<Fork>>, f2: &Arc<Cown<Fork>>, f3: &Arc<Cown<Fork>>,
           f4: &Arc<Cown<Fork>>, f5: &Arc<Cown<Fork>>) -> Table {
        Table { count: 5, f1: f1.clone(), f2: f2.clone(), f3: f3.clone(), 
                f4: f4.clone(), f5: f5.clone() }
    }

    fn leave(table: Arc<Cown<Table>>) {
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
    hunger: u64,
    left: Arc<Cown<Fork>>,
    right: Arc<Cown<Fork>>,
    table: Arc<Cown<Table>>,
}
impl Phil {
    fn new(left: &Arc<Cown<Fork>>, right: &Arc<Cown<Fork>>, table: &Arc<Cown<Table>>) -> Phil {
        Phil { hunger: 10, left: left.clone(), right: right.clone(), table: table.clone() }
    }

    fn eat(mut self) {
        (&self.left.clone(), &self.right.clone()).when(|left, right| {
            self.hunger = self.hunger - 1;
            left.eat();
            right.eat();
            if self.hunger > 0 {
                self.eat();
            } else {
                Table::leave(self.table);
            }
        });
    }
}

fn main() {
    /* straight-forward example
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
    (c1.clone(), c1).when(|c1, c4| {
        println!("c1 is: {}", c1);
    });
    */

    let f1 = Cown::create(Fork::new());
    let f2 = Cown::create(Fork::new());
    let f3 = Cown::create(Fork::new());
    let f4 = Cown::create(Fork::new());
    let f5 = Cown::create(Fork::new());

    let table = Cown::create(Table::new(&f1, &f2, &f3, &f4, &f5));

    let p1 = Phil::new(&f1, &f2, &table);
    let p2 = Phil::new(&f2, &f3, &table);
    let p3 = Phil::new(&f3, &f4, &table);
    let p4 = Phil::new(&f4, &f5, &table);
    let p5 = Phil::new(&f5, &f1, &table);

    p1.eat();
    p2.eat();
    p3.eat();
    p4.eat();
    p5.eat();

    end();
}
