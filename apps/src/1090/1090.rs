use std::io::{BufRead, BufReader};
use std::net::TcpStream;

use adsb_deku::Frame;
use clap::Parser;

// Include BEAST parser from parent directory
#[path = "../beast.rs"]
mod beast;

#[derive(Debug, Parser)]
#[command(
    name = "1090",
    version,
    author = "wcampbell0x2a",
    about = "Dump ADS-B protocol info from demodulator"
)]
struct Options {
    /// ip address of ADS-B demodulated bytes server
    #[arg(long, default_value = "localhost")]
    host: String,
    /// port of ADS-B demodulated bytes server  
    /// Default: 30002 for raw mode, 30005 for BEAST mode
    #[arg(long)]
    port: Option<u16>,
    /// Panic on adsb_deku::Frame::fmt::Display not implemented
    #[arg(long)]
    panic_display: bool,
    /// Panic on adsb_deku::Frame::from_bytes() error
    #[arg(long)]
    panic_decode: bool,
    /// Display debug of adsb::Frame
    #[arg(long)]
    debug: bool,
    /// Use BEAST mode binary format instead of raw hex format
    #[arg(long)]
    beast_mode: bool,
}

impl Options {
    /// Get the appropriate port based on mode and user input
    fn get_port(&self) -> u16 {
        self.port.unwrap_or({
            if self.beast_mode {
                30005  // BEAST mode default port
            } else {
                30002  // Raw mode default port
            }
        })
    }
}

fn main() {
    let options = Options::parse();
    let port = options.get_port();
    let stream = TcpStream::connect((options.host.as_str(), port)).unwrap();
    // Use longer timeout for BEAST mode to allow for binary data buffering
    let timeout = if options.beast_mode { 
        std::time::Duration::from_millis(1000) 
    } else { 
        std::time::Duration::from_millis(50) 
    };
    stream.set_read_timeout(Some(timeout)).unwrap();
    
    // Initialize BEAST parser if in BEAST mode
    let mut beast_parser = if options.beast_mode {
        Some(beast::BeastParser::new())
    } else {
        None
    };
    
    // Use different reader types based on mode
    let mut reader = BufReader::new(&stream);
    let mut input = String::new();

    loop {
        if let Some(ref mut parser) = beast_parser {
            // BEAST mode processing - use raw stream for binary data
            let mut stream_ref = reader.get_mut();
            match parser.parse_frames(&mut stream_ref) {
                Ok(frames) => {
                    for beast_frame in frames {
                        let bytes = beast_frame.message_bytes();
                        
                        if options.debug {
                            println!("BEAST frame type: {:?}, signal: {}, timestamp: {}ms", 
                                beast_frame.frame_type, beast_frame.signal_level, beast_frame.timestamp_ms());
                        }
                        
                        // Print hex representation
                        let hex_string: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
                        println!("{}", hex_string.to_lowercase());

                        // check for all 0's
                        if bytes.iter().all(|&b| b == 0) {
                            continue;
                        }

                        // decode
                        match Frame::from_bytes(bytes) {
                            Ok(frame) => {
                                if options.debug {
                                    println!("{frame:#?}");
                                }
                                println!("{frame}");
                                assert!(
                                    !((frame.to_string() == "") && options.panic_display),
                                    "[E] fmt::Display not implemented"
                                );
                            }
                            Err(e) => {
                                assert!(!options.panic_decode, "[E] {e}");
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("BEAST parser error: {e:?}");
                    break;
                }
            }
        } else {
            // Raw mode processing (original logic)
            input.clear();
            if let Ok(len) = reader.read_line(&mut input) {
                if len == 0 {
                    continue;
                }
                // convert from string hex -> bytes
                let hex = &mut input.to_string()[1..len - 2].to_string();
                println!("{}", hex.to_lowercase());
                let bytes = if let Ok(bytes) = hex::decode(hex) {
                    bytes
                } else {
                    continue;
                };

                // check for all 0's
                if bytes.iter().all(|&b| b == 0) {
                    continue;
                }

                // decode
                match Frame::from_bytes(&bytes) {
                    Ok(frame) => {
                        if options.debug {
                            println!("{frame:#?}");
                        }
                        println!("{frame}");
                        assert!(
                            !((frame.to_string() == "") && options.panic_display),
                            "[E] fmt::Display not implemented"
                        );
                    }
                    Err(e) => {
                        assert!(!options.panic_decode, "[E] {e}");
                    }
                }
            }
        }
    }
}
