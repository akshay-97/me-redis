pub struct Pool {
    capacity : usize,
    workers: Vec<Worker>,
}

impl Pool{
    pub fn new() -> Self{
        Self {
            capacity : 10,
            workers: {
                let mut w = Vec::with_capacity(10);
                for _i in 0..10{
                    w.push(Worker{is_available : true});
                }
                w
            }
        }
    }

    pub fn execute<F>(&mut self, handler_function : F)
    where
        F: FnOnce() -> () + Send + 'static  
    {
        for i in 0..self.capacity{
            if self.workers[i].can_work(){
                self.workers[i].do_work();
                self.workers[i].work(handler_function);
                break;
            }
        }

    }
}

struct Worker{
    is_available : bool,
}

impl Worker{
    fn can_work(&self) -> bool{
        self.is_available
    }

    fn do_work(&mut self){
        self.is_available = false;
    }

    fn work<F> (&self, handler : F)
    where
        F: FnOnce() -> () + Send + 'static
    {
        std::thread::spawn(handler);
    }
}
