use embassy_time::{Duration, Timer};
use esp_hal::{
    gpio::DriveMode,
    ledc::{
        channel::{self, Channel, ChannelIFace},
        timer::TimerIFace,
        LowSpeed,
    },
};

pub struct Servo<'a> {
    channel: Channel<'a, LowSpeed>,
    back: u8,
    front: u8,
    middle: u8,
    pause_ms: u64,
}

impl<'a> Servo<'a> {
    pub fn new(mut channel: Channel<'a, LowSpeed>, timer: &'a dyn TimerIFace<LowSpeed>) -> Self {
        channel
            .configure(channel::config::Config {
                timer,
                duty_pct: 0,
                drive_mode: DriveMode::PushPull,
            })
            .unwrap();

        Self {
            channel,
            back: 5,
            front: 10,
            middle: 7,
            pause_ms: 200,
        }
    }

    pub async fn wave(&mut self) {
        self.set(self.middle);
        Timer::after(Duration::from_millis(self.pause_ms)).await;
        self.set(self.front);
        Timer::after(Duration::from_millis(self.pause_ms)).await;
        self.set(self.back);
        Timer::after(Duration::from_millis(self.pause_ms * 3)).await;
        self.set(self.front);
        Timer::after(Duration::from_millis(self.pause_ms * 3)).await;
        self.set(self.back);
        Timer::after(Duration::from_millis(self.pause_ms * 3)).await;
        self.set(self.middle);
        Timer::after(Duration::from_millis(self.pause_ms * 3)).await;
        self.set(0);
    }

    pub fn wave_duration(&self) -> Duration {
        Duration::from_millis(self.pause_ms * 14)
    }

    fn set(&self, duty_pct: u8) {
        let _ = self.channel.set_duty(duty_pct);
    }
}
