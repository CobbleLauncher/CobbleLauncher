// SPDX-License-Identifier: MIT
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod appearance;
mod cache;
mod catalog_install;
mod change_share;
mod compatibility;
mod curseforge;
mod games;
mod java;
mod launcher_updates;
mod logging;
mod market_images;
mod microsoft;
mod offline;
mod onboarding;
mod plugin_catalogs;
mod plugins;
mod preferences;
mod runtime_resources;
mod seed_map;
mod startup_metrics;
mod window_geometry;
use tauri::{Emitter, Manager};

#[tauri::command]
fn get_app_info() -> cobble_domain::AppInfo {
    cobble_core::app_info()
}

#[tauri::command]
fn get_active_task_count(app: tauri::AppHandle) -> usize {
    games::tasks::active_count(&app)
}

#[tauri::command]
fn cancel_active_tasks(app: tauri::AppHandle) {
    games::tasks::cancel_active(&app);
}

#[tauri::command]
fn get_download_settings() -> Result<cobble_minecraft::DownloadSettings, String> {
    cobble_minecraft::read_download_settings().map_err(|error| error.to_string())
}

#[tauri::command]
fn set_download_settings(settings: cobble_minecraft::DownloadSettings) -> Result<(), String> {
    cobble_minecraft::save_download_settings(&settings).map_err(|error| error.to_string())
}

