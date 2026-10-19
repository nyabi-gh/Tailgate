//! Manual check of the in-app tunnel against a real Mullvad relay.
//!
//!   cargo run --example tunnel_probe -- [location-prefix] [--serve]
//!   cargo run --example tunnel_probe -- --forget
//!
//! Set `PROBE_DOWNLOAD=<url>` to also time a download of that URL through the tunnel.
//!
//! Reads the account number from the credential store entry `mullvad-account` and keeps the
//! registered device in `mullvad-device`, so repeated runs reuse one device slot.

use std::{env, error::Error, time::Instant};

use tailgate_lib::{
    network::USER_AGENT,
    secrets,
    tunnel::{
        Tunnel,
        mullvad::{Mullvad, RegisteredDevice},
        socks::{Route, SocksServer},
    },
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();
    let args: Vec<String> = env::args().skip(1).collect();
    let serve = args.iter().any(|arg| arg == "--serve");
    let forget = args.iter().any(|arg| arg == "--forget");
    let location = args
        .iter()
        .find(|arg| !arg.starts_with("--"))
        .map_or("jp-tyo", String::as_str);

    secrets::init()?;
    let account = secrets::get(secrets::MULLVAD_ACCOUNT)?.ok_or(
        "no `mullvad-account` entry in the credential store (service pro.tb.nyabi.Tailgate)",
    )?;
    let mullvad = Mullvad::new(reqwest::Client::builder().user_agent(USER_AGENT).build()?);

    let stored = secrets::get(secrets::MULLVAD_DEVICE)?;
    if forget {
        let Some(json) = stored else {
            println!("no stored device");
            return Ok(());
        };
        let device: RegisteredDevice = serde_json::from_str(&json)?;
        let token = mullvad.access_token(account.trim()).await?;
        mullvad.remove_device(&token, &device.id).await?;
        secrets::delete(secrets::MULLVAD_DEVICE)?;
        println!("removed device {}", device.name);
        return Ok(());
    }

    let device = match stored {
        Some(json) => serde_json::from_str::<RegisteredDevice>(&json)?,
        None => {
            let token = mullvad.access_token(account.trim()).await?;
            let device = mullvad.register_device(&token).await?;
            secrets::set(secrets::MULLVAD_DEVICE, &serde_json::to_string(&device)?)?;
            println!("registered new device {} ({})", device.name, device.ipv4);
            device
        }
    };

    let relays = mullvad.relays().await?;
    let relay = relays
        .iter()
        .find(|relay| relay.active && relay.location.starts_with(location))
        .ok_or_else(|| format!("no active relay matching {location}"))?;
    println!("relay {} ({})", relay.hostname, relay.ipv4_addr_in);

    let started = Instant::now();
    let tunnel = Tunnel::start(&device.tunnel_config(relay)?).await?;
    let socks = SocksServer::bind(Route::Tunnel(tunnel.stack().clone())).await?;
    println!(
        "tunnel up in {:?}, SOCKS5 at {}",
        started.elapsed(),
        socks.address()
    );

    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .proxy(reqwest::Proxy::all(format!(
            "socks5h://{}",
            socks.address()
        ))?)
        .build()?;

    let started = Instant::now();
    let ip: serde_json::Value = client
        .get("https://am.i.mullvad.net/json")
        .send()
        .await?
        .json()
        .await?;
    println!(
        "exit check in {:?}: ip {} via mullvad={} ({})",
        started.elapsed(),
        ip["ip"],
        ip["mullvad_exit_ip"],
        ip["mullvad_exit_ip_hostname"]
    );

    let started = Instant::now();
    let response = client
        .get("https://e621.net/posts.json?limit=5&tags=rating:s")
        .send()
        .await?;
    let status = response.status();
    let body: serde_json::Value = response.json().await?;
    println!("e621 API {status} in {:?}", started.elapsed());
    let posts = body["posts"].as_array().cloned().unwrap_or_default();
    for post in &posts {
        println!(
            "  post {} {}x{} {}",
            post["id"], post["file"]["width"], post["file"]["height"], post["file"]["ext"]
        );
    }

    if let Some(url) = posts
        .iter()
        .filter_map(|post| post["file"]["url"].as_str())
        .next()
    {
        let started = Instant::now();
        let bytes = client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        let seconds = started.elapsed().as_secs_f64();
        println!(
            "original file {:.1} MB in {:.2}s = {:.2} MB/s",
            bytes.len() as f64 / 1e6,
            seconds,
            bytes.len() as f64 / 1e6 / seconds
        );
    }

    if let Ok(url) = env::var("PROBE_DOWNLOAD") {
        let started = Instant::now();
        let bytes = client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        let seconds = started.elapsed().as_secs_f64();
        println!(
            "PROBE_DOWNLOAD {:.1} MB in {:.2}s = {:.2} MB/s",
            bytes.len() as f64 / 1e6,
            seconds,
            bytes.len() as f64 / 1e6 / seconds
        );
    }

    if serve {
        println!("serving SOCKS5 at {} until Ctrl-C", socks.address());
        tokio::signal::ctrl_c().await?;
    }
    drop(socks);
    tunnel.stop().await;
    Ok(())
}
