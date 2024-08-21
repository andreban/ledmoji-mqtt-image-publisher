// Copyright 2023 André Cipriani Bandarra
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//

use std::{error::Error, io, path::Path, thread, time::Duration};

use env_logger::Env;
use image::DynamicImage;
use reqwest::ClientBuilder;
use rumqttc::{AsyncClient, Event, Incoming, MqttOptions, Outgoing, QoS};
use serde::Deserialize;
use tokio::task;

const SIZES: [(u32, u32); 2] = [(32, 32), (128, 128)];

// Path to Noto Emoji font directory (https://github.com/googlefonts/noto-emoji)
static ENV_EMOJI_DIRECTORY: &str = "EMOJI_DIRECTORY";

// URL to the firebase database record to listen to.
// eg: 'https://my-firebase-project.firebaseio.com/ledgrids/1.json'
static ENV_FIREBASE_URL: &str = "FIREBASE_URL";

// MQTT client ID to use.
static ENV_MQTT_CLIENT_ID: &str = "MQTT_CLIENT_ID";
static ENV_MQTT_HOST: &str = "MQTT_HOST";
static ENV_MQTT_PORT: &str = "MQTT_PORT";
static DEFAULT_MQTT_PORT: u16 = 1883;

#[derive(Debug, Deserialize)]
struct Config {
    pub emoji_directory: String,
    pub firebase_url: String,
    pub mqtt_client_id: String,
    pub mqtt_server: String,
    pub mqtt_port: u16,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn Error>> {
        let mqtt_port = match std::env::var(ENV_MQTT_PORT) {
            Ok(port) => port.parse()?,
            Err(_) => DEFAULT_MQTT_PORT,
        };

        Ok(Self {
            emoji_directory: std::env::var(ENV_EMOJI_DIRECTORY).unwrap_or_else(|_| {
                panic!("{} environment variable not set", ENV_EMOJI_DIRECTORY);
            }),
            firebase_url: std::env::var(ENV_FIREBASE_URL).unwrap_or_else(|_| {
                panic!("{} environment variable not set", ENV_FIREBASE_URL);
            }),
            mqtt_client_id: std::env::var(ENV_MQTT_CLIENT_ID).unwrap_or_else(|_| {
                panic!("{} environment variable not set", ENV_MQTT_CLIENT_ID);
            }),
            mqtt_server: std::env::var(ENV_MQTT_HOST).unwrap_or_else(|_| {
                panic!("{} environment variable not set", ENV_MQTT_HOST);
            }),
            mqtt_port: mqtt_port,
        })
    }
}

#[derive(Debug, Deserialize)]
struct PayloadData {
    emoji: String,
}

#[derive(Debug, Deserialize)]
struct Payload {
    data: PayloadData,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(Env::default().default_filter_or("daemon=info")).init();

    let config: Config = Config::from_env()?;

    let mut mqttoptions =
        MqttOptions::new(config.mqtt_client_id, config.mqtt_server, config.mqtt_port);
    mqttoptions.set_max_packet_size(usize::MAX, usize::MAX);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (mqtt_client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    // Spawn a task to run the eventloop and ensure tasks progress.
    task::spawn(async move {
        loop {
            let notification = eventloop.poll().await;
            match notification {
                Ok(Event::Incoming(Incoming::PingResp) | Event::Outgoing(Outgoing::PingReq)) => {
                    continue
                }
                Ok(notification) => log::info!("Notification = {:?}", notification),
                Err(e) => log::error!("Error = {:?}", e),
            }
        }
    });

    // Listen for events from Firebase.
    let http_client = ClientBuilder::new().build()?;
    loop {
        let Ok(mut response) = http_client
            .get(&config.firebase_url)
            .header("Accept", "text/event-stream")
            .send()
            .await
        else {
            log::error!("Failed to get Firebase URL");
            continue;
        };

        loop {
            let Ok(chunk) = tokio::time::timeout(Duration::from_secs(60), response.chunk()).await
            else {
                log::error!("Timed out getting chunk");
                break;
            };

            let Ok(Some(chunk)) = chunk else {
                log::error!("Failed to get chunk");
                break;
            };

            let chunk_vec = chunk.to_vec();
            let chunk_str = String::from_utf8_lossy(&chunk_vec);
            let lines = chunk_str.lines().collect::<Vec<_>>();
            if lines.len() < 2 {
                log::error!("Not enough lines. Skipping...");
            }

            let Ok((_, command)) = parse_chunk_line(lines[0]) else {
                log::error!("Failed to parse command: {:?}. Skipping...", lines);
                continue;
            };

            match command {
                "put" => {
                    log::info!("Received command {}", command);
                    let (_, data) = parse_chunk_line(lines[1])?;
                    let emoji = serde_json::from_str::<Payload>(data).unwrap().data.emoji;

                    let Ok(img) = load_emoji_image(&config.emoji_directory, &emoji) else {
                        log::error!("Failed to load emoji image for {}", emoji);
                        continue;
                    };

                    for (width, height) in SIZES {
                        let out = img
                            .resize(width, height, image::imageops::FilterType::Nearest)
                            .to_rgb8()
                            .to_vec();
                        let topic = format!("ledmoji/{}x{}", width, height);
                        let result = mqtt_client
                            .publish(&topic, QoS::AtLeastOnce, true, out)
                            .await;
                        match result {
                            Ok(_) => log::info!("Published {emoji} to {topic}"),
                            Err(e) => {
                                log::error!("Failed to publish {} to {}: {}", emoji, topic, e)
                            }
                        };
                        thread::sleep(Duration::from_millis(100));
                    }
                }
                "keep-alive" => {
                    log::debug!("Received keep-alive command");
                    continue;
                }
                command => {
                    log::info!("Ignoring unknown command {}", command);
                    continue;
                }
            }
        }
    }
}

fn load_emoji_image(emoji_directory: &str, emoji: &str) -> Result<DynamicImage, Box<dyn Error>> {
    let unicode = emoji
        .escape_unicode()
        .to_string()
        .replacen("\\u", "emoji_u", 1)
        .replace("\\u", "_")
        .replace(['{', '}'], "");

    let mut filename = emoji_directory.to_string() + "/" + &unicode + ".png";
    if !Path::new(&filename).exists() {
        let previous_unicode = unicode.rsplitn(2, '_').last().unwrap();
        filename = emoji_directory.to_string() + "/" + previous_unicode + ".png";
    }

    Ok(image::open(filename)?)
}

fn parse_chunk_line(input: &str) -> io::Result<(&str, &str)> {
    let parts = input.splitn(2, ':').map(|s| s.trim()).collect::<Vec<_>>();

    if parts.len() < 2 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid input"));
    }

    Ok(((parts[0]), (parts[1])))
}

#[cfg(test)]
mod test {
    #[test]
    fn test_parse_chunk_line() {
        let input = "event: put\ndata: {\"emoji\":\"👍\"}\n\n";
        let (command, data) = super::parse_chunk_line(input).unwrap();
        assert_eq!(command, "event");
        assert_eq!(data, "put\ndata: {\"emoji\":\"👍\"}");
    }
}
