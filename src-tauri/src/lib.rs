// Island — hôte agnostique. `lib.rs` ne fait que le câblage : déclaration des
// modules, setup, et enregistrement des commandes exposées au front/SDK.
//
//   overlay   : fenêtre fixe du notch (placement, click-through, capture-exclude, focus)
//   launcher  : launcher natif (index apps / réglages / fichiers, recherche, ouverture)
//   vault     : coffre (pont Bitwarden)
//   host      : fenêtres, shell, tray, détection plein écran
//   ext       : système d'extensions (permissions, storage, install, scaffold)
//   services  : services exposés aux extensions (capture, apps, media, net, system)
//
// Chaque service spécifique à l'OS sépare la couche commande/permission (cross-platform)
// de l'implémentation native (`windows.rs`, gated `#[cfg(target_os = "windows")]`).

use tauri::Manager;

mod detached;
mod backup;
mod extensions;
mod host;
mod launcher;
mod overlay;
mod services;
mod store;
mod vault;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Relance élevée pour débloquer les FPS : ajoute l'utilisateur au groupe puis quitte,
    // AVANT l'instance unique (sinon elle transmettrait l'argv à l'instance vivante).
    #[cfg(target_os = "windows")]
    {
        let args: Vec<String> = std::env::args().collect();
        if let Some(i) = args.iter().position(|a| a == services::fps::ELEVATE_ARG) {
            std::process::exit(services::fps::elevated_main(args.get(i + 1).map(String::as_str).unwrap_or("")));
        }
    }

    tauri::Builder::default()
        // DOIT être le 1er plugin : si l'app tourne déjà, un double-clic sur un
        // .island transmet l'argv à l'instance vivante au lieu d'en lancer une 2ᵉ.
        .plugin(tauri_plugin_single_instance::init(|app, argv, _cwd| {
            extensions::install::handle_argv(app, &argv);
        }))
        .register_uri_scheme_protocol(extensions::protocol::SCHEME, extensions::protocol::handle)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    use tauri::Emitter;
                    use tauri_plugin_global_shortcut::ShortcutState;
                    if event.state == ShortcutState::Pressed && host::is_open_shortcut(shortcut) {
                        let _ = app.emit("notch://toggle", ());
                    } else if event.state == ShortcutState::Pressed && launcher::hotkey::is_launcher_shortcut(shortcut) {
                        let _ = app.emit("launcher://toggle", ());
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            // Ordre : placer → afficher → click-through (doit suivre le `show`).
            if let Some(overlay) = app.get_webview_window("overlay") {
                let _ = overlay::place(&overlay);
                overlay::keep_on_top_edge(&overlay);
                let _ = overlay.show();
                #[cfg(target_os = "windows")]
                overlay::start_click_through(overlay.clone());
                #[cfg(target_os = "windows")]
                overlay::install_drop_bridge(&overlay);
            }
            // Marge réservée (AppBar) : rendue au shell à la fermeture.
            overlay::install_reserve_cleanup(app.handle());

            // Contrôleur média natif (SMTC) — une des API exposées aux extensions.
            services::media::start(app.handle().clone());
            // Égaliseur : thread parké tant qu'aucun égaliseur n'est visible.
            services::eq::start(app.handle().clone());
            // Historique du presse-papiers (en mémoire).
            services::cliphist::start(app.handle().clone());
            // Coffre Bitwarden : verrouillage automatique (glissant).
            vault::start(app.handle().clone());
            services::notifs::start(app.handle().clone());
            launcher::start(app.handle().clone());
            // Activité (jeux & apps) : hook de premier plan + relevé des process toutes les 2 s.
            services::activity::start(app.handle().clone());

            // v2 : loader d'extensions gelé (cf. docs/internal/v2-migration.md) → pas de
            // watcher `dist/` tant qu'aucune extension ne se charge.

            // Auto-hide : détecte une app en plein écran → l'île se rétracte.
            host::start_fullscreen_monitor(app.handle().clone());

            // Garde-fou mémoire : Island surveille sa conso et se récupère si ça dérape.
            host::start_memory_watchdog(app.handle().clone());

            // Raccourci global d'ouverture : enregistré par le front (réglable) via
            // `notch_set_shortcut`.

            // Zone de notification : ouvrir, réglages, mode focus, autostart, quitter.
            host::build_tray(app.handle())?;

            // Double-clic au démarrage : un .island dans l'argv → modal d'install.
            let argv: Vec<String> = std::env::args().collect();
            extensions::install::handle_argv(app.handle(), &argv);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            overlay::set_hit_regions,
            overlay::overlay_focus,
            overlay::notch_set_pinned,
            overlay::notch_set_passthrough,
            overlay::notch_set_hide_in_fullscreen,
            overlay::notch_set_reserve,
            overlay::notch_monitors,
            overlay::notch_get_display,
            overlay::notch_set_display,
            overlay::notch_focus,
            overlay::notch_memory_low,
            host::tray_sync,
            host::notch_set_shortcut,
            host::reveal_path,
            host::open_url,
            services::media::np_state,
            services::activity::activity_state,
            services::activity::activity_running,
            services::activity::activity_set_override,
            services::activity::activity_overrides,
            services::activity::activity_stats,
            services::activity::activity_settings,
            services::activity::activity_settings_set,
            services::activity::activity_refresh_catalog,
            services::activity::activity_ext_state,
            services::activity::activity_ext_stats,
            services::media::np_toggle,
            services::media::np_next,
            services::media::np_prev,
            services::media::np_seek,
            services::media::np_open_source,
            services::mixer::mixer_list,
            services::mixer::mixer_set,
            services::eq::eq_subscribe,
            services::system_ctl::audio_outputs,
            services::system_ctl::audio_set_output,
            services::system_ctl::app_close,
            services::system_ctl::app_launch,
            services::cliphist::clip_list,
            services::cliphist::clip_set_mode,
            services::cliphist::clip_copy,
            services::cliphist::clip_remove,
            services::cliphist::clip_clear,
            services::cliphist::clip_preview,
            services::files::files_probe,
            services::hardware::hw_sample,
            services::hardware::hw_ping_state,
            services::privacy::privacy_state,
            services::discord::discord_configure,
            services::discord::discord_state,
            services::discord::discord_set_secret,
            services::discord::discord_has_secret,
            services::discord::discord_action,
            services::fps::imp::fps_sample,
            services::ai::ai_chat,
            services::ai::ai_cancel,
            services::ai::ai_set_key,
            services::ai::ai_has_key,
            services::usage::usage_get,
            services::convert::convert_files,
            services::convert::convert_ffmpeg_available,
            services::fps::imp::fps_unlock,
            services::files::file_open,
            services::files::files_copy,
            services::files::export_csv,
            detached::window_open,
            backup::backup_current,
            backup::backup_export,
            backup::backup_inspect,
            backup::backup_import,
            backup::backup_open_folder,
            detached::window_close_kind,
            detached::window_args,
            detached::window_fullscreen,
            services::notes::notes_list,
            services::notes::notes_read,
            services::notes::notes_meta,
            services::notes::notes_write,
            services::notes::notes_create,
            services::notes::notes_move,
            services::notes::notes_pin,
            services::notes::notes_trash,
            services::notes::notes_purge,
            services::notes::notes_folder_create,
            services::notes::notes_folder_move,
            services::notes::notes_folder_delete,
            services::notes::notes_search,
            services::files::files_from_clipboard,
            services::weather::weather_forecast,
            services::weather::weather_geocode,
            services::weather::weather_locate,
            services::markets::markets_stocks,
            services::markets::markets_stock_search,
            services::markets::markets_crypto,
            services::markets::markets_crypto_search,
            services::desktop::master_volume,
            services::capture::capture_displays,
            services::capture::capture_select,
            services::capture::capture_select_init,
            services::capture::capture_select_done,
            services::capture::capture_screenshot,
            services::capture::capture_record_start,
            services::capture::capture_record_stop,
            services::capture::capture_record_state,
            vault::vault_status,
            vault::vault_install,
            vault::vault_set_server,
            vault::vault_login,
            vault::vault_login_code,
            vault::vault_login_cancel,
            vault::vault_unlock,
            vault::vault_lock,
            vault::vault_logout,
            vault::vault_sync,
            vault::vault_set_timeout,
            vault::vault_hello_set,
            vault::vault_hello_unlock,
            services::hello::hello_status,
            services::hello::hello_enroll,
            services::hello::hello_disable,
            services::hello::hello_verify,
            services::notifs::notifs_status,
            services::notifs::notifs_request_access,
            services::notifs::notifs_set_enabled,
            services::notifs::notifs_list,
            services::notifs::notifs_dismiss,
            services::notifs::notifs_clear,
            services::notifs::notifs_open,
            services::notifs::notifs_open_settings,
            launcher::launcher_search,
            extensions::ext2_list,
            extensions::ext2_storage_get,
            extensions::ext2_storage_set,
            extensions::ext2_log,
            extensions::install::ext2_set_dev,
            extensions::install::ext2_dev_inspect,
            extensions::install::ext2_inspect,
            extensions::install::ext2_install,
            extensions::install::ext2_uninstall,
            extensions::install::ext2_take_pending,
            extensions::services::ext2_secret_get,
            extensions::services::ext2_secret_set,
            extensions::services::ext2_secret_delete,
            extensions::services::ext2_read_text,
            extensions::services::ext2_clipboard_read,
            extensions::services::ext2_clipboard_write,
            extensions::services::ext2_http,
            extensions::services::ext2_http_bytes,
            extensions::services::ext2_cookies_clear,
            launcher::launcher_open,
            launcher::launcher_open_url,
            launcher::launcher_icons,
            launcher::launcher_opened,
            launcher::launcher_set_lang,
            launcher::hotkey::launcher_config,
            launcher::hotkey::launcher_capture,
            launcher::hotkey::launcher_paste_text,
            launcher::processes::launcher_processes,
            vault::vault_list,
            vault::vault_codes,
            vault::vault_copy,
            vault::vault_reveal,
            vault::vault_add,
            vault::vault_edit,
            vault::vault_delete,
            vault::vault_scan_qr,
            vault::vault_clear_scan,
            vault::vault_add_totp,
            store::storage_get,
            store::storage_set,
            store::storage_keys
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
