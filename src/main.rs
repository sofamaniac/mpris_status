use std::{collections::HashMap, io::Read, time::Duration, vec};

use futures_util::stream::StreamExt;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;
use zbus::{dbus_proxy, Connection};

#[dbus_proxy(
    interface = "org.mpris.MediaPlayer2.Player",
    default_path = "/org/mpris/MediaPlayer2"
)]
trait Player {
    /// Metadata property
    #[dbus_proxy(property)]
    fn metadata(&self) -> zbus::Result<HashMap<String, zbus::zvariant::OwnedValue>>;
}

fn string_len(string: &str) -> usize {
    UnicodeWidthStr::width_cjk(string)
}

fn split<'a>(chars: &'a [&'a str], limit: usize) -> Vec<String> {
    let mut title = chars.to_vec();
    let mut res = Vec::new();
    // If the title is short enough, no need to build chunks
    if string_len(&chars.join("")) <= limit {
        res.push(chars.join(""));
        return res;
    };
    // title = "TITLE {space} TITLE" with limit/2 spaces
    title.resize(title.len() + limit / 2, "\u{00A0}");
    for c in chars {
        title.push(c);
    }
    for index in 0..(chars.len() + limit / 2) {
        let mut end = index;
        while string_len(&title[index..end].join("")) < limit {
            end += 1;
        }
        if string_len(&title[index..end].join("")) > limit {
            end -= 1;
        }
        let mut s = title[index..end].join("");
        // pad right to the correct length
        while string_len(&s) < limit {
            s.push('\u{00A0}');
        }
        res.push(s);
    }
    res
}

fn get_player() -> String {
    let mut file = match std::fs::File::open("/var/tmp/player_selector") {
        Ok(f) => f,
        Err(_) => return "whatever".to_string(),
    };

    let mut player = String::new();
    match file.read_to_string(&mut player) {
        Ok(_) => player.trim().to_string(),
        Err(_) => "whatever".to_string(),
    }
}

struct ConnectionHandler<'a> {
    connection: Connection,
    proxy: PlayerProxy<'a>,
    player: String,
    pub stream: zbus::PropertyStream<'a, HashMap<String, zbus::zvariant::OwnedValue>>,
}

impl<'a> ConnectionHandler<'a> {
    pub async fn update(&mut self) -> Result<(), zbus::Error> {
        self.player = get_player();
        self.proxy = PlayerProxy::builder(&self.connection)
            .destination(format!("org.mpris.MediaPlayer2.{}", self.player))?
            .build()
            .await?;
        self.stream = self.proxy.receive_metadata_changed().await;
        Ok(())
    }
    pub async fn new() -> Result<Self, zbus::Error> {
        let connection = Connection::session().await?;
        let player = get_player();
        let proxy = PlayerProxy::builder(&connection)
            .destination(format!("org.mpris.MediaPlayer2.{}", player))?
            .build()
            .await?;
        let stream = proxy.receive_metadata_changed().await;
        Ok(ConnectionHandler {
            connection,
            proxy,
            player,
            stream,
        })
    }
    pub fn need_update(&self) -> bool {
        get_player() != self.player
    }
}

const LIMIT: usize = 20; // chunks length
const SHIFT_DELAY: Duration = Duration::from_millis(500);
const INIT_DELAY: u64 = 5; // INIT_DELAY * SHIFT_DELAY ms to wait before starting scrolling

#[tokio::main]
async fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut connection_handler = ConnectionHandler::new().await?;
    let mut chunks: Vec<String> = vec![];
    let mut index = 0;
    let mut wait = 2;
    loop {
        while let Ok(Some(v)) =
            tokio::time::timeout(SHIFT_DELAY, connection_handler.stream.next()).await
        {
            let data = v.get().await?;
            let title: &str = match data.get("xesam:title") {
                Some(t) => t.downcast_ref().unwrap(),
                None => continue,
            };
            let chars = UnicodeSegmentation::graphemes(title, true).collect::<Vec<&str>>();
            chunks = split(&chars, LIMIT);
            index = 0;
            wait = INIT_DELAY;
        }
        if index < chunks.len() {
            println!("{}", chunks[index]);
        }
        if wait == 0 {
            index += 1;
        } else {
            wait -= 1;
        }
        if connection_handler.need_update() || index >= chunks.len() {
            // Update the handler if the player has changed
            // but also force update periodically
            connection_handler.update().await?;
        }
    }
}
