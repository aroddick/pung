extern crate pung;
extern crate gj;
extern crate gjio;
extern crate capnp;
extern crate getopts;
extern crate time;

use getopts::Options;

use pung::client::PungClient;
use pung::db;
use time::PreciseTime;

fn print_usage(program: &str, opts: Options) {
    let brief = format!("Usage: {} [options]", program);
    print!("{}", opts.usage(&brief));
}

pub fn main() {

    let args: Vec<String> = std::env::args().collect();
    let program = args[0].clone();

    let mut opts = Options::new();
    opts.optflag("", "help", "print this help menu");

    // required parameters
    opts.reqopt("n", "name", "name of this client", "NAME");
    opts.reqopt("p", "peer", "name of peer", "PEER");
    opts.reqopt("x", "secret", "shared secret", "SECRET");

    // optional parameters
    opts.optopt("h", "host", "server's address", "IP:PORT");
    opts.optopt("k", "ret-rate", "ret rate", "RATE");
    opts.optopt("c", "contact-size", "contact size", "RATE");
    opts.optopt("s", "send-rate", "send rate", "RATE");
    //    opts.optopt("a", "alpha", "PIR aggregation", "ALPHA");
    opts.optopt("d", "depth", "PIR depth", "DEPTH");
    opts.optopt("o", "opt", "power (p) or hybrid (h)", "p / h");
    opts.optopt("r", "round", "number of rounds", "ROUND");
    opts.optopt("t", "type", "retrieval type", "e / b / t");
    opts.optopt("b", "extra", "change server extra", "EXTRA");

    // TODO: Maybe an option for a JSON config file to describe multiple peers.

    // Parse parameters
    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(e) => {
            print_usage(&program, opts);
            panic!(e.to_string())
        }
    };

    if matches.opt_present("help") {
        print_usage(&program, opts);
        return;
    }

    // required params (no available defaults)
    let user_name: String = matches.opt_str("n").unwrap();
    let peer_name: String = matches.opt_str("p").unwrap();
    let secret: Vec<u8> = matches.opt_str("x").unwrap().into_bytes();

    // optional params
    let server_addr: String = match matches.opt_str("h") {
        Some(v) => v,
        None => "127.0.0.1:12345".to_string(),
    };

    let ret_rate: u32 = match matches.opt_str("k") {
        Some(v) => u32::from_str_radix(&v, 10).unwrap(),
        None => 1,
    };

    let contact_size: u32 = match matches.opt_str("c") {
        Some(v) => u32::from_str_radix(&v, 10).unwrap(),
        None => 1,
    };

    let send_rate: u32 = match matches.opt_str("s") {
        Some(v) => u32::from_str_radix(&v, 10).unwrap(),
        None => 1,
    };

    let depth: u64 = match matches.opt_str("d") {
        Some(v) => u64::from_str_radix(&v, 10).unwrap(),
        None => 1,
    };

    //   let alpha: u64 = match matches.opt_str("a") {
    // Some(v) => u64::from_str_radix(&v, 10).unwrap(),
    // None => 1,
    // };
    //

    let rounds: usize = match matches.opt_str("r") {
        Some(v) => usize::from_str_radix(&v, 10).unwrap(),
        None => 1,
    };

    let extra: u64 = match matches.opt_str("b") {
        Some(v) => u64::from_str_radix(&v, 10).unwrap(),
        None => 0,
    };

    let ret_scheme: db::RetScheme = match matches.opt_str("t") {
        Some(v) => {
            match v.as_ref() {
                "n" => db::RetScheme::Naive,
                "e" => db::RetScheme::Explicit,
                "b" => db::RetScheme::Bloom,
                "t" => db::RetScheme::Tree,
                _ => panic!("Invalid retrieval parameter {}. Choose either e, b, or t.", v),
            }
        }

        None => db::RetScheme::Explicit,
    };


    let opt_scheme: db::OptScheme = match matches.opt_str("o") {
        Some(v) => {
            if ret_rate > 1 {

                match v.as_ref() {
                    "p" => db::OptScheme::Aliasing,
                    "h2" => db::OptScheme::Hybrid2,
                    "h4" => db::OptScheme::Hybrid4,
                    _ => panic!("Invalid optimization parameters {}. Choose either p, h2, or h4.", v),
                }

            } else {
                panic!("Multiret optimizations require retrieval rate (k)> 1");
            }
        }

        None => db::OptScheme::Normal,
    };


    gj::EventLoop::top_level(move |wait_scope| -> Result<(), capnp::Error> {

            let mut event_port = gjio::EventPort::new().unwrap();
            let mut client = PungClient::new(&user_name,
                                             &server_addr,
                                             send_rate,
                                             ret_rate,
                                             contact_size,
                                             depth,
                                             ret_scheme,
                                             opt_scheme,
                                             wait_scope,
                                             &mut event_port);

            client.init_dummy_peer();
            client.add_peer(&peer_name, &secret);

            // Register with the service
            let unique_id: u64 = (client.register(&wait_scope, &mut event_port))?;
            println!("Client {} registered with Pung server.", unique_id);

            // Changing the extra tuple value at the server (if requested).
            if extra > 0 {
                client.extra(extra, &wait_scope, &mut event_port)?;
                println!("Client {} is changing the extra tuples value at Pung server to {}...", unique_id, extra);
            }

            // Get current round number
            client.sync(&wait_scope, &mut event_port)?;
            println!("Client {} synchronized with the Pung server.", unique_id);

            //        std::thread::sleep(std::time::Duration::new(5, 0));

            let start_round = PreciseTime::now();
            for rnd in 0..rounds {

                println!("------------------");

                //      println!("{} - Sending {} tuples for round {}", unique_id, send_rate, client.get_round());
            

                // create random message
                let mut messages = Vec::with_capacity(send_rate as usize);
                let mut is_dial = false;
                let mut k = ret_rate;

                if rnd % 5 == 0 {
                    println!("[Round {}] Dialing...", client.get_round());
                    is_dial = true;
                    k = contact_size;
                    let msg = format!("Hello from {}", unique_id).into_bytes();
                    messages.push(msg);
                } else {
                    for i in 0..send_rate {
                        let msg = format!("Message #{} from User {}", i, unique_id).into_bytes();
                        messages.push(msg);
                    } 
                }

                let start = PreciseTime::now();

                // send tuple
                client.send(&peer_name, &mut messages, &wait_scope, &mut event_port, is_dial);

                let end = PreciseTime::now();
                let duration = start.to(end);

                // TODO: Do we need to change this send_rate?
                println!("[Round {}] Sent     | {} msgs   | {:?} usec", client.get_round(), send_rate, duration.num_microseconds().unwrap());


                // retrieve msg
                println!("[Round {}] Retrieving Message...", client.get_round());

                let start = PreciseTime::now();

                // create a ret request
                let mut peers: Vec<&str> = vec![];

                for _ in 0..k {
                    peers.push(&peer_name);
                }

                let msgs = client.retr(&peers[..], &wait_scope, &mut event_port, is_dial)?;

                let end = PreciseTime::now();
                println!("[Round {}] Retr     | {} msgs   | {:?} usec", client.get_round(),
                        msgs.len(),
                        start.to(end).num_microseconds().unwrap());

                for msg in msgs {
                    println!("[Round {}] Retr Message: \"{}\"", client.get_round(), String::from_utf8(msg).unwrap());
                }

                client.inc_round(1);
            }

            let end_round = PreciseTime::now();
            let duration = start_round.to(end_round);
            println!("Processed {} rounds in {} usec", rounds, duration.num_microseconds().unwrap());

            client.close(&wait_scope, &mut event_port)?;

            Ok(())
        })
        .expect("top level error");
}
