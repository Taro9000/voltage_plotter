use serial2::SerialPort;
use std::io::{BufReader, Read};
use std::time::{Duration, Instant};
use std::{array, thread};

use crate::{Sample, Samples};

type BuffReader<T> = BufReader<T>;

fn read_with_timeout(port: &mut BuffReader<SerialPort>, timeout: Duration) -> Vec<u8> {
    let mut line = Vec::new();
    let start = Instant::now();
    let mut buffer = [0u8; 1];
    while port.read(&mut buffer).is_ok() && start.elapsed() < timeout {
        line.push(buffer[0]);
    }
    line
}

pub enum State {
    SearchingPorts,
    HandleCommands([u8; 5], BuffReader<SerialPort>),
}

impl State {
    fn searching_ports() -> Self {
        thread::sleep(Duration::from_millis(100));
        let ports = match SerialPort::available_ports() {
            Ok(ports) => ports,
            Err(err) => {
                eprintln!("{err}");
                return Self::SearchingPorts;
            }
        };
        println!("{} ports available", ports.len());
        #[allow(clippy::unnecessary_debug_formatting)]
        for port_info in &ports {
            let port = SerialPort::open(port_info.as_path(), 115_200);
            match port {
                Ok(mut port) => {
                    if port.set_read_timeout(Duration::from_millis(100)).is_err() {
                        continue;
                    }
                    // if port.write(b"Simulating length missmatch...").is_err() {
                    //     continue
                    // }
                    if port.write(
                        b"\nUmm, h-hi~. \
                        Is this perhaps by any chance a Capacitive Soil Moisture Sensor Module Esp32 relayed \
                        that just so happens to be made by Taro900?. \
                        No offense if not my d-dear slave~. *blushes*\n"
                    ).is_err() {
                        continue
                    }
                    // if port.flush().is_err() {
                    //     continue;
                    // }
                    let mut port = BuffReader::new(port);
                    let line = read_with_timeout(&mut port, Duration::from_millis(100));

                    println!("Got {}", String::from_utf8_lossy(&line));
                    let needle = b"Indeed, master.";
                    if line.windows(needle.len()).any(|window| window == needle) {
                        println!("Succesfully connected to {port_info:?}");
                        return Self::HandleCommands([0; 5], port);
                    }
                    println!(
                        "Not a remote presentation control WiFi LoRa 32 V3 receiver \
                        that just so happens to be made by Taro900: {port_info:?}"
                    );
                }
                Err(error) => eprintln!("Failed to open {port_info:?}: {error}"),
            }
        }
        Self::SearchingPorts
    }
    fn handle_commands(
        mut window: [u8; 5],
        mut port: BuffReader<SerialPort>,
        samples: &mut Samples,
    ) -> Self {
        let mut byte = [0; 1];
        match port.read(&mut byte) {
            Ok(1) => {}
            Ok(_) => return Self::HandleCommands(window, port),
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                // println!("Time out!");
            }
            Err(e) => {
                eprintln!("Read error: {e}");
                return Self::SearchingPorts;
            }
        }
        let byte = byte[0];
        window = array::from_fn(|index| {
            if let Some(&next) = window.get(index + 1) {
                next
            } else {
                byte
            }
        });

        if let [b'\t', first, b'\t', last, b'\n'] = window {
            let reading = u16::from_ne_bytes([first, last]);
            samples.push_back(Sample {
                time: Instant::now(),
                value: reading as f64,
            });
            println!("{reading}");
        }
        Self::HandleCommands(window, port)
    }
    pub fn iterate(self, samples: &mut Samples) -> Self {
        match self {
            Self::SearchingPorts => Self::searching_ports(),
            Self::HandleCommands(window, serial_port_informations) => {
                Self::handle_commands(window, serial_port_informations, samples)
            }
        }
    }
}
