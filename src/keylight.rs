use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::net::{AddrParseError, Ipv4Addr};
use std::ops::DerefMut;
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

    #[error("NoLight")]
    NoLight,

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
    pub on: u8,
    pub brightness: u8,
    pub temperature: u8,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct KeyLight {
    addr: Ipv4Addr,
    url: String,

    client: reqwest::Client,
    status: Arc<Mutex<Status>>,
}

impl KeyLight {
    pub async fn new_from_ip(addr: Ipv4Addr) -> Result<KeyLight, ElgatoError> {
        Ok(KeyLight::create(addr, 9123).await?)
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

        Ok(KeyLight::create(addr, *m.port()).await?)
    }

    async fn create(ip: Ipv4Addr, port: u16) -> Result<KeyLight, ElgatoError> {
        let k = KeyLight {
            addr: ip,
            url: format!("http://{}:{}/elgato/lights", ip.to_string(), port),

            client: reqwest::Client::new(),
            status: Default::default(),
        };

        //Test the light
        let s = k.get().await?;
        *k.status.lock().await.deref_mut() = s;

        tokio::spawn(KeyLight::poll_status(
            k.url.clone(),
            k.client.clone(),
            k.status.clone(),
        ));

        Ok(k)
    }

    async fn poll_status(url: String, client: Client, cache: Arc<Mutex<Status>>) {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            match client.get(&url).send().await {
                Ok(data) => match data.json::<Status>().await {
                    Ok(status) => {
                        *cache.lock().await.deref_mut() = status;
                    }
                    Err(_) => {}
                },
                Err(_) => {}
            }
        }
    }

    pub async fn get(&self) -> Result<Status, ElgatoError> {
        let resp = self.client.get(&self.url).send().await?;

        Ok(resp.json::<Status>().await?)
    }

    pub async fn set_brightness(&mut self, mut brightness: u8) -> Result<(), ElgatoError> {
        if brightness > 100 {
            brightness = 100;
        }

        let mut lock = self.status.lock().await;
        let mut current = lock.clone();
        for i in current.lights.iter_mut() {
            i.brightness = brightness;
        }

        self.client.put(&self.url).json(&current).send().await?;

        *lock.deref_mut() = current;

        Ok(())
    }

    pub async fn set_temperature(&mut self, temperature: u8) -> Result<(), ElgatoError> {
        let mut lock = self.status.lock().await;
        let mut current = lock.clone();
        for i in current.lights.iter_mut() {
            i.temperature = temperature;
        }

        self.client.put(&self.url).json(&current).send().await?;

        *lock.deref_mut() = current;

        Ok(())
    }
}
