use std::{error::Error, thread, time::Duration};

use env_logger::Env;
use image::{GenericImageView, Rgb, Rgba};
use rumqttc::{Client, Event, MqttOptions, Packet, QoS};

fn merge_colors(foreground: &Rgba<u8>, background: &Rgb<u8>) -> Vec<u8> {
    // Foreground is opaque, just return the color.
    if foreground.0[3] == 255 {
        return vec![foreground.0[0], foreground.0[1], foreground.0[2]];
    }

    // Convert the factor from u8 to f32, so that 0 is 0.0 and 255 is 1.0.
    let factor = foreground.0[3] as f32 / 255.0;

    // Function for mixing foreground and background colors.
    let map_channel = |fg_color: u8, bg_color: u8| {
        ((bg_color as f32 * (1.0 - factor)) + fg_color as f32 * factor) as u8
    };

    // Zip over fb and bg colors, converting to the output color.
    foreground.0
        .into_iter()
        .zip(background.0)
        .map(|(fg, bg)| map_channel(fg, bg))
        .collect::<Vec<_>>()
}

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
        .flat_map(|(_x, _y, rgba)| merge_colors(&rgba, &Rgb([0, 0, 0])))
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
