use core::{num::NonZero, sync::atomic::Ordering};

use embassy_futures::select::{select, Either};
use embassy_net::{dns::DnsQueryType, tcp::TcpSocket, IpAddress, Stack};
use embassy_time::{with_timeout, Duration, Timer};
use esp_println::println;
use rust_mqtt::{
    buffer::AllocBuffer,
    client::{options::ConnectOptions, Client},
    config::KeepAlive,
    types::MqttString,
};

#[derive(Debug)]
enum Error {
    Dns,
    Tcp,
    Timeout,
    Mqtt,
}

#[embassy_executor::task]
pub async fn task(stack: Stack<'static>) -> ! {
    loop {
        stack.wait_config_up().await;
        match session(stack).await {
            Ok(()) => println!("MQTT session ended"),
            Err(e) => println!("MQTT error: {:?}", e),
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
    let addrs = stack.dns_query(crate::MQTT_HOST, DnsQueryType::A).await.map_err(|_| Error::Dns)?;
    addrs.first().copied().ok_or(Error::Dns)
}

async fn session(stack: Stack<'static>) -> Result<(), Error> {
    let ip = resolve(stack).await?;
    println!("Connecting to MQTT server {}:{}", ip, crate::MQTT_PORT);

    let mut rx = [0u8; 1024];
    let mut tx = [0u8; 1024];
    let mut socket = TcpSocket::new(stack, &mut rx, &mut tx);
    socket.set_timeout(Some(Duration::from_secs(crate::MQTT_KEEPALIVE_SECS as u64 * 2)));

    match with_timeout(Duration::from_secs(10), socket.connect((ip, crate::MQTT_PORT))).await {
        Ok(Ok(())) => {}
        Ok(Err(_)) => return Err(Error::Tcp),
        Err(_) => return Err(Error::Timeout),
    }

    let mut buffer = AllocBuffer;
    let mut client = Client::<'_, '_, _, _, 1, 1, 1, 1, 16, 0, 1>::new(&mut buffer);

    let id = alloc::format!("cat-{}", crate::CAT_NAME_LOWER);
    let options = ConnectOptions::new()
        .clean_start()
        .keep_alive(KeepAlive::Seconds(NonZero::new(crate::MQTT_KEEPALIVE_SECS).unwrap()));
    let client_id = MqttString::try_from(id.as_str()).map_err(|_| Error::Mqtt)?;

    client.connect(socket, &options, Some(client_id)).await.map_err(|e| {
        println!("MQTT connection failed: {:?}", e);
        Error::Mqtt
    })?;
    println!("Connected to MQTT as '{}', keepalive {}s", id, crate::MQTT_KEEPALIVE_SECS);
    crate::MQTT_UP.store(true, Ordering::Relaxed);

    let ping_every = Duration::from_secs((crate::MQTT_KEEPALIVE_SECS as u64 / 2).max(1));
    loop {
        let r = select(Timer::after(ping_every), client.poll_header()).await;
        match r {
            Either::First(()) => client.ping().await.map_err(|e| {
                println!("MQTT ping failed: {:?}", e);
                Error::Mqtt
            })?,
            Either::Second(header) => {
                let header = header.map_err(|e| {
                    println!("MQTT error in poll header: {:?}", e);
                    Error::Mqtt
                })?;
                let event = client.poll_body(header).await.map_err(|e| {
                    println!("MQTT error in poll body: {:?}", e);
                    Error::Mqtt
                })?;
                println!("mqtt: {:?}", event);
            }
        }
    }
}
