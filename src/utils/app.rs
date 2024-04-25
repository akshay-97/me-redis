use std::net::TcpStream;
use std::io::{Read, Write};
use crate::utils::{dat::InMem, resp::{Resp, Encoder, decode_resp}};

pub fn handle_client(mut s : TcpStream, state : AppState){
    let mut buf = [0;512];
    loop {
        
        let count = s.read(&mut buf).expect("read stream");
        if count ==0{
            break;
        }

        let (parsed_input, _) = decode_resp(&buf).expect("unexpected decode");
        println!("what is input {:?}", parsed_input);

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
                       if state.is_master(){
                            response = Resp::BulkStr("role:master".to_owned());
                       }
                       else {
                        response = Resp::BulkStr("slave:master".to_owned());
                       }
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
    server_info : Info
}

#[derive(Clone)]
enum Info{
    Master(MasterInfo),
    Replica(ReplicaInfo)
}
#[derive(Clone)]
struct MasterInfo {}

#[derive(Clone)]
struct ReplicaInfo {}

impl AppState {
    fn new(master_info : Option<Vec<String>>) -> Self{
        Self{
            store : InMem::new(),
            server_info: master_info
                .map_or(Info::Master(MasterInfo{}), |_| Info::Replica(ReplicaInfo{}))
        }
    }

    fn is_master(&self) -> bool{
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
pub fn make_app_state(master_info : Option<Vec<String>>) -> AppState{
    AppState::new(master_info)
}