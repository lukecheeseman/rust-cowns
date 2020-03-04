#[macro_use]
extern crate cowns;

use cowns::whens::*;

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
        when!(table).run(|table| {
            table.count = table.count - 1;
            if table.count == 0 {
                when!(table.f1, table.f2, table.f3, table.f4, table.f5).run(|f1, f2, f3, f4, f5| {
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
        when!(self.left, self.right).run(|left, right| {
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
    /*
    // straight-forward example
    let c1 = Cown::create(1);
    let c2 = Cown::create(2);

    when!(&c1).run(|c1| {
        *c1 = *c1 + 1;
    });

    when!(&c1).run(|c1| {
        println!("c1 is: {}", c1);
    });

    when!(&c2).run(|c2| {
        println!("c2 is: {}", c2);
    });

    when!(&c1, &c2).run(|c1, c2| {
        println!("c1 + c2 is: {}", *c1 + *c2);
    });

    // broken example: runtime aborts due to alaising cowns
    // (&c1.clone(), &c1).when(|c1, c4| {
    //    println!("c1 is: {}", *c1 + *c4);
    // });
    */

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

    // THIS WILDLY IMPORTANT FUNCTION THAT MEANS YOU EXAMPLE PROBABLY FINISHES
    end();
}
