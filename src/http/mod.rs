mod routes;

use rocket::{
    Build, Rocket,
    data::{Limits, ToByteUnit},
};

use routes::{default_error, health, match_glossary, prepare_translation_prompt};

pub fn build() -> Rocket<Build> {
    let figment = rocket::Config::figment()
        .merge(("limits", Limits::default().limit("json", 16.mebibytes())));

    rocket::custom(figment)
        .mount("/", rocket::routes![health])
        .mount(
            "/api/v1",
            rocket::routes![match_glossary, prepare_translation_prompt,],
        )
        .register("/", rocket::catchers![default_error])
}

pub async fn launch() -> Result<(), rocket::Error> {
    build().launch().await?;

    Ok(())
}
