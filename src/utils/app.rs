use std::collections::VecDeque;
use std::net::TcpStream;
use std::io::{Read, Write, Error, ErrorKind};
use std::sync::mpsc::{Sender, Receiver};
use std::sync::{atomic, Mutex, Arc};
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
    MoveToPool,
    ReadAndShare,
}

pub fn handle_client_replication(mut stream : TcpStream, state: &AppState){
    let mut buf = [0;512];
    let mut rem : Vec<u8> = Vec::new();
    loop {

        let source : &[u8];
        let mut read_count:usize = 0;

        if rem.is_empty(){
            read_count = stream.read(&mut buf).expect("read stream");
            if read_count == 0{
                continue
            }
            source = &buf;
        }
        else{
            source = &rem;
        }

        println!(" debug : request is {:?}" , String::from_utf8(Vec::from(source)));

        match decode_resp(&source){
            None => {
                println!("response cant be decoded, ignoring message");
                rem.clear();
                //handle_client_replication(stream, state)
            },
            Some((parsed_input,remainder)) =>{
        
                let mut response = Some(Resp::Nil);

                let _next_op = match parsed_input.clone(){
                    Resp::Arr(list) => handle_list_command(list, &mut response, state, &mut stream, source.len() - remainder.len()),
                    _ =>  NextOp::Read,
                };

                response.map( |r| r.send(&mut stream, &mut [0;128]).expect("write to stream"));
                
                if read_count != (512 - remainder.len()){
                    rem  = Vec::from(remainder);
                }

                
            }
        };
    }
}
//PLS REFACTOR
pub fn handle_client_2(mut stream : TcpStream, state: &AppState){
    let mut buf = [0;512];
    let mut rem : Vec<u8> = Vec::new();

    //read_buf(&mut self, buf: BorrowedCursor<'_>);
    //let mut borrowed_buf = buf.into();
    loop {
        let source : &[u8];
        let mut read_count :usize = 0;

        if check_empty(&rem){
            read_count = stream.read(&mut buf).expect("read stream");
            if read_count == 0{
                break
            }
            source = &buf;
        }
        else{
            source = &rem;
        }

        println!(" debug : request is {:?}" , String::from_utf8(Vec::from(source)));

        let (parsed_input, remainder) = decode_resp(source).expect("decode failed");
        
        

        let mut response = Some(Resp::Nil);

        let next_op = match parsed_input.clone(){
            Resp::Arr(list) => handle_list_command(list, &mut response, state, &mut stream, source.len() - remainder.len()),
            _ =>  NextOp::Read,
        };

        response.map( |r| r.send(&mut stream, &mut [0;128]).expect("write to stream"));

        if read_count != (512 - remainder.len()){
            rem  = Vec::from(remainder);
        }

        match next_op{
            NextOp::MoveToPool => {
                state.server_info.add_to_replication_pool(stream);
                break;
            },
            NextOp::ReadAndShare => {
                state.server_info.add_command_to_channel(parsed_input);
                //handle_client_2(stream, state);
            },
            NextOp::Read => {
                //handle_client_2(stream, state);
            }
        }
        //buf[] = *remainder;
    }
}

