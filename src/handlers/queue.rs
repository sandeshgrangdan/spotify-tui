use super::{super::app::App, common_key_events};
use crate::event::Key;
use crate::network::IoEvent;

fn queue_len(app: &App) -> usize {
  app.queue.as_ref().map(|q| q.queue.len()).unwrap_or(0)
}

pub fn handler(key: Key, app: &mut App) {
  match key {
    k if common_key_events::left_event(k) => common_key_events::handle_left_event(app),
    k if common_key_events::down_event(k) => {
      let len = queue_len(app);
      if len > 0 && app.queue_selected_index + 1 < len {
        app.queue_selected_index += 1;
      }
    }
    k if common_key_events::up_event(k) => {
      if app.queue_selected_index > 0 {
        app.queue_selected_index -= 1;
      }
    }
    Key::Enter => {
      let len = queue_len(app);
      if app.queue_selected_index > 0 && app.queue_selected_index < len {
        let target = app.queue_selected_index;
        app.queue_selected_index = 0;
        app.dispatch(IoEvent::SkipToQueueIndex(target));
      }
      // selected == 0 (the head) is a no-op — it's already playing.
    }
    Key::Char('x') => {
      if queue_len(app) > 0 {
        app.dispatch(IoEvent::NextTrack);
        app.dispatch(IoEvent::GetQueue);
      }
    }
    _ => {}
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn down_at_end_does_not_advance() {
    let mut app = App::default();
    // No queue loaded → queue_len returns 0 → down should not panic and stay at 0.
    handler(Key::Char('j'), &mut app);
    assert_eq!(app.queue_selected_index, 0);
  }
}
