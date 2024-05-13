use std::collections::VecDeque;
use std::net::TcpStream;
use std::io::{Read, Write, Error, ErrorKind};
use std::sync::mpsc::{Sender, Receiver};
use std::sync::Mutex;
use crate::utils::{dat::InMem, resp::{Resp, Encoder, decode_resp}};

use super::resp::Message;

pub fn handle_replication(rx : Receiver<Resp>, state: &AppState){
    loop {
        match rx.recv(){
            Ok(resp) => {
                println!("debug: resp sent {:?}", resp);
                let message = resp.encode();
                state.server_info.send_to_replica(message);
            },
            Err(err) => println!("replication failure {:?}", err),
        }
    }
}

enum NextOp{
    Read,
    MoveToPool
}

pub fn handle_client_2(mut stream : TcpStream, state: &AppState){
    let mut buf = [0;512];
    if stream.read(&mut buf).expect("read stream") == 0{
        return
    }

    let (parsed_input, _) = decode_resp(&buf).expect("decode failed");
    let mut response = Resp::Nil;

    let next_op = match parsed_input{
        Resp::Arr(list) => handle_list_command(list, &mut response, state, &mut stream),
        _ =>  NextOp::Read,
    };

    response.clone().send(&mut stream, &mut [0;128]).expect("write to stream");

    match next_op{
        NextOp::MoveToPool => state.server_info.add_to_replication_pool(stream),
        NextOp::Read => {
            state.server_info.add_command_to_channel(response);
            handle_client_2(stream, state);
        }
    }

}

fn handle_list_command(mut list : VecDeque<Resp> , response :&mut Resp, state: &AppState, stream : &mut TcpStream) -> NextOp{
    let first_val  = list.pop_front();
    let mut res = NextOp::Read;
    match first_val {
        Some(Resp::BulkStr(s)) if s == "echo" || s == "ECHO" => {
            *response = list
                .pop_front()
                .and_then(|x| if Resp::if_str(&x){
                    Some(x)
                } else {None})
                .expect("echo value invalid")
        },
        Some(Resp::BulkStr(s)) if s == "ping" || s == "PING" => {
            *response = Resp::SimpleStr("PONG".to_owned());
        },
        Some(Resp::BulkStr(s)) if s == "get" || s == "GET"=> {
            list
                .pop_front()
                .and_then(|x| x.get_str())
                .and_then(|str_key| (&state.store).get(str_key.as_str()))
                .map(|x| {*response = x;});

        },
        Some(Resp::BulkStr(s)) if s == "set" || s == "SET"=> {
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
                ).map(|_| {*response = Resp::SimpleStr("OK".to_owned());});
        },
        Some(Resp::BulkStr(s)) if s == "INFO" || s == "info" => {
            *response = Resp::BulkStr(state.server_info.get_info());
        },
        Some(Resp::BulkStr(s)) if s == "REPLCONF" => {
            *response =Resp::SimpleStr("OK".to_owned());
        },
        Some(Resp::BulkStr(s)) if s == "PSYNC" => {
            let dat = format!("+FULLRESYNC {} 0", state.server_info.get_repl_id().unwrap_or(""));
            stream.write_all(Encoder::encode(Resp::SimpleStr(dat)).unwrap().as_ref())
                //.and_then(|_| stream.read(&mut [0;128]))
                .and_then(|_| std::fs::read("src/utils/empty.rdb"))
                .and_then(|bytes_content| String::from_utf8(bytes_content).map_err(|_err| Error::new(ErrorKind::BrokenPipe, "bytes to string failed")))
                .and_then(|str_content| hex::decode(str_content).map_err(|_err|  Error::new(ErrorKind::BrokenPipe, "bytes to string failed")))
                .map(|file_contents| {*response = Resp::FileContent(file_contents);}).unwrap();
                res = NextOp::MoveToPool;
            
        },
        _ => {}
    }
    res
}

pub struct AppState{
    store : InMem,
    server_info : Info,
}


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

    fn get_repl_id(&self) -> Option<&str>{
        match self{
            Info::Master(m) => Some(m.get_repl_id()),
            Info::Replica(_) => None,
        }
    }

    fn send_to_replica(&self, message : Option<Vec<u8>>){
        match (self, message){
            (Info::Master(m), Some(mes)) =>{
                let mut pool = m.replication_connection_pool.lock().unwrap();
                for i in pool.iter_mut(){
                    i.write_all(&mes).expect("write failed");
                }
            }
            _ => {},
        }
    }

    fn add_to_replication_pool(&self, stream : TcpStream){
        match self{
            Info::Master(m) => {
                let mut pool = m.replication_connection_pool.lock().unwrap();
                pool.push(stream);
            },
            Info::Replica(_) => println!("warn: replica cant store stream in pool"),
        }
    }

    fn add_command_to_channel(&self, resp: Resp){
        match self{
            Info::Master(m) => {
                let _ = m.commands_channel.send(resp);
            },
            Info::Replica(_) => println!("warn: replica cant store stream in pool"),
        }
    }
}

struct MasterInfo {
    master_replid : String,
    master_repl_offset : u32,
    replication_connection_pool : Mutex<Vec<TcpStream>>,
    commands_channel : Sender<Resp>,
}

impl MasterInfo{
    fn get_info(&self) -> String{
        format!("role:master\nmaster_replid:{}\nmaster_repl_offset:{}",self.master_replid, self.master_repl_offset)
    }

    fn get_repl_id(&self) -> &str{
        &self.master_replid
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
    fn new(master_info : Option<Vec<String>>, current_port : u32, maybe_tx : Option<Sender<Resp>>) -> Self{
        Self{
            store : InMem::new(),
            server_info: master_info
                .map_or(Info::Master(MasterInfo{
                    master_replid : "8371b4fb1155b71f4a04d3e1bc3e18c4a990aeeb".to_string(),
                    master_repl_offset : 0,
                    commands_channel : maybe_tx.expect("commands channel not initiated"),
                    replication_connection_pool : Mutex::new(Vec::new()),
                }), |mut master_inf| {
                    let port = master_inf.pop().and_then(|x| x.parse::<u32>().ok()).unwrap_or(6379);
                    let host = master_inf.pop().unwrap_or("localhost".to_string());
                    
                    let addr = format!("{}:{}", &host, &port);
                    let mut stream = TcpStream::connect(addr).expect("connection to master failed");
                    stream.write_all("*1\r\n$4\r\nping\r\n".as_bytes())
                        .and_then(|_| stream.read(&mut [0;128]))
                        .and_then(|_| {
                            let request = format!("*3\r\n$8\r\nREPLCONF\r\n$14\r\nlistening-port\r\n$4\r\n{}\r\n", current_port);
                            stream.write_all(request.as_bytes())
                        })
                        .and_then(|_| stream.read(&mut [0;128]))
                        .and_then(|_| stream.write_all("*3\r\n$8\r\nREPLCONF\r\n$4\r\ncapa\r\n$6\r\npsync2\r\n".as_bytes()))
                        .and_then(|_| stream.read(&mut [0;128]))
                        .and_then(|_| stream.write_all("*3\r\n$5\r\nPSYNC\r\n$1\r\n?\r\n$2\r\n-1\r\n".as_bytes()))
                        .and_then(|_| stream.read(&mut [0;128]))
                        .expect("handshake to master failed");

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

pub fn make_app_state(master_info : Option<Vec<String>>, current_port : u32, maybe_tx : Option<Sender<Resp>>) -> AppState{
    AppState::new(master_info, current_port, maybe_tx)
}