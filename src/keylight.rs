use serde::{Deserialize, Serialize};
use std::any::Any;
use std::net::{AddrParseError, Ipv4Addr};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use thiserror::Error;
use tokio::sync::{mpsc, Mutex};
use zeroconf::prelude::*;
use zeroconf::{MdnsBrowser, ServiceDiscovery, ServiceType};

#[derive(Error, Debug)]
pub enum ElgatoError {
    #[error("ParseError")]
    ParseError,

    #[error("DiscoverError")]
    DiscoverError,

    #[error(transparent)]
    RequestError(#[from] reqwest::Error),

    #[error(transparent)]
    IPError(#[from] AddrParseError),

    #[error(transparent)]
    CancelError(#[from] std::sync::mpsc::SendError<bool>),
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub number_of_lights: i64,
    pub lights: Vec<Light>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Light {
    pub on: i64,
    pub brightness: i64,
    pub temperature: i64,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct KeyLight {
    addr: Ipv4Addr,
    url: String,

    client: reqwest::Client,
    status: Arc<Mutex<Light>>,
}

impl KeyLight {
    pub fn new_from_ip(addr: Ipv4Addr) -> KeyLight {
        KeyLight {
            addr,
            url: format!("http://{}:9123/elgato/lights", addr.to_string()),

            client: reqwest::Client::new(),
            status: Default::default(),
        }
    }

    pub async fn new_from_name(name: &str) -> Result<KeyLight, ElgatoError> {
        let (tx, mut rx) = mpsc::channel(200);
        let (ctx, crx) = std::sync::mpsc::channel();

        let name = name.to_string();

        tokio::task::spawn_blocking(move || {
            let mut browser = MdnsBrowser::new(ServiceType::new("elg", "tcp").unwrap());

            browser.set_service_discovered_callback(Box::new(
                move |result: zeroconf::Result<ServiceDiscovery>,
                      _context: Option<Arc<dyn Any>>| {
                    let res = result.unwrap();
                    if res.name() == &name {
                        tx.blocking_send(res);
                    }
                },
            ));

            let event_loop = browser.browse_services().unwrap();

            loop {
                event_loop.poll(Duration::from_millis(500)).unwrap();

                match crx.try_recv() {
                    Ok(_) => return,
                    Err(_) => {}
                }
            }
        });

        let m = rx.recv().await.ok_or(ElgatoError::DiscoverError)?;

        ctx.send(true)?;

        let addr = Ipv4Addr::from_str(&m.address())?;

        Ok(KeyLight {
            addr,
            url: format!("http://{}:{}/elgato/lights", m.address(), m.port()),

            client: reqwest::Client::new(),
            status: Default::default(),
        })
    }

    pub async fn get(&mut self) -> Result<Status, ElgatoError> {
        let resp = self.client.get(&self.url).send().await?;

        Ok(resp.json::<Status>().await?)
    }
}
