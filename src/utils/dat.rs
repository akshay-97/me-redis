use std::collections::HashMap;
use crate::Resp;
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

    pub fn set(&self, key: String, value : Resp, ttl : Option<i64>) -> Result<(), String>{
        print!("what");
        let mut store = self.dat.lock().unwrap();
        let time_param = ttl
            .and_then(|x| TryInto::try_into(x).ok())
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