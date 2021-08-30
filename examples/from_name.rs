use elgato_keylight::KeyLight;

#[tokio::main]
async fn main() {
    let mut kl = KeyLight::new_from_name("Key Light Left").await.unwrap();

    dbg!(&kl);

    kl.set_brightness(10).await.unwrap();
}
