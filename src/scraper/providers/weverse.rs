use rand::rngs::OsRng;
use regex::Regex;
use reqwest::Client;
use rsa::{PaddingScheme, PublicKey, RSAPublicKey};
use serde::{Deserialize, Serialize};
use sha1::Sha1;

/// https://gist.github.com/Xetera/aa59e84f3959a37c16a3309b5d9ab5a0
async fn get_public_key(client: &Client) -> anyhow::Result<RSAPublicKey> {
    let login_page = client
        .post("https://account.weverse.io/login/auth?client_id=weverse-test&hl=en")
        .send()
        .await?
        .text()
        .await?;
    let regex = Regex::new(r"/(static/js/main\..*.js)").unwrap();
    let js_bundle_captures = regex.captures(&login_page).unwrap();

    let js_name = js_bundle_captures
        .get(1)
        .expect("Couldn't match a main js bundle on account.weverse.io, the site was changed")
        .as_str();
    let js_bundle_url = format!("https://account.weverse.io/{}", js_name);
    let js_bundle = client.get(&js_bundle_url).send().await?.text().await?;
    let rsa_captures =
        Regex::new(r"(-----BEGIN RSA PUBLIC KEY-----(.|\n)+----END RSA PUBLIC KEY-----)")
            .unwrap()
            .captures(&js_bundle)
            .expect(&format!(
                "Couldn't find a hardcoded RSA key in {}",
                &js_bundle_url
            ));

    let rsa_key = rsa_captures.get(1).unwrap().as_str().to_owned();

    let der_encoded = rsa_key
        .replace("\\n", "\n")
        .lines()
        .filter(|line| !line.starts_with("-"))
        .fold(String::new(), |mut data, line| {
            data.push_str(&line);
            data
        });

    let der_bytes = base64::decode(&der_encoded).expect("failed to decode base64 content");
    let public_key = RSAPublicKey::from_pkcs8(&der_bytes).expect("failed to parse key");
    Ok(public_key)
}

fn encrypted_password(password: String, public_key: RSAPublicKey) -> anyhow::Result<String> {
    let mut rng = OsRng;
    let padding = PaddingScheme::new_oaep::<Sha1>();
    let encrypted = public_key.encrypt(&mut rng, padding, &password.as_bytes())?;
    Ok(base64::encode(encrypted))
}

#[derive(Serialize)]
struct WeverseLoginRequest {
    grant_type: String,
    client_id: String,
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
pub struct WeverseLoginResponse {
    access_token: String,
    refresh_token: String,
}

async fn get_access_key(
    username: String,
    encrypted_password: String,
    client: &Client,
) -> anyhow::Result<WeverseLoginResponse> {
    Ok(client
        .post("https://accountapi.weverse.io/api/v1/oauth/token")
        .json(&WeverseLoginRequest {
            grant_type: "password".to_owned(),
            client_id: "weverse-test".to_owned(),
            username,
            password: encrypted_password,
        })
        .send()
        .await?
        .json::<WeverseLoginResponse>()
        .await?)
}
