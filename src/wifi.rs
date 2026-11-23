use core::sync::atomic::Ordering;

use embassy_executor::Spawner;
use embassy_net::{Config, DhcpConfig, Runner, Stack, StackResources};
use embassy_time::{Duration, Timer};
use esp_hal::{peripherals::WIFI, rng::Rng};
use esp_println::println;
use esp_radio::wifi::{
    sta::StationConfig, AuthenticationMethodConfig, Config as WifiConfig, ControllerConfig,
    Interface, WifiController,
};
use static_cell::StaticCell;

static RESOURCES: StaticCell<StackResources<5>> = StaticCell::new();

pub fn start(spawner: Spawner, wifi: WIFI<'static>) -> Stack<'static> {
    let mut controller =
        WifiController::new(wifi, ControllerConfig::default()).expect("Error: WiFi initialisation failed!");

    let station = StationConfig::default()
        .with_ssid(crate::WIFI_SSID.try_into().unwrap())
        .with_authentication(AuthenticationMethodConfig::Wpa2Personal(
            crate::WIFI_PASSWORD.try_into().unwrap(),
        ));
    controller.set_config(&WifiConfig::Station(station)).unwrap();

    let rng = Rng::new();
    let seed = ((rng.random() as u64) << 32) | rng.random() as u64;

    let mut dhcp = DhcpConfig::default();
    dhcp.hostname = Some(
        crate::CAT_NAME_LOWER
            .try_into()
            .expect("cat.name must be at most 32 bytes for DHCP"),
    );

    let (stack, runner) = embassy_net::new(
        Interface::station(),
        Config::dhcpv4(dhcp),
        RESOURCES.init(StackResources::new()),
        seed,
    );

    spawner.spawn(net_task(runner).unwrap());
    spawner.spawn(connection_task(controller).unwrap());
    spawner.spawn(ip_task(stack).unwrap());
    stack
}

#[embassy_executor::task]
async fn ip_task(stack: Stack<'static>) -> ! {
    loop {
        stack.wait_config_up().await;
        if let Some(cfg) = stack.config_v4() {
            // embassy-net does not expose the DHCP server identifier (option 54),
            // so use the gateway, which is normally the DHCP server anyway.
            match cfg.gateway {
                Some(gateway) => println!(
                    "DHCP: acquired {}/{} from server {} (hostname '{}')",
                    cfg.address.address(),
                    cfg.address.prefix_len(),
                    gateway,
                    crate::CAT_NAME_LOWER
                ),
                None => println!(
                    "DHCP: acquired {}/{} (DHCP server unknown, hostname '{}')",
                    cfg.address.address(),
                    cfg.address.prefix_len(),
                    crate::CAT_NAME_LOWER
                ),
            }
        }
        stack.wait_config_down().await;
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, Interface>) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn connection_task(mut controller: WifiController<'static>) {
    loop {
        match controller.connect_async().await {
            Ok(_) => {
                println!("Connected to '{}'", crate::WIFI_SSID);
                crate::WIFI_UP.store(true, Ordering::Relaxed);
                let _ = controller.wait_for_disconnect_async().await;
                crate::WIFI_UP.store(false, Ordering::Relaxed);
                println!("WiFi connection lost, will try again...");
            }
            Err(e) => {
                println!("WiFi connection failed: {:?}", e);
                Timer::after(Duration::from_secs(5)).await;
            }
        }
    }
}
