use std::{io::Write, io::Read, net::{TcpListener, TcpStream}};
mod utils;

use utils::pool;
use utils::resp::{decode_resp, Resp, Encoder};

fn handle_client(mut s : TcpStream){
    let mut buf = [0;512];
    loop {
        
        let count = s.read(&mut buf).expect("read stream");
        if count ==0{
            println!("end of connection");
            break;
        }
        println!("{:?}", buf);

        let (parsed_input, _) = decode_resp(&buf).expect("unexpected decode");

        let mut response = Resp::Nil;

        match parsed_input{
            Resp::BulkStr(str) if str.eq("ping") => {
                response = Resp::BulkStr("PONG".to_owned());
            },
            Resp::Arr(mut list) => {
                let first_val  = list.pop_front();
                match first_val {
                    Some(Resp::BulkStr(s)) if s == "echo" => {
                        response = list
                            .pop_front()
                            .and_then(|x| if Resp::if_str(&x){
                                Some(x)
                            } else {None})
                            .expect("echo value invalid")
                    },
                    Some(Resp::BulkStr(s)) if s == "ping" => {
                        response = Resp::BulkStr("PONG".to_owned());
                    },
                    _ => {}
                }
            }
            _ => {}
        }
        s.write_all(Encoder::encode(response).unwrap().as_bytes()).expect("stream should have written");

    }
}



fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port = args.get(1).map_or(6379, |v| v.as_str().parse::<u32>().unwrap());
    let address  = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(address).unwrap();
    let mut thread_pool = pool::Pool::new(); 
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
