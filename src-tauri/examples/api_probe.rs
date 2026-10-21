//! Manual check of the API layer against the real e621, through the route the app would use on
//! a first run (the saved Mullvad device at `jp-tyo`, or direct when there is none).
//!
//!   cargo run --example api_probe -- [tags]
//!
//! Prints what each call returned, so a change in e621's responses shows up as a decode error.

use std::{env, error::Error, sync::Arc, time::Instant};

use tailgate_lib::{
    api::{Api, PostSearch},
    network::{Connection, Network, UserAgent},
    secrets,
    settings::Settings,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info".into()))
        .init();
    let tags = env::args().skip(1).collect::<Vec<_>>().join(" ");
    let tags = if tags.is_empty() { "wolf".into() } else { tags };

    secrets::init()?;
    // No settings file: the first-run choice, without touching the app's own settings.
    let scratch = env::temp_dir().join("tailgate-api-probe");
    let settings = Arc::new(Settings::load(scratch.join("settings.json")));
    let network = Arc::new(Network::start(settings, scratch.join("relays.json")).await?);
    tokio::spawn(Arc::clone(&network).run());
    let connection = network
        .watch_connection()
        .wait_for(|connection| *connection != Connection::Connecting)
        .await?
        .clone();
    match connection {
        connection @ (Connection::Direct | Connection::Tunnel { .. }) => {
            println!("route: {connection:?}");
        }
        other => return Err(format!("no route: {other:?}").into()),
    }
    let api = Api::new(network.http().clone(), UserAgent::new());
    let started = Instant::now();
    let at = || format!("{:>6.2}s", started.elapsed().as_secs_f64());

    println!("{} me: {:?}", at(), api.me().await?);

    let mut search = PostSearch::new(&tags);
    for _ in 0..2 {
        let posts = search.next_page(&api).await?;
        let (first, last) = (
            posts.first().map(|post| post.id),
            posts.last().map(|post| post.id),
        );
        println!(
            "{} search {tags:?}: {} posts, {first:?}..{last:?}, done={}",
            at(),
            posts.len(),
            search.is_done()
        );
    }

    let mut ordered = PostSearch::new(&format!("{tags} order:score"));
    let posts = ordered.next_page(&api).await?;
    println!(
        "{} search {tags:?} order:score: {} posts, top score {:?}",
        at(),
        posts.len(),
        posts.first().map(|post| post.score.total)
    );

    let videos = PostSearch::new("type:webm").next_page(&api).await?;
    let video = videos.first().ok_or("no webm posts")?;
    println!(
        "{} webm {}: 720p={:?} mp4={:?}",
        at(),
        video.id,
        video
            .video
            .as_ref()
            .and_then(|video| video.p720.as_ref())
            .map(|source| &source.url),
        video
            .video
            .as_ref()
            .and_then(|video| video.mp4.as_ref())
            .map(|source| &source.url)
    );

    let deleted = PostSearch::new("status:deleted").next_page(&api).await?;
    println!(
        "{} deleted: {} posts, {} with a file URL",
        at(),
        deleted.len(),
        deleted
            .iter()
            .filter(|post| post.file.url.is_some())
            .count()
    );

    let post = api.post(video.id).await?;
    println!(
        "{} post {}: {} tags",
        at(),
        post.id,
        post.tags.general.len()
    );

    for text in ["wol", "zzzzqqqq"] {
        let suggestions = api.autocomplete(text).await?;
        println!(
            "{} autocomplete {text:?}: {:?}",
            at(),
            suggestions
                .iter()
                .take(3)
                .map(|tag| (&tag.name, tag.category, tag.post_count))
                .collect::<Vec<_>>()
        );
    }
    Ok(())
}
