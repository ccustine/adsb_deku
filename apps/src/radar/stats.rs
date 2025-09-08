use std::collections::VecDeque;
use std::time::{Duration, SystemTime};

use adsb_deku::ICAO;
use ratatui::layout::{Constraint, Rect};
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Row, Table};
use rsadsb_common::{Added, AirplaneCoor, Airplanes};
use tracing::info;

use crate::{Settings, DEFAULT_PRECISION};

#[derive(Debug)]
pub struct Stats {
    most_distance: Option<(SystemTime, ICAO, AirplaneCoor)>,
    most_airplanes: Option<(SystemTime, u32)>,
    total_airplanes: u32,
    message_timestamps: VecDeque<SystemTime>,
    messages_per_second: f64,
    last_rate_update: SystemTime,
}

impl Default for Stats {
    fn default() -> Self {
        Self {
            most_distance: None,
            most_airplanes: None,
            total_airplanes: 0,
            message_timestamps: VecDeque::new(),
            messages_per_second: 0.0,
            last_rate_update: SystemTime::now(),
        }
    }
}

impl Stats {
    pub fn track_message(&mut self) {
        let now = SystemTime::now();
        self.message_timestamps.push_back(now);

        // Remove timestamps older than 60 seconds to keep a rolling window
        let sixty_seconds_ago = now - Duration::from_secs(60);
        while let Some(&front_time) = self.message_timestamps.front() {
            if front_time < sixty_seconds_ago {
                self.message_timestamps.pop_front();
            } else {
                break;
            }
        }
    }

    pub fn update_message_rate(&mut self) {
        let now = SystemTime::now();

        // Update rate every 500ms or if this is the first time
        if self.last_rate_update.elapsed().unwrap_or(Duration::from_secs(1))
            >= Duration::from_millis(500)
        {
            // Count messages in the last second
            let one_second_ago = now - Duration::from_secs(1);
            let recent_messages = self
                .message_timestamps
                .iter()
                .filter(|&&timestamp| timestamp >= one_second_ago)
                .count();

            self.messages_per_second = recent_messages as f64;
            self.last_rate_update = now;
        }
    }

    pub fn update(&mut self, airplanes: &Airplanes, airplane_added: Added) {
        // Update most_distance
        let current_distance = self.most_distance.map_or(0.0, |most_distance| {
            most_distance.2.kilo_distance.map_or(0.0, |kilo_distance| kilo_distance)
        });
        for (key, state) in airplanes.iter() {
            if let Some(distance) = state.coords.kilo_distance {
                if distance > current_distance {
                    info!("new max distance: [{}]{:?}", key, state.coords);
                    self.most_distance = Some((SystemTime::now(), *key, state.coords));
                }
            }
        }

        // Update most airplanes
        let current_len = airplanes.len();
        let most_airplanes = self.most_airplanes.map_or(0, |most_airplanes| most_airplanes.1);

        if most_airplanes < current_len as u32 {
            info!("new most airplanes: {}", current_len);
            self.most_airplanes = Some((SystemTime::now(), current_len as u32));
        }

        // Update total airplanes
        if airplane_added == Added::Yes {
            self.total_airplanes += 1;
        }
    }
}

/// Render Help tab for tui display
pub fn build_tab_stats(
    f: &mut ratatui::Frame,
    chunks: &[Rect],
    stats: &Stats,
    settings: &Settings,
) {
    let format = time::format_description::parse("[month]/[day] [hour]:[minute]:[second]").unwrap();
    let mut rows: Vec<Row> = vec![];
    // Most distance
    let (time, value) = if let Some((time, key, value)) = stats.most_distance {
        let position = value.position.unwrap();
        let lat = format!("{:.DEFAULT_PRECISION$}", position.latitude);
        let lon = format!("{:.DEFAULT_PRECISION$}", position.longitude);
        let distance = format!("{:.DEFAULT_PRECISION$}", value.kilo_distance.unwrap());

        // display time
        let datetime = time::OffsetDateTime::from(time);
        (
            datetime.to_offset(settings.utc_offset).format(&format).unwrap(),
            format!("[{key}]: {distance}km {lat},{lon}"),
        )
    } else {
        ("None".to_string(), "".to_string())
    };
    rows.push(Row::new(vec!["Max Distance", &time, &value]));

    // Most airplanes tracked at one time
    let (time, value) = if let Some((time, most_airplanes)) = stats.most_airplanes {
        // display time
        let datetime = time::OffsetDateTime::from(time);
        (
            datetime.to_offset(settings.utc_offset).format(&format).unwrap(),
            most_airplanes.to_string(),
        )
    } else {
        ("None".to_string(), "".to_string())
    };
    rows.push(Row::new(vec!["Most Airplanes", &time, &value]));

    // Total Airplanes Tracked
    let total_airplanes_s = stats.total_airplanes.to_string();
    rows.push(Row::new(vec!["Total Airplanes", "All Time", &total_airplanes_s]));

    // Messages per second
    let messages_per_sec_s = format!("{:.1}", stats.messages_per_second);
    rows.push(Row::new(vec!["Messages/Sec", "Live", &messages_per_sec_s]));

    // draw table
    let widths = &[Constraint::Length(16), Constraint::Length(15), Constraint::Length(200)];
    let table = Table::new(rows, widths)
        .style(Style::default().fg(Color::White))
        .header(Row::new(vec!["Type", "DateTime", "Value"]).bottom_margin(1))
        .block(Block::bordered().title("Stats"))
        .column_spacing(1);
    f.render_widget(table, chunks[1]);
}
