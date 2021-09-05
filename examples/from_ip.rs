use elgato_keylight::KeyLight;
use std::error::Error;
use std::net::Ipv4Addr;
use std::str::FromStr;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    //Lookup lamp using IP
    let ip = Ipv4Addr::from_str("10.0.1.32")?;
    let mut kl = KeyLight::new_from_ip("Key Light 2000", ip, true).await?;

    //Turn on the light
    kl.set_power(true).await?;

    //Set brightness to 30
    kl.set_brightness(30).await?;

    //Slowly increase the color temperature
    for n in 143..344 {
        //Set temperature
        kl.set_temperature(n).await?;

        //Sleep for 1 ms
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }

    //Turn of the light
    kl.set_power(false).await?;

    //Get the lamp status
    let status = kl.get().await?;
    println!("{:?}", status);

    Ok(())
}
