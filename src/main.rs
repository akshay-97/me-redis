mod utils;
use utils::{pool, app};
use std::net::TcpListener;
use clap::Parser;

#[derive(Parser,Default,Debug)]
struct Cli{
    #[clap(short, long)]
    port : Option<u32>,
    #[clap(long, num_args = 2)]
    replicaof: Option<Vec<String>>,
}


//./spawn_redis_server.sh --port <PORT> --replicaof <MASTER_HOST> <MASTER_PORT>
fn main() {
    let args = Cli::parse();
    let port = args.port.unwrap_or(6379);
    let address  = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(address).unwrap();
    let mut thread_pool = pool::Pool::new();
    let app_state = app::make_app_state(args.replicaof);
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let app = app_state.clone();
                thread_pool.execute( move || app::handle_client(stream, app));
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
