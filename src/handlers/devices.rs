use super::{super::app::App, common_key_events};
use crate::event::Key;
use crate::network::IoEvent;

pub fn handler(key: Key, app: &mut App) {
  match key {
    k if common_key_events::right_event(k) => common_key_events::handle_right_event(app),
    k if common_key_events::down_event(k) => {
      if let Some(payload) = &app.devices {
        let next = common_key_events::on_down_press_handler(
          &payload.devices,
          app.selected_device_index.or(Some(0)),
        );
        app.selected_device_index = Some(next);
      }
    }
    k if common_key_events::up_event(k) => {
      if let Some(payload) = &app.devices {
        let next = common_key_events::on_up_press_handler(
          &payload.devices,
          app.selected_device_index.or(Some(0)),
        );
        app.selected_device_index = Some(next);
      }
    }
    k if common_key_events::high_event(k) => {
      app.selected_device_index = Some(common_key_events::on_high_press_handler());
    }
    k if common_key_events::middle_event(k) => {
      if let Some(payload) = &app.devices {
        app.selected_device_index =
          Some(common_key_events::on_middle_press_handler(&payload.devices));
      }
    }
    k if common_key_events::low_event(k) => {
      if let Some(payload) = &app.devices {
        app.selected_device_index =
          Some(common_key_events::on_low_press_handler(&payload.devices));
      }
    }
    Key::Enter => {
      if let (Some(payload), Some(idx)) = (app.devices.as_ref(), app.selected_device_index) {
        if let Some(device) = payload.devices.get(idx) {
          if let Some(device_id) = device.id.clone() {
            app.dispatch(IoEvent::TransferPlaybackToDevice(device_id));
          }
        }
      }
    }
    _ => {}
  }
}
