// Uncomment this block to pass the first stage
use std::{io::Write, net::{TcpListener, TcpStream}};

fn handle_client(mut s : TcpStream) -> &'static str{
    let response =  "+PONG\r\n+PONG\r\n";
    s.write_all(response.as_bytes()).expect("stream should have written");
    "asdsf"
}
fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    println!("Logs from your program will appear here!");
    
    let listener = TcpListener::bind("127.0.0.1:6370").unwrap();
    
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                handle_client(stream);
                println!("accepted  connection");
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
