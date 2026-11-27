#![allow(unsafe_code)]

use super::Release;
use crate::shared::android::with_application_class;
use jni::{EnvUnowned, JValue, jni_sig, jni_str, objects::JString};
use std::sync::{Arc, Mutex};

const CLASS: &str = "io.github.shiroksh.streamtube.AppUpdater";
type Listener = dyn Fn(i32, i32, String) + Send + Sync + 'static;
static LISTENER: Mutex<Option<Arc<Listener>>> = Mutex::new(None);

pub fn listen(callback: impl Fn(i32, i32, String) + Send + Sync + 'static) {
    if let Ok(mut listener) = LISTENER.lock() {
        *listener = Some(Arc::new(callback));
    }
}

/// The installed package's `versionCode`, or 0 when it cannot be read.
pub fn version_code() -> i64 {
    with_application_class(CLASS, |env, activity, class| {
        env.call_static_method(
            class,
            jni_str!("versionCode"),
            jni_sig!("(Landroid/app/Activity;)J"),
            &[JValue::Object(activity)],
        )?
        .j()
    })
    .unwrap_or(0)
}

/// The installed package's `versionName`.
pub fn version_name() -> String {
    with_application_class(CLASS, |env, activity, class| {
        let name = env
            .call_static_method(
                class,
                jni_str!("versionName"),
                jni_sig!("(Landroid/app/Activity;)Ljava/lang/String;"),
                &[JValue::Object(activity)],
            )?
            .into_object()?;
        let name = env.cast_local::<JString>(name)?;
        name.try_to_string(env)
    })
    .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_owned())
}

/// Starts downloading `release` into an Android package installer session.
pub fn install(release: &Release) -> bool {
    with_application_class(CLASS, |env, activity, class| {
        let url = env.new_string(&release.url)?;
        let sha256 = env.new_string(&release.sha256)?;
        env.call_static_method(
            class,
            jni_str!("install"),
            jni_sig!("(Landroid/app/Activity;Ljava/lang/String;Ljava/lang/String;J)V"),
            &[
                JValue::Object(activity),
                JValue::Object(&url),
                JValue::Object(&sha256),
                JValue::Long(i64::try_from(release.size).unwrap_or(0)),
            ],
        )?;
        Ok(())
    })
    .is_ok()
}

#[unsafe(no_mangle)]
extern "system" fn Java_io_github_shiroksh_streamtube_AppUpdater_deliver<'local>(
    mut env: EnvUnowned<'local>,
    _class: jni::objects::JClass<'local>,
    status: i32,
    progress: i32,
    message: JString<'local>,
) {
    use jni::errors::ThrowRuntimeExAndDefault;

    env.with_env(|env| -> jni::errors::Result<()> {
        let message = message.try_to_string(env)?;
        let listener = LISTENER.lock().ok().and_then(|listener| listener.clone());
        if let Some(listener) = listener {
            listener(status, progress, message);
        }
        Ok(())
    })
    .resolve::<ThrowRuntimeExAndDefault>();
}
