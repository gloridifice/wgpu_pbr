//! Optional CPU profiling. Initialize before calling instrumented dependencies.

#[doc(hidden)]
pub use bevy_log::tracing;

#[cfg(feature = "profile-tracy")]
#[derive(bevy_ecs::prelude::Resource)]
struct TracyClient {
    _client: tracy_client::Client,
}

/// Keep the client alive for the App's lifetime, including custom runners.
/// This does not install a tracing subscriber; the application's LogPlugin does.
pub fn init(_app: &mut bevy_app::App) {
    #[cfg(feature = "profile-tracy")]
    if !_app.world().contains_resource::<TracyClient>() {
        _app.insert_resource(TracyClient {
            _client: tracy_client::Client::start(),
        });
    }
}

/// Name a thread once, at its entry point. Safe without profiling initialization.
pub fn name_thread(_name: &str) {
    #[cfg(feature = "profile-tracy")]
    if let Some(client) = tracy_client::Client::running() {
        client.set_thread_name(_name);
    }
}

/// One mark per application update, not per camera, submission or presentation.
pub fn frame_mark() {
    #[cfg(feature = "profile-tracy")]
    if let Some(client) = tracy_client::Client::running() {
        client.frame_mark();
    }
}

/// A lexical CPU elapsed-time scope; disabled builds do not evaluate its fields.
#[cfg(feature = "profile-tracy")]
#[macro_export]
macro_rules! profile_scope {
    ($name:literal $(, $field:ident = $value:expr)* $(,)?) => {
        let _profile_scope = $crate::profiling::tracing::info_span!(
            $name $(, $field = $value)*
        ).entered();
    };
}

#[cfg(not(feature = "profile-tracy"))]
#[macro_export]
macro_rules! profile_scope {
    ($($tokens:tt)*) => {};
}
