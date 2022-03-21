use i2p::sam::SignatureType;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use clap::{ArgMatches, App, Arg, SubCommand};
use anyhow::{anyhow, Result};
use i2p::net::{I2pListener, I2pStream, I2pAddr};
use std::io::{Write, Read};
use std::sync::Arc;
use log::*;
use std::{thread, time};
use std::str::from_utf8;
#[tokio::main]
async fn main() -> Result<()> {
	env_logger::init();

	let matches = App::new("proxy")
    .arg(
        Arg::with_name("config")
        .help("path to the config file")
        .long("config")
        .takes_value(true)
        .value_name("FILE")
    )
	.subcommands(vec![
        SubCommand::with_name("config")
        .about("config management commands")
        .subcommands(vec![
            SubCommand::with_name("new")
            .about("generate a new config file")
        ]),
        SubCommand::with_name("utils")
        .about("utility commands")
        .subcommands(vec![
            SubCommand::with_name("gen-destination")
            .about("generate a destination public/private keypair")
        ]),
		SubCommand::with_name("forwarder")
		.arg(Arg::with_name("ip").long("ip").takes_value(true).required(true))
		.arg(Arg::with_name("destination").long("destination").takes_value(true).required(true)),
		SubCommand::with_name("server"),
        SubCommand::with_name("echo")
        .arg(Arg::with_name("ip").long("ip").takes_value(true).required(true)),
	]).get_matches();

    let config_file_path = matches.value_of("config").unwrap_or("config.yaml");

	process_matches(&matches, config_file_path).await?;
	Ok(())
}

async fn process_matches(matches: &ArgMatches<'_>, config_file_path: &str) -> Result<()> {
	match matches.subcommand() {
        ("config", Some(cfg)) => match cfg.subcommand() {
            ("new", Some(_)) => {
                let conf = config::Configuration::new();
                conf.save(config_file_path)
            }
            _ => return Err(anyhow!("invalid subcommand")),
        }
        ("utils", Some(utils)) => match utils.subcommand() {
            ("gen-destination", Some(_)) => {
                let conf = config::Configuration::load(config_file_path)?;
                let mut sam_client = conf.new_sam_client()?;
                let (pubkey, seckey) = sam_client.generate_destination(SignatureType::EdDsaSha512Ed25519).unwrap();
                println!("public key {}", pubkey);
                println!("secret key {}", seckey);

                Ok(())
            },
            _ => return Err(anyhow!("invalid subcommand")),
        }
        ("echo", Some(echo)) => {
            let ip = echo.value_of("ip").unwrap();
			let listener = match TcpListener::bind(ip.to_string()).await {
                Ok(listener) => listener,
                Err(err) => return Err(anyhow!("failed to create listener {:#?}", err)),
            };
            info!("addr {}", listener.local_addr().unwrap());
            loop {
                // Asynchronously wait for an inbound socket.
                let (mut socket, _) = listener.accept().await?;
                // And this is where much of the magic of this server happens. We
                // crucially want all clients to make progress concurrently, rather than
                // blocking one on completion of another. To achieve this we use the
                // `tokio::spawn` function to execute the work in the background.
                //
                // Essentially here we're executing a new task to run concurrently,
                // which will allow all of our clients to be processed concurrently.

                    let mut buf = vec![0; 1024];
                    // In a loop, read data from the socket and write the data back.
                    loop {
                        let n = socket
                            .read(&mut buf)
                            .await
                            .expect("failed to read data from socket");

                        if n == 0 {
                            match socket.shutdown().await {
                                Ok(_) => (),
                                Err(err) => error!("socket shutdown failed {:#?}", err),
                            }
                            info!("finished processing socket");
                            break;
                        }
                        let data_str = match from_utf8(&buf[0..n]) {
                            Ok(utf8) => utf8,
                            Err(err) => {
                                error!("failed to read data {:#?}", err);
                                continue;
                            }
                        };
                        info!("received data {}", data_str);
                        match socket.write(&buf[0..n]).await {
                            Ok(_) => (),
                            Err(err) => error!("failed to write to socket {:#?}", err)
                        }
                    }
            }
        }
		("forwarder", Some(forwarder)) => {
			let ip = forwarder.value_of("ip").unwrap();
			let destination = forwarder.value_of("destination").unwrap();
			let mut stream = I2pStream::connect(destination).unwrap();
			let listener = match TcpListener::bind(ip.to_string()).await {
                Ok(listener) => listener,
                Err(err) => return Err(anyhow!("failed to create listener {:#?}", err)),
            };
            loop {
                // Asynchronously wait for an inbound socket.
                let (mut socket, _) = listener.accept().await?;

                // And this is where much of the magic of this server happens. We
                // crucially want all clients to make progress concurrently, rather than
                // blocking one on completion of another. To achieve this we use the
                // `tokio::spawn` function to execute the work in the background.
                //
                // Essentially here we're executing a new task to run concurrently,
                // which will allow all of our clients to be processed concurrently.

                    let mut buf = vec![0; 1024];
                    // In a loop, read data from the socket and write the data back.
                    loop {
                        let n = socket
                            .read(&mut buf)
                            .await
                            .expect("failed to read data from socket");

                        if n == 0 {
                            break;
                        }
                        if let Ok(msg) = from_utf8(&buf[0..n]) {
                            info!("received data {}", msg);
                        }
						match stream.write(&buf[0..n]) {
                            Ok(_) => (),
                            Err(err) => error!("failed to write data to stream {:#?}", err)
                        }
                        match stream.read(&mut buf) {
                            Ok(n) => {
                                match from_utf8(&buf[0..n]) {
                                    Ok(data) => {
                                        info!("stream received reply {}", data);
                                    },
                                    Err(err) => {
                                        error!("failed to read stream {:#?}", err);
                                        break;
                                    }
                                }
                            }
                            Err(err) => {
                                error!("failed to read from stream {:#?}", err);
                                break;
                            }
                        }
                    }
            }
		}
		("server", Some(_)) => {
            let conf = config::Configuration::load(config_file_path)?;
            let srv = Arc::new(server::Server::new(conf));
            srv.start().await?;
			Ok(())
		}
		_ => return Err(anyhow!("invalid subcommand")),
	}
}