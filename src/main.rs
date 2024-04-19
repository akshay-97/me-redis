// Uncomment this block to pass the first stage
use std::{io::Write, io::Read, net::{TcpListener, TcpStream}};

fn handle_client(mut s : TcpStream){
    let mut buf = [0;512];
    loop {
        
        let count = s.read(&mut buf).expect("read stream");
        if count ==0{
            break;
        }
        let response =  "+PONG\r\n";
        s.write_all(response.as_bytes()).expect("stream should have written");

    }
}


struct Pool {
    capacity : usize,
    workers: Vec<Worker>,
}

impl Pool{
    fn new() -> Self{
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

    fn execute<F>(&mut self, handler_function : F)
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

        // std::thread::spawn(handler_function);
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

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port = args.get(1).map_or(6379, |v| v.as_str().parse::<u32>().unwrap());
    let address  = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(address).unwrap();
    let mut thread_pool = Pool::new(); 
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                thread_pool.execute( || handle_client(stream));
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
