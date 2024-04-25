use std::net::TcpStream;
use std::io::{Read, Write};
use crate::utils::{dat::InMem, resp::{Resp, Encoder, decode_resp}};

pub fn handle_client(mut s : TcpStream, state : &AppState){
    let mut buf = [0;512];
    loop {
        
        let count = s.read(&mut buf).expect("read stream");
        if count ==0{
            break;
        }

        let (parsed_input, _) = decode_resp(&buf).expect("unexpected decode");

        let mut response = Resp::Nil;

        match parsed_input{
            Resp::BulkStr(str) if str.eq("ping") => {
                response = Resp::SimpleStr("PONG".to_owned());
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
                        response = Resp::SimpleStr("PONG".to_owned());
                    },
                    Some(Resp::BulkStr(s)) if s == "get" => {
                        list
                            .pop_front()
                            .and_then(|x| x.get_str())
                            .and_then(|str_key| (&state.store).get(str_key.as_str()))
                            .map(|x| {response = x;});

                    },
                    Some(Resp::BulkStr(s)) if s == "set" => {
                        list
                            .pop_front()
                            .and_then(|x| x.get_str())
                            .and_then(|str_key| {
                                list
                                    .pop_front()
                                    .and_then(|v| {
                                        let ttl = list
                                            .pop_front()
                                            .and_then(|x| x.get_str().and_then(|option_ttl|
                                                if option_ttl == "px" {
                                                    Some(())
                                                }
                                                else{
                                                    None
                                                }))
                                            .and_then(|_| list.pop_front()
                                                            .and_then(|x| x.get_str())
                                                            .and_then(|x| x.parse::<u128>().ok())
                                                    );
                                        (&state.store).set(str_key, v, ttl).ok()
                                    })
                                }
                            ).map(|_| {response = Resp::SimpleStr("OK".to_owned());});
                    },
                    Some(Resp::BulkStr(s)) if s == "INFO" || s == "info" => {
                       response = Resp::BulkStr(state.server_info.get_info());
                    },
                    _ => {}
                }
            }
            _ => {}
        }
        s.write_all(Encoder::encode(response).unwrap().as_bytes()).expect("stream should have written");
    }
}


pub struct AppState{
    store : InMem,
    server_info : Info,
   // pool : 
}

#[derive(Clone)]
enum Info{
    Master(MasterInfo),
    Replica(ReplicaInfo)
}

impl Info{
    fn get_info(&self) -> String{
        match self{
            Info::Master(m) => m.get_info(),
            Info::Replica(_) => ReplicaInfo::get_info(),
        }
    }
}
#[derive(Clone)]
struct MasterInfo {
    master_replid : String,
    master_repl_offset : u32,
}

impl MasterInfo{
    fn get_info(&self) -> String{
        format!("role:master\nmaster_replid:{}\nmaster_repl_offset:{}",self.master_replid, self.master_repl_offset)
    }
}

#[derive(Clone)]
struct ReplicaInfo {
    _master_host : String,
    _master_port : u32,
    //connection : Rc<TcpStream>
}

impl ReplicaInfo{
    fn get_info() -> String{
        String::from("role:slave")
    }
}

impl AppState {
    fn new(master_info : Option<Vec<String>>, current_port : u32) -> Self{
        Self{
            store : InMem::new(),
            server_info: master_info
                .map_or(Info::Master(MasterInfo{
                    master_replid : "8371b4fb1155b71f4a04d3e1bc3e18c4a990aeeb".to_string(),
                    master_repl_offset : 0
                }), |mut master_inf| {
                    let port = master_inf.pop().and_then(|x| x.parse::<u32>().ok()).unwrap_or(6379);
                    let host = master_inf.pop().unwrap_or("localhost".to_string());
                    
                    let addr = format!("{}:{}", &host, &port);
                    let mut stream = TcpStream::connect(addr).expect("connection to master failed");
                    stream.write_all("*1\r\n$4\r\nping\r\n".as_bytes())
                        .and_then(|_| {
                            let request = format!("*3\r\n$8\r\nREPLCONF\r\n$14\r\nlistening-port\r\n$4\r\n{}\r\n", current_port);
                            stream.write_all(request.as_bytes())
                        })
                        .and_then(|_| stream.write_all("*3\r\n$8\r\nREPLCONF\r\n$4\r\ncapa\r\n$6\r\npsync2\r\n".as_bytes())).expect("error sending data to master");
                    Info::Replica(ReplicaInfo{
                        _master_host : host,
                        _master_port : port,
                        //connection  : stream,
                    })}
                )
        }
    }

    fn _is_master(&self) -> bool{
        match (&self).server_info{
            Info::Master(_) => true,
            Info::Replica(_) => false,
        }
    }
}

impl Clone for AppState {
    fn clone(&self) -> Self {
        Self {
            store : (&self).store.clone(),
            server_info : (&self).server_info.clone(),
        }
    }
}
pub fn make_app_state(master_info : Option<Vec<String>>, current_port : u32) -> AppState{
    AppState::new(master_info, current_port)
}