fn handle_list_command(mut list : VecDeque<Resp> , response :&mut Option<Resp>, state: &AppState, stream : &mut TcpStream, num_bytes: usize) -> NextOp{
    let first_val  = list.pop_front();
    let mut res = NextOp::Read;
    let mut respond = |resp, master_only| {
        if !state._is_master() && master_only{
            *response = None;
        }
        else{
            *response = Some(resp);
        }
    };
    match first_val {
        Some(Resp::BulkStr(s)) if s == "echo" || s == "ECHO" => {
            let r = list
                .pop_front()
                .and_then(|x| if Resp::if_str(&x){
                    Some(x)
                } else {None})
                .expect("echo value invalid");
            respond(r, true);
        },
        Some(Resp::BulkStr(s)) if s == "ping" || s == "PING" => {
            respond(Resp::SimpleStr("PONG".to_owned()), true);
            state.update_ack_bytes(num_bytes);
        },
        Some(Resp::BulkStr(s)) if s == "get" || s == "GET"=> {
            list
                .pop_front()
                .and_then(|x| x.get_str())
                .and_then(|str_key| (&state.store).get(str_key.as_str()))
                .map(|x| {respond(x, false);});

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
                ).map(|_| {respond(Resp::SimpleStr("OK".to_owned()), true);});
            res = NextOp::ReadAndShare;
            state.update_ack_bytes(num_bytes);
        },
        Some(Resp::BulkStr(s)) if s == "INFO" || s == "info" => {
            respond(Resp::BulkStr(state.server_info.get_info()), false);
        },
        Some(Resp::BulkStr(s)) if s == "REPLCONF" => {
            let is_ack = list
                .pop_front()
                .and_then(|x| x.get_str())
                .map(|x| x == "GETACK" || x == "getack")
                .unwrap_or(false);
            if is_ack{
                let mut r_list = VecDeque::new();
                r_list.push_back(Resp::BulkStr("REPLCONF".to_owned()));
                r_list.push_back(Resp::BulkStr("ACK".to_owned()));
                r_list.push_back(Resp::BulkStr(state.get_ack_bytes().to_string()));
                respond(Resp::Arr(r_list), false);
            }
            else {respond(Resp::SimpleStr("OK".to_owned()), true);}

            state.update_ack_bytes(num_bytes);
            
        },
        Some(Resp::BulkStr(s)) if s == "PSYNC" => {
            let dat = format!("+FULLRESYNC {} 0", state.server_info.get_repl_id().unwrap_or(""));
            stream.write_all(Encoder::encode(Resp::SimpleStr(dat)).unwrap().as_ref())
                //.and_then(|_| stream.read(&mut [0;128]))
                .and_then(|_| std::fs::read("src/utils/empty.rdb"))
                .and_then(|bytes_content| String::from_utf8(bytes_content).map_err(|_err| Error::new(ErrorKind::BrokenPipe, "bytes to string failed")))
                .and_then(|str_content| hex::decode(str_content).map_err(|_err|  Error::new(ErrorKind::BrokenPipe, "bytes to string failed")))
                .map(|file_contents| respond(Resp::FileContent(file_contents), true)).unwrap();
                res = NextOp::MoveToPool;
            
        },
        Some(Resp::BulkStr(s)) if s == "WAIT" => {
            respond(Resp::Num(state.get_replicas_connected() as i64), true);
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

    fn get_replication_connection(&mut self) -> Option<TcpStream>{
        match self{
            Info::Master(_) => None,
            Info::Replica(r) => {
                std::mem::take(&mut r._connection)
            }
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

struct ReplicaInfo {
    _master_host : String,
    _master_port : u32,
    _connection : Option<TcpStream>,
    ack_bytes : Arc<atomic::AtomicU64>,
}

impl ReplicaInfo{
    fn get_info() -> String{
        String::from("role:slave")
    }
}

impl AppState {
    fn new(master_info : Option<String>, current_port : u32, maybe_tx : Option<Sender<Resp>>) -> Self{
        Self{
            store : InMem::new(),
            server_info: 
                match master_info{
                    None => {
                        Info::Master(MasterInfo{
                            master_replid : "8371b4fb1155b71f4a04d3e1bc3e18c4a990aeeb".to_string(),
                            master_repl_offset : 0,
                            commands_channel : maybe_tx.expect("commands channel not initiated"),
                            replication_connection_pool : Mutex::new(Vec::new()),
                        })
                    },
                    Some(master_inf) => {
                        let mut server_info : Vec<&str> = master_inf.split(" ").collect();
                        let port = server_info.pop().and_then(|x| x.parse::<u32>().ok()).unwrap_or(6379);
                        let host = server_info.pop().unwrap_or("localhost").to_string();
                        
                        let addr = format!("{}:{}", &host, &port);
                        let mut stream = TcpStream::connect(addr).expect("connection to master failed");
                        let _ = stream.write_all("*1\r\n$4\r\nping\r\n".as_bytes())
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
                            .and_then(|_| stream.read(&mut [0;1024])); // read rdb file
    

                        Info::Replica(ReplicaInfo{
                            _master_host : host,
                            _master_port : port,
                            _connection : Some(stream),
                            ack_bytes : Arc::new(atomic::AtomicU64::new(0))
                        })
                    }
                }
            }
    }

    fn _is_master(&self) -> bool{
        match (&self).server_info{
            Info::Master(_) => true,
            Info::Replica(_) => false,
        }
    }

    fn get_ack_bytes(&self) -> u64{
        match &self.server_info{
            Info::Master(_) => 0,
            Info::Replica(repl) => repl.ack_bytes.load(atomic::Ordering::Acquire),
        }
    }

    fn update_ack_bytes(&self, u : usize) {
        match &self.server_info{
            Info::Master(_) => {},
            Info::Replica(repl) => {repl.ack_bytes.fetch_add(u as u64, atomic::Ordering::SeqCst);},
        }
    }

    fn get_replicas_connected(&self) -> usize {
        match &self.server_info{
            Info::Replica(_) => 0,
            Info::Master(m) => {
                let pool = m.replication_connection_pool.lock().unwrap();
                pool.len()
            }
        }
    }
}

pub fn make_app_state(master_info : Option<String>, current_port : u32, maybe_tx : Option<Sender<Resp>>) -> AppState{
    AppState::new(master_info, current_port, maybe_tx)
}

pub fn get_replication_connection(app : &mut AppState) -> Option<TcpStream>{
    app.server_info.get_replication_connection()
}


fn check_empty(buf : &Vec<u8>) -> bool{
    if buf.is_empty(){
        return true
    }

    for i in buf{
        if *i != 0{
            return false
        }
    }
    return true
}