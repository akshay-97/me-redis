use std::net::TcpStream;
use std::io::{Write, Read};

fn main() {
    test_replication();
}


fn test_replication(){
    let addr = format!("{}:{}", "localhost", 6382);
    let mut stream = TcpStream::connect(addr).unwrap();
    
    stream.write_all("*3\r\n$3\r\nSET\r\n$3\r\nfoo\r\n$1\r\n1\r\n".as_bytes()).unwrap();
    stream.write_all("*3\r\n$3\r\nSET\r\n$3\r\nyel\r\n$1\r\n4\r\n".as_bytes()).unwrap();

    let r_addr = format!("{}:{}", "localhost", 6381);
    let mut replica_stream = TcpStream::connect(r_addr).unwrap();
    let mut buf = [0;32];
    replica_stream.write_all("*2\r\n$3\r\nGET\r\n$3\r\nfoo\r\n".as_bytes()).unwrap();
    replica_stream.read(&mut buf).expect("read failed");
    println!("client resp {:?}", String::from_utf8(Vec::from(buf)));
    replica_stream.write_all("*2\r\n$3\r\nGET\r\n$3\r\nyel\r\n".as_bytes()).unwrap();
    replica_stream.read(&mut buf).expect("read failed");
    println!("client resp {:?}", String::from_utf8(Vec::from(buf)));
    loop {
        
    }
}

/*
*2\r\n$3\r\nGET\r\n$3\r\nfoo\r\n
*/