fn main() {
    startup_metrics::mark_started();
    let _logging = logging::init();
    plugins::queue_file(std::env::args().skip(1));
    games::quick_launch::queue_arguments(std::env::args().skip(1));
    tauri::Builder::default()
        .register_uri_scheme_protocol("cobble-plugin", plugins::plugin_views::serve_asset)
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(launcher_updates::UpdateState::default())
        .plugin(tauri_plugin_single_instance::init(|app, arguments, _| {
            if games::quick_launch::queue_arguments(arguments.iter().skip(1).cloned()) {
                let _ = app.emit("launch-intent", ());
            }
            if plugins::queue_file(arguments.into_iter().skip(1)) {
                let _ = app.emit("schematic-open", ());
            }
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .manage(games::GameLibrary::load())
        .manage(games::tasks::TaskLibrary::load())
        .setup(|app| {
            window_geometry::install(app)?;
            let handle = app.handle().clone();
            tauri::async_runtime::spawn_blocking(move || {
                match games::tasks::reconcile_interrupted_pack_copies(&handle) {
                    Ok(count) if count > 0 => {
                        tracing::info!(count, "Cleaned interrupted pack copies")
                    }
                    Ok(_) => {}
                    Err(error) => tracing::warn!(%error, "Interrupted pack copy cleanup failed"),
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main"
                && let tauri::WindowEvent::DragDrop(tauri::DragDropEvent::Drop { paths, .. }) =
                    event
            {
                let app = window.app_handle().clone();
                let paths = paths.clone();
                tauri::async_runtime::spawn(async move {
                    let notice =
                        match plugins::plugin_file_inbox::capture_drop(app.clone(), paths).await {
                            Ok(true) => return,
                            Ok(false) => "unavailable",
                            Err(_) => "failed",
                        };
                    let _ = app.emit_to("main", "plugin-file-inbox-notice", notice);
                });
            }
            if matches!(event, tauri::WindowEvent::Focused(true)) {
                let _ = games::mesh::network::mesh_network_resume_if_needed(
                    window.app_handle().clone(),
                );
            }
            if matches!(event, tauri::WindowEvent::Destroyed) {
                plugins::pack_inspection_pages::revoke_window(window.label());
                plugins::blueprint_provider::revoke_window(window.label());
                games::screenshot_sheet::revoke_window(window.label());
                plugins::plugin_reports::folder_report::revoke_window(window.label());
                plugins::plugin_views::revoke_window(window.label());
            }
        })
        .invoke_handler(tauri::generate_handler![
            change_share::preview_game_change_export,
            change_share::export_game_change,
            change_share::import_game_change,
            change_share::preview_game_change_apply,
            change_share::apply_game_change,
            change_share::undo_game_change_new_config,
            games::game_transfer::game_transfer_context,
            games::game_transfer::game_transfer_prepare,
            games::game_transfer::game_transfer_send,
            games::game_transfer::game_transfer_cancel,
            games::game_transfer::game_transfer_offers,
            games::game_transfer::game_transfer_list_outgoing,
            games::game_transfer::game_transfer_status,
            games::game_transfer::game_transfer_accept,
            games::game_transfer::game_transfer_reject,
            games::game_transfer::game_transfer_discard,
            startup_metrics::report_home_ready,
            onboarding::get_onboarding_state,
            onboarding::save_onboarding_state,
            launcher_updates::launcher_update_status,
            launcher_updates::set_launcher_update_preferences,
            launcher_updates::check_launcher_update,
            launcher_updates::install_launcher_update,
            games::sessions::retry_game_session_recovery,
            games::mesh::network::mesh_network_snapshot,
            games::mesh::network::mesh_network_start,
            games::mesh::network::mesh_network_retry,
            games::mesh::network::mesh_network_resume_if_needed,
            games::mesh::network::mesh_network_set_enabled,
            get_app_info,
            games::quick_launch::peek_pending_launch,
            games::quick_launch::acknowledge_pending_launch,
            games::quick_launch::create_game_shortcut,
            games::game_servers::list_game_servers,
            games::game_servers::start_game_server,
            plugins::plugin_views::open_plugin_view,
            plugins::plugin_views::resize_plugin_view,
            plugins::plugin_views::close_plugin_view,
            plugins::plugin_views::set_plugin_view_visible,
            plugins::plugin_views::plugin_view_is_dirty,
            plugins::plugin_frontend_bridge::frontend_call,
            plugins::plugin_access_log::plugin_access_log,
            games::pack_update_preview::get_game_pack_update_context,
            games::pack_update_preview::preview_game_pack_update,
            games::pack_update_copy::start_game_pack_update_copy,
            games::mesh::app::mesh_remove_contact,
            games::mesh::app::mesh_identity_state,
            games::mesh::app::mesh_set_identity_mode,
            games::mesh::app::mesh_start_app,
            games::mesh::app::mesh_app_snapshot,
            games::mesh::app::mesh_stop_app,
            games::mesh::app::mesh_set_display_name,
            games::mesh::app::mesh_create_contact_invitation,
            games::mesh::app::mesh_accept_contact_invitation,
            games::mesh::app::mesh_send_message,
            games::mesh::app::mesh_list_messages,
            games::mesh::composer::mesh_list_message_composers,
            games::mesh::composer::mesh_cancel_message_composer,
            games::mesh::composer::mesh_compose_message,
            market_images::get_market_image,
            games::game_config::list_game_configs,
            games::game_config::read_game_config,
            games::game_config::save_game_config,
            games::game_config::restore_game_config_backup,
            games::screenshots::list_game_screenshots,
            games::screenshots::get_game_screenshot,
            games::screenshots::open_game_screenshot,
            games::screenshots::open_game_screenshots_folder,
            plugins::blueprint_provider::get_blueprint_provider_availability,
            plugins::blueprint_viewers::get_blueprint_viewer_choices,
            plugins::blueprint_provider::search_online_blueprints,
            plugins::blueprint_provider::online_blueprint_details,
            plugins::blueprint_provider::open_online_blueprint,
            plugins::blueprint_provider::download_online_blueprint,
            plugins::blueprint_provider::cancel_blueprint_provider,
            plugins::update_plugin,
            games::world_snapshots::retention::preview_world_snapshot_cleanup,
            games::world_snapshots::retention::apply_world_snapshot_cleanup,
            games::world_datapacks::list_world_datapacks,
            games::world_datapacks::import_world_datapacks,
            games::world_datapacks::move_world_datapack,
            games::curseforge_pack::preview_curseforge_pack,
            games::curseforge_pack::preview_curseforge_online_pack,
            games::curseforge_pack::prepare_curseforge_pack,
            games::curseforge_pack::select_curseforge_pack_manual_file,
            games::curseforge_pack::create_curseforge_pack_game,
            games::curseforge_pack::get_task_curseforge_pack_preview,
            curseforge::get_curseforge_availability,
            curseforge::search_curseforge,
            curseforge::list_curseforge_files,
            games::curseforge_install::preview_curseforge_mod_install,
            games::curseforge_install::select_curseforge_manual_file,
            games::curseforge_install::install_curseforge_mod_plan,
            games::curseforge_install::open_curseforge_project,
            games::curseforge_install::list_game_curseforge_mods,
            games::curseforge_install::check_game_curseforge_updates,
            games::game_appearance::set_game_appearance_preset,
            games::game_appearance::reset_game_icon,
            games::game_appearance::pick_game_icon,
            games::game_appearance::get_game_icons,
            games::game_appearance::set_game_pinned,
            games::game_appearance::pick_game_cover,
            games::game_appearance::reset_game_cover,
            games::game_appearance::get_game_covers,
            games::game_config::keybindings::preview_game_keybindings,
            games::game_config::keybindings::apply_game_keybindings,
            games::game_config::keybindings::restore_game_keybindings,
            preferences::set_default_game_directory,
            games::mesh::mesh_availability,
            games::mesh::overview::mesh_overview,
            games::mesh::overview::mesh_host_options,
            games::mesh::overview::mesh_discover,
            games::mesh::mesh_host,
            games::mesh::mesh_host_server,
            games::mesh::mesh_network_status,
            games::mesh::mesh_set_enabled,
            games::mesh::mesh_join,
            games::mesh::mesh_status,
            games::mesh::mesh_end,
            games::mesh::groups::commands::mesh_group_snapshot,
            games::mesh::groups::commands::mesh_group_create,
            games::mesh::groups::commands::mesh_group_join,
            games::mesh::groups::commands::mesh_group_invite,
            games::mesh::groups::commands::mesh_group_revoke,
            games::mesh::groups::commands::mesh_group_kick,
            games::mesh::groups::commands::mesh_group_stop,
            logging::open_launcher_logs,
            logging::export_launcher_diagnostics,
            plugins::extension_contributions::list_extension_contributions,
            plugins::extension_contributions::resolve_extension_command,
            plugins::extension_settings::get_extension_settings,
            plugins::extension_settings::interchange::export_extension_settings_yaml,
            plugins::extension_settings::interchange::preview_extension_settings_yaml,
            plugins::extension_settings::set_extension_setting,
            plugins::extension_settings::declarative::save_extension_settings_form,
            plugins::plugin_signatures::get_pending_plugin_signature,
            plugins::list_plugins,
            plugins::plugin_details::get_plugin_details,
            plugins::get_plugin_icon,
            plugins::bundled_plugins::list_bundled_plugins,
            plugins::bundled_plugins::preview_bundled_plugin,
            plugins::plugin_reports::analyze_plugin_report,
            plugins::plugin_reports::folder_report::analyze_plugin_folder_report,
            plugins::plugin_reports::managed_config_report::analyze_game_config_batch_report,
            plugins::plugin_reports::config_diagnosis::analyze_game_config_report,
            plugins::pack_inspection_pages::begin_pack_inspection,
            plugins::pack_inspection_pages::query_pack_inspection,
            plugins::pack_inspection_pages::detail_pack_inspection,
            plugins::pack_inspection_pages::close_pack_inspection,
            plugins::plugin_reports::analyze_game_report,
            games::world_snapshots::list_world_snapshots,
            games::world_snapshots::insights::get_world_snapshot_storage,
            games::world_snapshots::insights::compare_world_snapshots,
            games::screenshot_sheet::list_screenshot_sheet_plugins,
            games::screenshot_sheet::preview_screenshot_sheet,
            games::screenshot_sheet::save_screenshot_sheet,
            games::screenshot_sheet::cancel_screenshot_sheet,
            games::world_snapshots::create_world_snapshot,
            games::world_snapshots::restore_world_snapshot,
            games::world_snapshots::recovery::export_world_snapshot_recovery,
            games::world_snapshots::recovery::import_world_snapshot_recovery,
            plugins::plugin_reports::cancel_plugin_report,
            plugins::tool_forms::run_plugin_tool_form,
            plugins::tool_forms::cancel_plugin_tool_form,
            plugin_catalogs::list_plugin_catalogs,
            plugin_catalogs::add_plugin_catalog,
            plugin_catalogs::refresh_plugin_catalog,
            plugin_catalogs::set_plugin_catalog_enabled,
            plugin_catalogs::remove_plugin_catalog,
            plugins::take_pending_schematic,
            plugins::peek_pending_schematic,
            plugins::preview_plugin_package,
            plugins::cancel_plugin_preview,
            catalog_install::preview_catalog_plugin,
            plugins::install_plugin,
            plugins::set_plugin_enabled,
            plugins::uninstall_plugin,
            plugins::open_schematic,
            plugins::read_viewer_chunk,
            plugins::viewer_textures::load_viewer_textures,
            plugins::close_viewer,
            plugins::save_blueprint,
            plugins::list_blueprints,
            plugins::open_blueprint,
            plugins::blueprint_library::rename_blueprint,
            plugins::blueprint_library::set_blueprint_favorite,
            plugins::blueprint_library::save_blueprint_thumbnail,
            plugins::blueprint_library::read_blueprint_thumbnail,
            plugins::blueprint_library::remove_blueprint,
            plugins::blueprint_export::export_blueprint_materials,
            plugins::blueprint_material_handoffs::blueprint_material_receivers,
            plugins::blueprint_material_handoffs::send_blueprint_materials,
            plugins::plugin_file_inbox::file_inbox_preview_handoff,
            plugins::plugin_file_inbox::file_inbox_apply_handoff,
            games::blueprint_import::import_blueprint,
            games::blueprint_import::open_blueprint_destination,
            games::launch_recovery::get_launch_recovery,
            games::launch_recovery::open_current_launch_log,
            games::launch_recovery::repair_game_artifacts,
            games::world_snapshots::automatic::get_automatic_world_backup,
            games::world_snapshots::automatic::set_automatic_world_backup,
            appearance::get_profile_appearance,
            appearance::pick_profile_skin,
            appearance::apply_profile_skin,
            cache::get_public_cache_stats,
            cache::get_download_tuning,
            cache::set_download_tuning,
            cache::clear_public_cache,
            games::tasks::list_tasks,
            get_active_task_count,
            cancel_active_tasks,
            games::tasks::cancel_task,
            games::tasks::retry_task,
            games::tasks::get_task_pack_preview,
            microsoft::begin_microsoft_login,
            microsoft::poll_microsoft_login,
            microsoft::cancel_microsoft_login,
            microsoft::open_microsoft_login,
            preferences::get_launcher_preferences,
            preferences::set_default_memory,
            preferences::set_default_advanced_runtime,
            games::runtime::set_game_advanced_runtime,
            preferences::pick_default_game_directory,
            preferences::pick_default_game_directory_candidate,
            preferences::get_standard_game_directory,
            preferences::get_default_game_directory_candidate,
            games::directory_migration::preview_game_directory_migration,
            games::directory_migration::start_game_directory_migration,
            games::directory_migration::open_game_directory_migration_source,
            games::directory_migration::inspect_game_directory_migration,
            games::directory_migration::cleanup_game_directory_migration_source,
            preferences::reset_default_game_directory,
            games::runtime::set_game_memory,
            games::runtime::set_game_java_mode,
            games::runtime::list_game_java_runtimes,
            games::runtime::set_game_java_runtime,
            games::runtime::pick_game_java_executable,
            get_download_settings,
            set_download_settings,
            java::get_java_settings,
            java::list_java_runtimes,
            compatibility::query_launch_compatibility,
            java::select_java_runtime,
            java::pick_java_executable,
            java::pick_java_candidate,
            java::pick_game_java_candidate,
            java::use_automatic_java,
            games::list_game_versions,
            games::list_games,
            games::default_game_directory,
            games::pick_game_directory,
            games::create_game,
            games::reverse_config::search_reverse_mods,
            games::reverse_config::reverse_configure_mods,
            games::reverse_config::cancel_reverse_configuration,
            games::reverse_config::create_game_from_mods,
            games::native_reverse::search_environment_mods,
            games::native_reverse::reverse_configure_environment,
            games::native_reverse::cancel_environment_configuration,
            games::native_reverse::preview_environment_installation,
            games::native_reverse::select_environment_manual_file,
            games::native_reverse::create_game_from_environment,
            games::native_reverse_install::get_task_environment_requirements,
            games::native_reverse_install::select_task_environment_manual_file,
            games::native_readiness::inspect_game_readiness,
            games::native_readiness::preview_game_readiness,
            games::native_readiness::prepare_game_readiness,
            games::builtin_copy::start_game_copy,
            games::mesh::environment::mesh_environment_share_preview,
            games::mesh::environment::mesh_environment_share_publish,
            games::mesh::environment::mesh_environment_receive,
            games::mesh::environment::mesh_environment_prepare,
            games::mesh::environment::mesh_environment_manual_file,
            games::mesh::environment::mesh_environment_create,
            games::native_readiness::cancel_game_readiness_check,
            games::mod_updates::get_game_mod_update_policy,
            games::curseforge_install::preview_game_curseforge_updates,
            games::mod_updates::set_game_mod_update_policy,
            games::mod_updates::list_game_maintenance,
            games::mesh::overview::mesh_route_policy_save,
            games::mesh::node::mesh_node_start,
            games::mesh::node::mesh_node_status,
            games::mesh::node::mesh_node_stop,
            games::retry_game,
            games::runtime::prepare_game_launch,
            games::runtime::start_game,
            games::set_game_profile,
            games::list_fabric_versions,
            games::list_loader_versions,
            games::rename_game,
            games::management::copy_game,
            games::management::export_game,
            games::management::remove_game,
            games::open_game_directory,
            games::open_game_log,
            games::preview_mrpack,
            games::create_mrpack_game,
            games::search_game_mods,
            games::preview_game_mod_install,
            games::install_game_mod_plan,
            games::mod_updates::preview_game_mod_operation_undo,
            games::mod_updates::list_game_mod_history,
            games::mod_updates::undo_game_mod_operation,
            games::mod_updates::check_game_mod_updates,
            games::mod_updates::list_game_mod_versions,
            games::mod_updates::get_game_mod_version_details,
            games::mod_updates::list_managed_game_mods,
            games::mod_updates::set_game_mod_pinned,
            games::mod_updates::preview_game_mod_version_changes,
            games::mod_updates::recognize_game_mods,
            games::local_packs::list_game_packs,
            games::local_packs::search_game_content_packs,
            games::local_packs::list_game_content_pack_versions,
            games::local_packs::install_game_content_pack,
            games::local_packs::import_game_packs,
            games::local_packs::remove_game_pack,
            games::local_packs::open_game_pack_directory,
            games::marketplace::search_modrinth_packs,
            games::marketplace::list_modrinth_pack_versions,
            games::marketplace::preview_modrinth_pack,
            games::worlds::list_game_worlds,
            games::worlds::metadata::read_local_world_metadata,
            games::worlds::open_game_world_directory,
            games::worlds::export_game_world,
            games::worlds::preview_world_backup,
            games::worlds::restore_game_world,
            games::worlds::list_game_world_backups,
            games::worlds::rollback_game_world_restore,
            games::launcher_import::preview_launcher_import,
            games::launcher_import::discover_launcher_import,
            games::launcher_import::get_launcher_import_result,
            games::launcher_import::create_launcher_import,
            games::mod_updates::preview_game_mod_updates,
            games::mod_updates::apply_game_mod_updates,
            games::list_game_mods,
            games::import_game_mods,
            games::set_game_mod_enabled,
            games::remove_game_mod,
            offline::list_player_profiles,
            offline::add_offline_profile,
            offline::select_player_profile,
            offline::update_profile_note,
            offline::remove_player_profile,
            offline::get_offline_profile,
            offline::save_offline_profile,
            offline::remove_offline_profile
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Cobble Launcher")
        .run(|_app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                window_geometry::flush(_app);
                plugins::plugin_file_inbox::shutdown();
                games::mesh::shutdown();
            }
        });
}
