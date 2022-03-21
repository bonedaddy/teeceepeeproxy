use std::fs::File;

use serde::{Serialize, Deserialize};
use anyhow::{anyhow, Result};
use i2p::sam::{SamConnection};

#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Configuration {
    pub proxy: Proxy,
    pub sam: SAM,
    pub server: Server,
}


/// configuration for a Proxy, which receives
/// connections over tcp, forwarding them to an i2p eepsite
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Proxy {
    pub listen_address: String,
    pub forward_address: String,
}

/// configuration for a Server, which registers
/// an i2p eepsite, and receives connections over i2p
/// forwarding them to a tcp service
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct Server{
    pub listen_address: String,
    pub forward_address: String,
    pub private_key: String,
    pub public_key: String,
}



/// configuration for the SAM bridge
#[derive(Clone, Default, Debug, Serialize, Deserialize)]
pub struct SAM {
    pub endpoint: String,
}

impl Configuration {
    pub fn new() -> Self { Default::default() }
    pub fn save(&self, path: &str) -> Result<()> {
        let config_data = serde_yaml::to_string(self)?;
        std::fs::write(path, config_data)?;
        Ok(())
    }
    pub fn new_sam_client(&self) -> Result<SamConnection> {
        match SamConnection::connect(self.sam.endpoint.clone()) {
            Ok(sam_conn) => Ok(sam_conn),
            Err(err) => return Err(anyhow!("failed to connect to sam bridge {:#?}", err))
        }
    }
    pub fn load(path: &str) -> Result<Self> {
        let data = std::fs::read(path)?;
        Ok(serde_yaml::from_slice(&data[..])?)
    }
}