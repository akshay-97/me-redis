use std::collections::HashMap;
use crate::utils::resp::Resp;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

type Val = (Resp, Option<(SystemTime, u128)>);
type Map = HashMap<String, Val>;

// INCOMING SHIT CODE, PLEASE REFACTOR
pub struct InMem {
    dat : Arc<Mutex<Map>>
}

impl InMem{
    pub fn new() -> Self{
        Self {
            dat: Arc::new(Mutex::new(HashMap::new()))
        }
    }

    pub fn get(&self, key : &str) -> Option<Resp>{
        let store = self.dat.lock().unwrap();
        store
            .get(key)
            .and_then(|(resp, maybe_time)| {
                if does_time_hold(maybe_time.as_ref()){
                    return Some(resp.clone())
                }
                None
            })
    }

    pub fn set(&self, key: String, value : Resp, ttl : Option<u128>) -> Result<(), String>{
        let mut store = self.dat.lock().unwrap();
        let time_param = ttl
            .map(|t| (SystemTime::now(),t));
        let _ = store.insert(key,(value, time_param));
        Ok(())
    }
}

impl Clone for InMem{
    fn clone(&self) -> Self {
        Self{
            dat  : Arc::clone(&self.dat)
        }
    }
}

fn does_time_hold(time_param: Option<&(SystemTime, u128)>) -> bool{
    match time_param {
        Some((created_time, ttl)) => {
            created_time.elapsed().map_or(false, |duration|{
                duration.as_millis() < *ttl
            })
        }
        None => true,
    }
}



// pub fn handle_client(mut stream : TcpStream, state : &AppState){
//     let mut buf = [0;512];
//     loop {
//         // TODO: find a way to read and decode so that message is not partially read
//         let count = stream.read(&mut buf).expect("read stream");
//         if count ==0{
//             break;
//         }
//         //println!("what is input {:?}", String::from_utf8(Vec::from(buf)));
//         let (parsed_input, _) = decode_resp(&buf).expect("unexpected decode");

//         let mut response = Resp::Nil;

//         match parsed_input{
//             Resp::BulkStr(str) if str.eq("ping") => {
//                 response = Resp::SimpleStr("PONG".to_owned());
//             },
//             Resp::Arr(mut list) => {
//                 let first_val  = list.pop_front();
//                 match first_val {
//                     Some(Resp::BulkStr(s)) if s == "echo" || s == "ECHO" => {
//                         response = list
//                             .pop_front()
//                             .and_then(|x| if Resp::if_str(&x){
//                                 Some(x)
//                             } else {None})
//                             .expect("echo value invalid")
//                     },
//                     Some(Resp::BulkStr(s)) if s == "ping" || s == "PING" => {
//                         response = Resp::SimpleStr("PONG".to_owned());
//                     },
//                     Some(Resp::BulkStr(s)) if s == "get" || s == "GET"=> {
//                         list
//                             .pop_front()
//                             .and_then(|x| x.get_str())
//                             .and_then(|str_key| (&state.store).get(str_key.as_str()))
//                             .map(|x| {response = x;});

//                     },
//                     Some(Resp::BulkStr(s)) if s == "set" || s == "SET"=> {
//                        // println!("strBulk {:?}" , list);
//                         list
//                             .pop_front()
//                             .and_then(|x| x.get_str())
//                             .and_then(|str_key| {
//                                 list
//                                     .pop_front()
//                                     .and_then(|v| {
//                                         let ttl = list
//                                             .pop_front()
//                                             .and_then(|x| x.get_str().and_then(|option_ttl|
//                                                 if option_ttl == "px" {
//                                                     Some(())
//                                                 }
//                                                 else{
//                                                     None
//                                                 }))
//                                             .and_then(|_| list.pop_front()
//                                                             .and_then(|x| x.get_str())
//                                                             .and_then(|x| x.parse::<u128>().ok())
//                                                     );
//                                         (&state.store).set(str_key, v, ttl).ok()
//                                     })
//                                 }
//                             ).map(|_| {response = Resp::SimpleStr("OK".to_owned());});
//                     },
//                     Some(Resp::BulkStr(s)) if s == "INFO" || s == "info" => {
//                        response = Resp::BulkStr(state.server_info.get_info());
//                     },
//                     Some(Resp::BulkStr(s)) if s == "REPLCONF" => {
//                         response =Resp::SimpleStr("OK".to_owned());
//                     },
//                     Some(Resp::BulkStr(s)) if s == "PSYNC" => {
//                         let dat = format!("+FULLRESYNC {} 0", state.server_info.get_repl_id().unwrap_or(""));
//                         stream.write_all(Encoder::encode(Resp::SimpleStr(dat)).unwrap().as_ref())
//                             //.and_then(|_| stream.read(&mut [0;128]))
//                             .and_then(|_| std::fs::read("src/utils/empty.rdb"))
//                             .and_then(|bytes_content| String::from_utf8(bytes_content).map_err(|_err| Error::new(ErrorKind::BrokenPipe, "bytes to string failed")))
//                             .and_then(|str_content| hex::decode(str_content).map_err(|_err|  Error::new(ErrorKind::BrokenPipe, "bytes to string failed")))
//                             .map(|file_contents| {response = Resp::FileContent(file_contents);}).unwrap();
//                         // state.server_info.add_stream(stream)
                        
//                     },
//                     _ => {}
//                 }
//             }
//             _ => {}
//         }
//         response.send(&mut stream, &mut [0;128]).expect("send response failed");
//         //stream.write_all(Encoder::encode(response).unwrap().as_ref()).expect("stream should have written");
//     }
// }