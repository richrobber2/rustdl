#![cfg(target_os = "android")]

#[path = "main.rs"]
#[allow(dead_code)]
mod app;

use jni::objects::{GlobalRef, JObject, JString, JValue};
use jni::sys::{jboolean, jint, jlong, jstring};
use jni::{JNIEnv, JavaVM};
use std::path::{Path, PathBuf};
use std::sync::{Once, OnceLock};

struct AndroidBridge {
    vm: JavaVM,
    activity: GlobalRef,
}

static ANDROID_BRIDGE: OnceLock<AndroidBridge> = OnceLock::new();
static SERVER_STARTED: Once = Once::new();

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_MainActivity_nativeSetRuntimeTuning(
    _env: JNIEnv,
    _activity: JObject,
    unmetered: jboolean,
    charging: jboolean,
    power_save: jboolean,
    thermal_status: jint,
    free_bytes: jlong,
    processors: jint,
) {
    app::set_runtime_tuning(
        unmetered != 0,
        charging != 0,
        power_save != 0,
        thermal_status,
        free_bytes.max(0) as u64,
        processors.max(1) as usize,
    );
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_MainActivity_nativeSetPeerPairing<'local>(
    mut env: JNIEnv<'local>,
    _activity: JObject<'local>,
    address: JString<'local>,
    key: JString<'local>,
) -> jboolean {
    let result = (|| -> Result<(), String> {
        let address: String = env
            .get_string(&address)
            .map_err(|error| error.to_string())?
            .into();
        let key: String = env
            .get_string(&key)
            .map_err(|error| error.to_string())?
            .into();
        app::set_outbound_peer_pairing(&address, &key)
    })();
    if result.is_ok() { 1 } else { 0 }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_MainActivity_nativeUpdateManifestUrl<'local>(
    env: JNIEnv<'local>,
    _activity: JObject<'local>,
) -> jstring {
    match env.new_string(option_env!("RUSTDL_UPDATE_MANIFEST_URL").unwrap_or("")) {
        Ok(value) => value.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_app_rustdl_MainActivity_nativeStartServer<'local>(
    mut env: JNIEnv<'local>,
    activity: JObject<'local>,
    bind: JString<'local>,
    output_dir: JString<'local>,
    inspection_mode: jboolean,
) {
    let result = (|| -> Result<(), String> {
        let bind: String = env
            .get_string(&bind)
            .map_err(|error| error.to_string())?
            .into();
        let output_dir: String = env
            .get_string(&output_dir)
            .map_err(|error| error.to_string())?
            .into();
        let bridge = AndroidBridge {
            vm: env.get_java_vm().map_err(|error| error.to_string())?,
            activity: env
                .new_global_ref(activity)
                .map_err(|error| error.to_string())?,
        };
        let _ = ANDROID_BRIDGE.set(bridge);
        app::set_publish_hook(publish_to_android_downloads);
        app::set_transfer_hook(update_android_transfer);
        if inspection_mode == 0 {
            app::set_event_hook(dispatch_android_event);
        }
        app::set_mux_hook(mux_android_tracks);
        app::set_extract_audio_hook(extract_android_audio);
        app::set_thumbnail_hook(generate_android_thumbnail);
        if inspection_mode == 0 {
            app::set_watched_hook(watched_from_android);
            app::set_delete_hook(delete_from_android_downloads);
        }
        app::set_inspection_mode(inspection_mode != 0);
        SERVER_STARTED.call_once(move || {
            std::thread::spawn(move || {
                app::run_embedded_server(bind, PathBuf::from(output_dir));
            });
        });
        Ok(())
    })();

    if let Err(error) = result {
        let _ = env.throw_new("java/lang/IllegalStateException", error);
    }
}

fn dispatch_android_event(event: &str) -> Result<(), String> {
    let bridge = ANDROID_BRIDGE
        .get()
        .ok_or_else(|| "Android bridge is not initialized".to_owned())?;
    let mut env = bridge
        .vm
        .attach_current_thread()
        .map_err(|error| error.to_string())?;
    let event = env.new_string(event).map_err(|error| error.to_string())?;
    let event_object = JObject::from(event);
    env.call_method(
        bridge.activity.as_obj(),
        "dispatchRustEvent",
        "(Ljava/lang/String;)V",
        &[JValue::Object(&event_object)],
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn generate_android_thumbnail(source: &Path, filename: &str) -> Result<bool, String> {
    let bridge = ANDROID_BRIDGE
        .get()
        .ok_or_else(|| "Android bridge is not initialized".to_owned())?;
    let mut env = bridge
        .vm
        .attach_current_thread()
        .map_err(|error| error.to_string())?;
    let source = env
        .new_string(source.to_string_lossy().as_ref())
        .map_err(|error| error.to_string())?;
    let filename = env
        .new_string(filename)
        .map_err(|error| error.to_string())?;
    let source_object = JObject::from(source);
    let filename_object = JObject::from(filename);
    env.call_method(
        bridge.activity.as_obj(),
        "ensureThumbnail",
        "(Ljava/lang/String;Ljava/lang/String;)Z",
        &[
            JValue::Object(&source_object),
            JValue::Object(&filename_object),
        ],
    )
    .and_then(|value| value.z())
    .map_err(|error| error.to_string())
}

fn mux_android_tracks(video: &Path, audio: &Path, output: &Path) -> Result<(), String> {
    let bridge = ANDROID_BRIDGE
        .get()
        .ok_or_else(|| "Android bridge is not initialized".to_owned())?;
    let mut env = bridge
        .vm
        .attach_current_thread()
        .map_err(|error| error.to_string())?;
    let video = env
        .new_string(video.to_string_lossy().as_ref())
        .map_err(|error| error.to_string())?;
    let audio = env
        .new_string(audio.to_string_lossy().as_ref())
        .map_err(|error| error.to_string())?;
    let output = env
        .new_string(output.to_string_lossy().as_ref())
        .map_err(|error| error.to_string())?;
    let video_object = JObject::from(video);
    let audio_object = JObject::from(audio);
    let output_object = JObject::from(output);
    env.call_method(
        bridge.activity.as_obj(),
        "muxDownloads",
        "(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V",
        &[
            JValue::Object(&video_object),
            JValue::Object(&audio_object),
            JValue::Object(&output_object),
        ],
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn extract_android_audio(source: &Path, output: &Path) -> Result<(), String> {
    let bridge = ANDROID_BRIDGE
        .get()
        .ok_or_else(|| "Android bridge is not initialized".to_owned())?;
    let mut env = bridge
        .vm
        .attach_current_thread()
        .map_err(|error| error.to_string())?;
    let source = env
        .new_string(source.to_string_lossy().as_ref())
        .map_err(|error| error.to_string())?;
    let output = env
        .new_string(output.to_string_lossy().as_ref())
        .map_err(|error| error.to_string())?;
    let source_object = JObject::from(source);
    let output_object = JObject::from(output);
    env.call_method(
        bridge.activity.as_obj(),
        "extractAudioTrack",
        "(Ljava/lang/String;Ljava/lang/String;)V",
        &[
            JValue::Object(&source_object),
            JValue::Object(&output_object),
        ],
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn update_android_transfer(summary: app::TransferSummary) -> Result<(), String> {
    let bridge = ANDROID_BRIDGE
        .get()
        .ok_or_else(|| "Android bridge is not initialized".to_owned())?;
    let mut env = bridge
        .vm
        .attach_current_thread()
        .map_err(|error| error.to_string())?;
    env.call_method(
        bridge.activity.as_obj(),
        "updateTransferNotification",
        "(IJJ)V",
        &[
            JValue::Int(summary.count as i32),
            JValue::Long(summary.downloaded as i64),
            JValue::Long(summary.total as i64),
        ],
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn watched_from_android() -> Result<Vec<String>, String> {
    let bridge = ANDROID_BRIDGE
        .get()
        .ok_or_else(|| "Android bridge is not initialized".to_owned())?;
    let mut env = bridge
        .vm
        .attach_current_thread()
        .map_err(|error| error.to_string())?;
    let value = env
        .call_method(
            bridge.activity.as_obj(),
            "watchedDownloads",
            "()Ljava/lang/String;",
            &[],
        )
        .and_then(|value| value.l())
        .map_err(|error| error.to_string())?;
    let value = JString::from(value);
    let filenames: String = env
        .get_string(&value)
        .map_err(|error| error.to_string())?
        .into();
    Ok(filenames
        .lines()
        .filter(|filename| !filename.is_empty())
        .map(str::to_owned)
        .collect())
}

fn delete_from_android_downloads(filename: &str) -> Result<(), String> {
    let bridge = ANDROID_BRIDGE
        .get()
        .ok_or_else(|| "Android bridge is not initialized".to_owned())?;
    let mut env = bridge
        .vm
        .attach_current_thread()
        .map_err(|error| error.to_string())?;
    let filename = env
        .new_string(filename)
        .map_err(|error| error.to_string())?;
    let filename_object = JObject::from(filename);
    env.call_method(
        bridge.activity.as_obj(),
        "deletePublishedDownload",
        "(Ljava/lang/String;)V",
        &[JValue::Object(&filename_object)],
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

fn publish_to_android_downloads(source: &Path, filename: &str) -> Result<bool, String> {
    let bridge = ANDROID_BRIDGE
        .get()
        .ok_or_else(|| "Android bridge is not initialized".to_owned())?;
    let mut env = bridge
        .vm
        .attach_current_thread()
        .map_err(|error| error.to_string())?;
    let source = env
        .new_string(source.to_string_lossy().as_ref())
        .map_err(|error| error.to_string())?;
    let filename = env
        .new_string(filename)
        .map_err(|error| error.to_string())?;
    let source_object = JObject::from(source);
    let filename_object = JObject::from(filename);
    env.call_method(
        bridge.activity.as_obj(),
        "publishDownload",
        "(Ljava/lang/String;Ljava/lang/String;)Z",
        &[
            JValue::Object(&source_object),
            JValue::Object(&filename_object),
        ],
    )
    .and_then(|value| value.z())
    .map_err(|error| error.to_string())
}
