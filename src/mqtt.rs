use core::{num::NonZero, sync::atomic::Ordering};

use alloc::string::{String, ToString};

use embassy_futures::select::{select, Either};
use embassy_net::{dns::DnsQueryType, tcp::TcpSocket, IpAddress, Stack};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Sender};
use embassy_time::{with_timeout, Duration, Instant, Timer};
use esp_println::println;
use rust_mqtt::{
  buffer::{AllocBuffer, BufferProvider},
  client::{
    event::{Event, Publish},
    options::{
      ConnectOptions, PublicationOptions, SubscriptionOptions, TopicReference, WillOptions,
    },
    Client,
  },
  config::KeepAlive,
  io::Transport,
  types::{MqttBinary, MqttString, TopicFilter, TopicName},
  Bytes,
};

use crate::Command;

#[derive(Debug)]
enum Error {
  Dns,
  Tcp,
  Timeout,
  Mqtt(String),
}

impl Error {
  fn mqtt(context: &str, detail: impl core::fmt::Debug) -> Self {
    Error::Mqtt(alloc::format!("{context}: {detail:?}"))
  }
}

impl core::fmt::Display for Error {
  fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
    match self {
      Error::Dns => write!(f, "DNS lookup for {} failed", crate::MQTT_HOST),
      Error::Tcp => write!(
        f,
        "TCP connection to {}:{} failed",
        crate::MQTT_HOST,
        crate::MQTT_PORT
      ),
      Error::Timeout => write!(
        f,
        "TCP connection to {}:{} timed out",
        crate::MQTT_HOST,
        crate::MQTT_PORT
      ),
      Error::Mqtt(detail) => f.write_str(detail),
    }
  }
}

#[embassy_executor::task]
pub async fn task(
  stack: Stack<'static>,
  commands: Sender<'static, CriticalSectionRawMutex, Command, 8>,
) -> ! {
  loop {
    stack.wait_config_up().await;
    match session(stack, commands).await {
      Ok(()) => println!("MQTT session ended"),
      Err(e) => println!("MQTT error: {}", e),
    }
    crate::MQTT_UP.store(false, Ordering::Relaxed);
    Timer::after(Duration::from_secs(5)).await;
  }
}

async fn resolve(stack: Stack<'static>) -> Result<IpAddress, Error> {
  if let Ok(ip) = crate::MQTT_HOST.parse::<core::net::Ipv4Addr>() {
    let o = ip.octets();
    return Ok(IpAddress::v4(o[0], o[1], o[2], o[3]));
  }
  let addrs = stack
    .dns_query(crate::MQTT_HOST, DnsQueryType::A)
    .await
    .map_err(|_| Error::Dns)?;
  addrs.first().copied().ok_or(Error::Dns)
}

