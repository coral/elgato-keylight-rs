use elgato_keylight::KeyLight;
use std::net::Ipv4Addr;
use std::str::FromStr;
use std::time::Duration;
#[tokio::main]
async fn main() {
    let ip = Ipv4Addr::from_str("10.0.1.32").unwrap();
    let mut kl = KeyLight::new_from_ip(ip).await.unwrap();

    dbg!(&kl);

    kl.set_brightness(10).await.unwrap();
}
