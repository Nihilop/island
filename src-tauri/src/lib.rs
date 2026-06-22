// Island — hôte agnostique. `lib.rs` ne fait que le câblage : déclaration des
// modules, setup, et enregistrement des commandes exposées au front/SDK.
//
//   overlay   : overlay plein écran (click-through, capture-exclude, focus)
//   launcher  : actions système intégrées
//   host      : fenêtres, shell, tray, détection plein écran
//   ext       : système d'extensions (permissions, storage, install, scaffold)
//   services  : services exposés aux extensions (capture, apps, media, net, system)
//
// Chaque service spécifique à l'OS sépare la couche commande/permission (cross-platform)
// de l'implémentation native (`windows.rs`, gated `#[cfg(target_os = "windows")]`).

use tauri::Manager;

mod ext;
mod host;
mod launcher;
mod overlay;
mod services;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // DOIT être le 1er plugin : si l'app tourne déjà, un double-clic sur un
        // .island transmet l'argv à l'instance vivante au lieu d'en lancer une 2ᵉ.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            ext::install::handle_island_argv(app, &argv, true);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            if let Some(overlay) = app.get_webview_window("overlay") {
                let _ = overlay::cover_monitor(&overlay);
                #[cfg(target_os = "windows")]
                overlay::start_click_through(overlay.clone());
            }

            // Contrôleur média natif (SMTC) — une des API exposées aux extensions.
            services::media::start(app.handle().clone());

            // Live-reload : recharge une extension quand son `dist/` est rebuildé.
            ext::install::start_dist_watcher(app.handle().clone());

            // Auto-hide : détecte une app en plein écran → l'île se rétracte.
            host::start_fullscreen_monitor(app.handle().clone());

            // Zone de notification : ouvrir Réglages / quitter Island.
            host::build_tray(app.handle())?;

            // Double-clic au démarrage : un .island dans l'argv → modal d'install.
            let argv: Vec<String> = std::env::args().collect();
            ext::install::handle_island_argv(app.handle(), &argv, false);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            overlay::set_hit_regions,
            overlay::overlay_focus,
            launcher::list_launcher,
            host::open_settings,
            host::reveal_path,
            host::open_url,
            services::system::system_stats,
            services::system::system_battery,
            services::system::system_online,
            services::system::system_idle_ms,
            services::system::system_volume,
            services::system::system_set_volume,
            services::system::system_set_muted,
            services::net::http_fetch,
            services::apps::list_apps,
            services::apps::launch_path,
            services::apps::app_icons,
            services::capture::capture_list_displays,
            services::capture::capture_screenshot,
            services::capture::capture_start_recording,
            services::capture::capture_stop_recording,
            services::capture::capture_is_recording,
            services::capture::ext_fetch_binary,
            services::media::media_toggle,
            services::media::media_next,
            services::media::media_prev,
            services::media::media_seek,
            services::media::media_get_volume,
            services::media::media_set_volume,
            services::clipboard::clipboard_read_text,
            services::clipboard::clipboard_write_text,
            services::clipboard::clipboard_read_image,
            services::clipboard::clipboard_write_image,
            services::secrets::secret_get,
            services::secrets::secret_set,
            services::secrets::secret_delete,
            services::tts::tts_speak,
            services::input::input_type_text,
            services::windows::window_foreground,
            services::windows::window_list,
            services::windows::window_focus,
            services::pty::pty_spawn,
            services::pty::pty_write,
            services::pty::pty_resize,
            services::pty::pty_kill,
            services::pty::pty_exec,
            ext::install::register_file_association,
            ext::install::take_pending_install,
            ext::install::list_installed,
            ext::install::read_ext_file,
            ext::install::pack_extension,
            ext::install::open_install,
            ext::install::install_island,
            ext::scaffold::scaffold_extension,
            ext::scaffold::open_create,
            ext::storage::storage_get,
            ext::storage::storage_set,
            ext::storage::storage_delete,
            ext::storage::storage_keys
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
