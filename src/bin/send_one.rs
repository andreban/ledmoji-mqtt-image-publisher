use std::{error::Error, thread, time::Duration};

use env_logger::Env;
use image::{GenericImageView, Rgb};
use mqtt_image_writer::imageutils::merge_colors;
use rumqttc::{Client, Event, MqttOptions, Packet, QoS};

const BACKGROUND_COLOR: Rgb<u8> = Rgb([0, 0, 0]);

fn main() -> Result<(), Box<dyn Error>> {
    env_logger::Builder::from_env(Env::default().default_filter_or("send_one=debug")).init();

    // Load image...
    let img =
        image::open("./assets/chrome.png")?.resize(32, 32, image::imageops::FilterType::Nearest);
    let width = img.width();
    let height = img.height();
    let img = img
        .resize(width, height, image::imageops::FilterType::Nearest)
        .pixels()
        .flat_map(|(_x, _y, rgba)| merge_colors(&rgba, &BACKGROUND_COLOR))
        .collect::<Vec<_>>();

    let mut mqttoptions = MqttOptions::new("send-one", "brucebanner.local", 1883);
    mqttoptions.set_max_packet_size(usize::MAX, usize::MAX);
    mqttoptions.set_keep_alive(Duration::from_secs(5));

    let (mut client, mut connection) = Client::new(mqttoptions, 10);
    thread::spawn(move || {
        client
            .publish(
                format!("ledmoji/{width}x{height}"),
                QoS::AtLeastOnce,
                true,
                img,
            )
            .unwrap();
        thread::sleep(Duration::from_millis(100));
    });

    // Iterate to poll the eventloop for connection progress
    for notification in connection.iter() {
        log::info!("Notification = {:?}", notification);

        // Server acknowledged receiving our package. We can quit :D.
        if let Ok(Event::Incoming(Packet::PubAck(_))) = notification {
            break;
        }
    }
    Ok(())
}
