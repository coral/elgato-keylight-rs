use elgato_keylight::KeyLight;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    //Lookup lamp by name (using zeroconf)
    let mut kl = KeyLight::new_from_name("Key Light Left", None).await?;

    //Turn on the light
    kl.set_power(true).await?;

    //Set brightness to 30
    kl.set_brightness(30).await?;

    //Slowly increase the color temperature
    let mut basecolor = 2900;
    while basecolor <= 7000 {
        //Set temperature
        kl.set_temperature(basecolor).await?;

        basecolor = basecolor + 100;

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
