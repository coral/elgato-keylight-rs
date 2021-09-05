use elgato_keylight::KeyLight;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    //Lookup lamp by name (using zeroconf)
    let mut kl = KeyLight::new_from_name("Key Light Left", true).await?;

    //Turn on the light
    kl.set_power(true).await?;

    //Set brightness to 30
    let rel = kl.set_relative_brightness(-0.2).await?;
    dbg!(rel);

    Ok(())
}
