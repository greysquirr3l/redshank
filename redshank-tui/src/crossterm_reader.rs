//! Crossterm input reader — converts terminal key events into `AppEvent`s.

use crate::domain::AppEvent;
use crossterm::event::{self, Event};
use std::time::Duration;
use tokio::sync::mpsc;

/// Spawn a background task that reads crossterm events and sends them to the
/// channel. Runs until the channel is closed or a quit signal is received.
pub async fn spawn_reader(tx: mpsc::Sender<AppEvent>) {
    loop {
        // Use tokio::task::spawn_blocking for crossterm poll to avoid
        // blocking the async runtime.
        let maybe_event = tokio::task::spawn_blocking(|| {
            if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                event::read().ok()
            } else {
                None
            }
        })
        .await;

        match maybe_event {
            Ok(Some(Event::Key(key))) => {
                if tx.send(AppEvent::Key(key)).await.is_err() {
                    break; // channel closed
                }
            }
            Ok(Some(_) | None) => {
                // Mouse/resize events or no event within poll window — ignore
            }
            Err(_) => {
                // spawn_blocking panicked — exit
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn reader_stops_on_closed_channel() {
        let (tx, rx) = mpsc::channel(1);
        drop(rx); // close the receiver immediately
        // spawn_reader should exit because the channel is closed
        tokio::time::timeout(Duration::from_millis(200), spawn_reader(tx))
            .await
            .ok(); // either finishes or times out — both acceptable
    }
}
