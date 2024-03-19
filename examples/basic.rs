use existing_instance::Msg;

fn main() {
    env_logger::init();
    // You can set this to true if you want to test nonblocking
    let nonblocking = false;
    match existing_instance::establish_endpoint("basic_example", nonblocking).unwrap() {
        existing_instance::Endpoint::New(listener) => {
            eprintln!("New instance, listening for messages");
            loop {
                if let Some(mut conn) = listener.accept() {
                    dbg!(conn.recv());
                }
            }
        }
        existing_instance::Endpoint::Existing(mut stream) => {
            eprintln!("Existing instance detected. Sending message.");
            stream.send(Msg::String(
                std::env::args().nth(1).unwrap_or("Default message".into()),
            ))
        }
    }
}
