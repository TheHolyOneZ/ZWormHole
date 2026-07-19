use tauri::Manager;

mod blocked;
mod commands;
mod discovery;
mod error;
mod events;
mod history;
mod identity;
mod net;
mod pairing;
mod profile;
mod settings;
mod state;
mod bench_history;
mod portmap;
mod transfer;
mod transport;
mod tray;
mod trusted;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(state::AppState::default())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let store_path = trusted::store::default_store_path(&app_data_dir);
            app.manage(trusted::store::TrustedStore::new(store_path));
            let blocked_path = blocked::default_store_path(&app_data_dir);
            app.manage(blocked::BlockStore::new(blocked_path));
            app.manage(profile::MyProfileStore::new(&app_data_dir));
            app.manage(profile::RemoteProfileStore::new(profile::remote_profiles_dir(
                &app_data_dir,
            )));
            let settings_path = settings::store::default_settings_path(&app_data_dir);
            app.manage(settings::store::SettingsStore::new(settings_path));
            let history_path = history::store::default_store_path(&app_data_dir);
            app.manage(history::store::HistoryStore::new(history_path));
            let queue_store_path = transfer::queue_store::default_store_path(&app_data_dir);
            app.manage(transfer::queue_store::QueueStore::new(queue_store_path));
            let bench_history_path = bench_history::default_store_path(&app_data_dir);
            app.manage(bench_history::BenchHistoryStore::new(bench_history_path));
            let device_identity = identity::load_or_create(&app_data_dir)?;
            app.manage(device_identity);
            tray::build_tray(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let state = window.state::<state::AppState>();
                let background_enabled = state
                    .background_listener_enabled
                    .load(std::sync::atomic::Ordering::Relaxed);
                if background_enabled {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::discovery::become_visible,
            commands::discovery::stop_visible,
            commands::discovery::start_browsing,
            commands::discovery::stop_browsing,
            commands::discovery::list_nearby_devices,
            commands::discovery::get_local_device_name,
            commands::discovery::get_local_connection_info,
            commands::pairing::list_trusted_devices,
            commands::pairing::revoke_trusted_device,
            commands::pairing::block_device,
            commands::pairing::unblock_device,
            commands::pairing::list_blocked_devices,
            commands::pairing::send_connect_request,
            commands::pairing::send_manual_connect_request,
            commands::pairing::confirm_pairing_code,
            commands::pairing::list_active_peers,
            commands::transfer::send_files,
            commands::transfer::send_dropped,
            commands::transfer::send_folder,
            commands::transfer::send_files_multi,
            commands::transfer::send_folder_multi,
            commands::transfer::read_folder_tree,
            commands::transfer::send_text,
            commands::transfer::send_clipboard_image,
            commands::pairing::set_device_nickname,
            commands::pairing::set_device_favorite,
            commands::pairing::set_device_auto_accept,
            commands::transfer::list_transfers,
            commands::transfer::list_resumable_transfers,
            commands::transfer::resume_device_transfers,
            commands::transfer::dismiss_resumable,
            commands::transfer::list_transfer_history,
            commands::transfer::clear_transfer_history,
            commands::transfer::cancel_transfer,
            commands::transfer::pause_transfer,
            commands::transfer::resume_transfer,
            commands::transfer::respond_to_incoming_transfer,
            commands::transfer::run_speed_test,
            commands::transfer::run_benchmark,
            commands::transfer::export_benchmark,
            commands::transfer::record_benchmark,
            commands::transfer::list_benchmark_history,
            commands::transfer::clear_benchmark_history,
            commands::transfer::respond_to_connection_test,
            commands::system::open_external_url,
            commands::system::open_path,
            commands::system::reveal_path,
            commands::system::save_text_file,
            commands::settings::get_background_listener_enabled,
            commands::settings::set_background_listener_enabled,
            commands::settings::get_transfer_settings,
            commands::settings::set_auto_accept,
            commands::settings::set_save_dir,
            commands::settings::set_auto_extract,
            commands::settings::set_max_send_mb_s,
            commands::settings::set_max_concurrent_sends,
            commands::settings::set_quic_port,
            commands::profile::get_my_profile,
            commands::profile::set_my_display_name,
            commands::profile::set_my_avatar,
            commands::profile::clear_my_avatar,
            commands::profile::list_profiles,
            commands::profile::refresh_advertisement,
            commands::wan::enable_internet_reachable,
            commands::wan::disable_internet_reachable,
            commands::wan::get_wan_status,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app_handle, event| {


            if let tauri::RunEvent::Exit = event {
                let app = app_handle.clone();
                tauri::async_runtime::block_on(async move {
                    commands::wan::release_on_exit(&app).await;
                });
            }
        });
}
