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
    pub temperature: u16,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct KeyLight {
    addr: Ipv4Addr,
    url: String,
    name: String,

    poll: bool,
    poll_cancel: tokio::sync::mpsc::Sender<bool>,
    client: reqwest::Client,
    status: Arc<Mutex<Status>>,
}

impl KeyLight {
    /// Create a new Keylight from a known IP.
    ///
    /// # Arguments
    ///
    /// * `addr` - IP address to the keylight
    /// * `poll` - If the library should poll the light for updates
    pub async fn new_from_ip(
        name: &str,
        addr: Ipv4Addr,
        poll: bool,
    ) -> Result<KeyLight, ElgatoError> {
        Ok(KeyLight::create(name, addr, 9123, poll).await?)
    }

    /// Create a new Keylight from device name
    /// This uses zeroconf to discover the light on the network.
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the lamp like "Key Light Left" or whatever your light is named
    /// * `poll` - If the library should poll the light for updates
    pub async fn new_from_name(name: &str, poll: bool) -> Result<KeyLight, ElgatoError> {
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
                        let _ = tx.blocking_send(res);
                    }
                },
            ));

            let event_loop = browser.browse_services().unwrap();

            loop {
                event_loop.poll(Duration::from_millis(500)).unwrap();

                match crx.try_recv() {
                    Ok(_) => return,
                    Err(e) => match e {
                        std::sync::mpsc::TryRecvError::Empty => {}
                        std::sync::mpsc::TryRecvError::Disconnected => return,
                    },
                }
            }
        });

        let m = rx.recv().await.ok_or(ElgatoError::DiscoverError)?;

        ctx.send(true)?;

        let addr = Ipv4Addr::from_str(&m.address())?;

        Ok(KeyLight::create(m.name(), addr, *m.port(), poll).await?)
    }

    async fn create(
        name: &str,
        ip: Ipv4Addr,
        port: u16,
        poll: bool,
    ) -> Result<KeyLight, ElgatoError> {
        let (ptx, ctx) = tokio::sync::mpsc::channel(5);

        let k = KeyLight {
            addr: ip,
            url: format!("http://{}:{}/elgato/lights", ip.to_string(), port),
            name: name.to_string(),

            poll,
            poll_cancel: ptx,
            client: reqwest::Client::new(),
            status: Default::default(),
        };

        //Test the light
        let s = k.get_status().await?;
        *k.status.lock().await.deref_mut() = s;

        if poll {
            tokio::spawn(KeyLight::poll_status(
                k.url.clone(),
                k.client.clone(),
                k.status.clone(),
                ctx,
            ));
        }

        Ok(k)
    }

    async fn poll_status(
        url: String,
        client: Client,
        cache: Arc<Mutex<Status>>,
        mut cancel: tokio::sync::mpsc::Receiver<bool>,
    ) {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = interval.tick() =>  {
                    match client.get(&url).send().await {
                        Ok(data) => match data.json::<Status>().await {
                            Ok(status) => {
                                *cache.lock().await.deref_mut() = status;
                            }
                            Err(_) => {}
                        },
                        Err(_) => {}
                    };
                }

                _ = cancel.recv() => {
                    return;
                }

            }
        }
    }

    /// Get the current settings of the light, if polling is enabled, returns the cached data.
    async fn get_status(&self) -> Result<Status, ElgatoError> {
        let resp = self.client.get(&self.url).send().await?;

        Ok(resp.json::<Status>().await?)
    }

    pub async fn get(&self) -> Result<Status, ElgatoError> {
        if self.poll {
            Ok(self.status.lock().await.clone())
        } else {
            self.get_status().await
        }
    }

    pub async fn name(&self) -> String {
        self.name.clone()
    }

    /// Set the brightness of the light
    ///
    /// # Arguments
    ///
    /// * `brightness` - Value between 0-100
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

    /// Set the brightness of the light relative to the current value
    ///
    /// # Arguments
    ///
    /// * `brightness` - f64 between -1.0 and 1.0
    pub async fn set_relative_brightness(
        &mut self,
        mut brightness: f64,
    ) -> Result<f64, ElgatoError> {
        if brightness > 1.0 {
            brightness = 1.0;
        }

        let mut lock = self.status.lock().await;
        let mut current = lock.clone();

        let mut avg = Vec::new();
        for i in current.lights.iter_mut() {
            let nv = (i.brightness as f64 + (brightness * 100.0)).clamp(0.0, 100.0);
            i.brightness = nv as u8;
            avg.push(nv);
        }

        self.client.put(&self.url).json(&current).send().await?;

        *lock.deref_mut() = current;

        Ok(avg.iter().sum::<f64>() / avg.len() as f64)
    }

    /// Set the color temperature of the light
    ///
    /// # Arguments
    ///
    /// * `temperature` - Value between 2900 - 7000 (Kelvin)
    pub async fn set_temperature(&mut self, temperature: u32) -> Result<(), ElgatoError> {
        // Light expects a value between 143 to 344 where 143 is 2900K and 344 is 7000K.
        // Figured this out by sniffing the official application.
        // Might be different for other lights?
        let temperature = (((temperature as f32).clamp(2900.0, 7000.0) - 2900.0)
            / (4100.0 / (344.0 - 143.0))
            + 143.0)
            .clamp(143.0, 344.0) as u16;

        let mut lock = self.status.lock().await;
        let mut current = lock.clone();
        for i in current.lights.iter_mut() {
            i.temperature = temperature;
        }

        self.client.put(&self.url).json(&current).send().await?;

        *lock.deref_mut() = current;

        Ok(())
    }

    /// Turn on/off the light
    ///
    /// # Arguments
    ///
    /// * `power` - true: on, false: off
    pub async fn set_power(&mut self, power: bool) -> Result<(), ElgatoError> {
        // Figured this out by using the official application.
        // Might be different for other lights?

        let mut lock = self.status.lock().await;
        let mut current = lock.clone();
        for i in current.lights.iter_mut() {
            i.on = power as u8;
        }

        self.client.put(&self.url).json(&current).send().await?;

        *lock.deref_mut() = current;

        Ok(())
    }
}