async fn session(
  stack: Stack<'static>,
  commands: Sender<'static, CriticalSectionRawMutex, Command, 8>,
) -> Result<(), Error> {
  let ip = resolve(stack).await?;
  println!("Connecting to MQTT server {}:{}", ip, crate::MQTT_PORT);

  let mut rx = [0u8; 1024];
  let mut tx = [0u8; 1024];
  let mut socket = TcpSocket::new(stack, &mut rx, &mut tx);
  socket.set_timeout(Some(Duration::from_millis(crate::MQTT_KEEPALIVE_MS * 2)));

  match with_timeout(
    Duration::from_secs(10),
    socket.connect((ip, crate::MQTT_PORT)),
  )
  .await
  {
    Ok(Ok(())) => {}
    Ok(Err(_)) => return Err(Error::Tcp),
    Err(_) => return Err(Error::Timeout),
  }

  let mut buffer = AllocBuffer;
  let mut client = Client::<'_, '_, _, _, 2, 1, 1, 1, 16, 0, 1>::new(&mut buffer);

  let id = alloc::format!("cat-{}", crate::CAT_NAME_LOWER);
  let connected_topic = alloc::format!("{}/connected", crate::CAT_NAME);
  let status_topic = alloc::format!("{}/status", crate::CAT_NAME);
  let config_topic = alloc::format!("{}/config/set", crate::CAT_NAME);
  let reset_topic = alloc::format!("{}/reset/set", crate::CAT_NAME);
  let command_filter = alloc::format!("{}/#", crate::CAT_NAME);
  let allcats_filter = String::from("winkekatze/allcats/#");

  let will_topic = topic_name(&connected_topic)?;
  let will = WillOptions::new(
    will_topic,
    MqttBinary::try_from("0").map_err(|e| Error::mqtt("encoding will payload", e))?,
  )
  .retain();

  let keepalive_secs = (crate::MQTT_KEEPALIVE_MS / 1000).clamp(1, u16::MAX as u64) as u16;
  let options = ConnectOptions::new()
    .clean_start()
    .will(will)
    .keep_alive(KeepAlive::Seconds(NonZero::new(keepalive_secs).unwrap()));
  let client_id =
    MqttString::try_from(id.as_str()).map_err(|e| Error::mqtt("encoding client id", e))?;

  client
    .connect(socket, &options, Some(client_id))
    .await
    .map_err(|e| Error::mqtt("connect", e))?;
  println!(
    "Connected to MQTT as '{}', keepalive {}ms",
    id,
    crate::MQTT_KEEPALIVE_MS
  );

  let own_filter = TopicFilter::new(
    MqttString::try_from(command_filter.as_str())
      .map_err(|e| Error::mqtt("encoding subscription filter", e))?,
  )
  .ok_or_else(|| Error::Mqtt(String::from("invalid subscription filter topic")))?;
  client
    .subscribe(own_filter, &SubscriptionOptions::new())
    .await
    .map_err(|e| Error::mqtt("subscribe to own topic", e))?;

  let allcats = TopicFilter::new(
    MqttString::try_from(allcats_filter.as_str())
      .map_err(|e| Error::mqtt("encoding allcats filter", e))?,
  )
  .ok_or_else(|| Error::Mqtt(String::from("invalid allcats filter topic")))?;
  client
    .subscribe(allcats, &SubscriptionOptions::new())
    .await
    .map_err(|e| Error::mqtt("subscribe to allcats", e))?;

  publish(&mut client, &connected_topic, "1", true).await?;

  crate::MQTT_UP.store(true, Ordering::Relaxed);
  println!("Boot complete, waving with green eyes");
  let _ = commands.try_send(Command::Color(crate::eye_leds::GREEN));
  let _ = commands.try_send(Command::Wave);

  let ping_every = Duration::from_millis((crate::MQTT_KEEPALIVE_MS / 2).max(1));
  let mut next_ping = Instant::now() + ping_every;
  loop {
    let r = select(Timer::at(next_ping), client.poll_header()).await;
    match r {
      Either::First(()) => {
        next_ping = Instant::now() + ping_every;
        client.ping().await.map_err(|e| Error::mqtt("ping", e))?
      }
      Either::Second(header) => {
        let header = header.map_err(|e| Error::mqtt("poll header", e))?;
        let published = match client
          .poll_body(header)
          .await
          .map_err(|e| Error::mqtt("poll body", e))?
        {
          Event::Publish(Publish { topic, message, .. }) => topic.name().map(|name| {
            (
              String::from(name.as_ref().as_str()),
              String::from_utf8_lossy(message.as_bytes())
                .trim()
                .to_string(),
            )
          }),
          Event::Pingresp => None,
          event => {
            println!("MQTT: {:?}", event);
            None
          }
        };

        if let Some((topic, message)) = published {
          if topic != connected_topic && topic != status_topic {
            println!("Message \"{}\" received via topic \"{}\"", topic, message);
          }

          if topic.ends_with("eye/set") {
            if let Some(color) = crate::eye_leds::color_by_name(&message) {
              let _ = commands.try_send(Command::Color(color));
            }
          } else if topic == "winkekatze/allcats" || topic.ends_with("command") {
            publish(&mut client, &status_topic, "fishing", false).await?;
            let _ = commands.try_send(Command::Wave);
          } else if topic == reset_topic {
            esp_hal::system::software_reset();
          } else if topic == config_topic {
            println!("Ignoring topic {}: no persistent config", topic);
          } else if topic != connected_topic && topic != status_topic {
            println!("Meow!");
          }
        }
      }
    }
  }
}

fn topic_name(name: &str) -> Result<TopicName<'_>, Error> {
  TopicName::new(MqttString::try_from(name).map_err(|e| Error::mqtt("encoding topic name", e))?)
    .ok_or_else(|| Error::Mqtt(alloc::format!("invalid topic name '{name}'")))
}

async fn publish<
  'a,
  'c,
  N,
  B,
  const S: usize,
  const R: usize,
  const SD: usize,
  const SI: usize,
  const UP: usize,
  const IA: usize,
  const OA: usize,
>(
  client: &mut Client<'a, 'c, N, B, S, R, SD, SI, UP, IA, OA>,
  topic: &str,
  message: &str,
  retain: bool,
) -> Result<(), Error>
where
  N: Transport,
  B: BufferProvider<'c>,
{
  let name = topic_name(topic)?;
  let mut options = PublicationOptions::new(TopicReference::Name(name));
  if retain {
    options = options.retain();
  }
  client
    .publish(&options, Bytes::from(message))
    .await
    .map_err(|e| Error::mqtt(&alloc::format!("publish to '{topic}'"), e))?;
  Ok(())
}
