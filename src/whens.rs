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
