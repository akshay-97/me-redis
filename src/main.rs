mod utils;
use utils::{app::{self, get_replication_connection, handle_client_replication, handle_replication, AppState}, pool, resp::Resp};
use std::{net::TcpListener, sync::mpsc::{self, Receiver, Sender}};
use clap::Parser;

#[derive(Parser,Default,Debug)]
struct Cli{
    #[clap(short, long)]
    port : Option<u32>,
    #[clap(long)]
    replicaof: Option<String>,
}

fn create_channel(is_master_server : bool) -> (Option<Sender<Resp>>, Option<Receiver<Resp>>){
    if is_master_server{
        let (tx,rx)  = mpsc::channel::<Resp>();
        (Some(tx), Some(rx))
    }
    else{
        (None,None)
    }
}

fn init_replication_thread(is_master_server : bool
    , maybe_rx : Option<Receiver<Resp>>
    , app_state : &'static AppState
    , replication_conn : Option<std::net::TcpStream>)
{
    if is_master_server{
        let rx = maybe_rx.expect("expected channel recv to be initialized");
        std::thread::spawn(|| {handle_replication(rx, app_state);});
    } else{
        let stream = replication_conn.expect("replication connection missing");
        std::thread::spawn(|| {handle_client_replication(stream, app_state);});
    }
}

//./spawn_redis_server.sh --port <PORT> --replicaof <MASTER_HOST> <MASTER_PORT>
fn main() {
    let args = Cli::parse();
    let port = args.port.unwrap_or(6379);
    let address  = format!("127.0.0.1:{}", port);
    let listener = TcpListener::bind(address).unwrap();
    
    let mut thread_pool = pool::Pool::new();
    let is_master_sv = args.replicaof.is_none();
    let (maybe_tx, maybe_rx) = create_channel(is_master_sv);
    let app_state = app::make_app_state(args.replicaof, port, maybe_tx);
    // this is unsafe, create rc instead
    let app = Box::leak(Box::new(app_state));
    
    let repl_conn = get_replication_connection(app);
    init_replication_thread(is_master_sv, maybe_rx, &*app, repl_conn);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let app_pointer = &*app;
                thread_pool.execute( move || app::handle_client_2(stream, app_pointer));
            }
            Err(e) => {
                println!("error: {}", e);
            }
        }
    }
}
