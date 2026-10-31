use serde::{de::DeserializeOwned, Serialize};
use tauri::{
    plugin::{Builder, PluginHandle, TauriPlugin},
    Manager, Runtime,
};

#[cfg(target_os = "android")]
const PLUGIN_IDENTIFIER: &str = "com.zwormhole.mobile";

#[cfg(target_os = "ios")]
tauri::ios_plugin_binding!(init_plugin_zwormhole_mobile);

pub struct Mobile<R: Runtime>(PluginHandle<R>);

impl<R: Runtime> Mobile<R> {
    pub fn call<T: DeserializeOwned>(
        &self,
        command: &str,
        payload: impl Serialize,
    ) -> Result<T, String> {
        self.0
            .run_mobile_plugin(command, payload)
            .map_err(|e| e.to_string())
    }
}

pub trait MobileExt<R: Runtime> {
    fn native(&self) -> &Mobile<R>;
}

impl<R: Runtime, T: Manager<R>> MobileExt<R> for T {
    fn native(&self) -> &Mobile<R> {
        self.state::<Mobile<R>>().inner()
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("zwormhole-mobile")
        .setup(|app, api| {
            #[cfg(target_os = "android")]
            let handle = api.register_android_plugin(PLUGIN_IDENTIFIER, "ZWormHoleMobilePlugin")?;
            #[cfg(target_os = "ios")]
            let handle = api.register_ios_plugin(init_plugin_zwormhole_mobile)?;
            app.manage(Mobile(handle));
            Ok(())
        })
        .build()
}
