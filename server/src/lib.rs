use config::Configuration;
use anyhow::{Result, anyhow};
use crossbeam::sync::WaitGroup;
use i2p::{sam::StreamForward, net::{I2pListener, I2pAddr}};
use std::{thread, time};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use clap::{ArgMatches, App, Arg, SubCommand};
use std::io::{Write, Read};
use log::*;
use std::str::from_utf8;
use std::sync::Arc;
use bufstream::BufStream;
pub struct Server {
    pub cfg: Configuration,
}

impl Server {
    pub fn new(cfg: Configuration) -> Self { Self { cfg }}
    pub async fn start(self: &Arc<Self>) -> Result<()> {
        let server = I2pListener::bind_persistent(&self.cfg.server.private_key).unwrap();
        let our_dest = server.local_addr().unwrap();
        // our destination address
        let our_dest_addr = I2pAddr::from_b64(&format!("{}", our_dest.dest())).unwrap();
        info!("server address: {}", our_dest_addr);
        for stream in server.incoming() {
            match stream {
                Ok(stream) => {
                    let mut stream = stream;
                    tokio::task::spawn(async move {
                        let mut tokio_stream = match stream.to_tokio_stream() {
                            Ok(stream) => stream,
                            Err(err) => {
                                error!("failed to convert to tokio stream {:#?}", err);
                                return;
                            }
                        };
                        let (mut read, mut write) = tokio::net::TcpStream::split(&mut tokio_stream);
                        let mut read_buf: [u8; 1024] = [0_u8; 1024];
                        loop {
                            tokio::select! {
                                res = read.read(&mut read_buf) => {
                                    match res {
                                        Ok(n) => {
                                            if n == 0 { continue; }
                                            info!("read {} bytes", n);
                                            match write.write(&read_buf[0..n]).await {
                                                Ok(_) => (),
                                                Err(err) => {
                                                    error!("failed to wite buffer {:#?}", err);
                                                    return;
                                                }
                                            }
                                        }
                                        Err(err) => {
                                            error!("fuck {:#?}", err);
                                            return;
                                        }
                                    }
                                }
                            }
                        }
                    });
                }
                Err(e) => error!("Error on incoming connection: {:?}", e),
            }
        }
        Ok(())
    }
}