use std::collections::HashMap;
use crate::Resp;
use std::sync::{Arc, Mutex};

pub type Map = HashMap<String, Resp>;

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
        store.get(key).map(std::clone::Clone::clone)
    }

    pub fn set(&self, key: String, value : Resp) -> Result<(), String>{
        let mut store = self.dat.lock().unwrap();
        let _ = store.insert(key,value);
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