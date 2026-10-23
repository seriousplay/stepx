use futures_util::future::LocalBoxFuture;

use crate::{
    assets::AssetProvider,
    integration::TestDriver,
    platform::{self, app::AppCallbackDispatcher},
    AppContext,
};

pub struct App {
    callbacks: AppCallbackDispatcher,
}

impl App {
    #[allow(dead_code)]
    pub(in crate::platform) fn new(
        callbacks: platform::app::AppCallbacks,
        assets: Box<dyn AssetProvider>,
        _test_driver: Option<&TestDriver>,
    ) -> Self {
        let ui_app = crate::App::new(
            Box::new(super::AppDelegate::new().expect("should create test app delegate")),
            Box::new(super::WindowManager::new()),
            Box::new(super::FontDB::new()),
            assets,
        )
        .expect("should construct test app");

        Self {
            callbacks: AppCallbackDispatcher::new(callbacks, ui_app),
        }
    }

    #[allow(dead_code)]
    pub(in crate::platform) fn run(
        mut self,
        init_fn: impl FnOnce(&mut AppContext, LocalBoxFuture<'static, crate::App>) + 'static,
    ) {
        self.callbacks.initialize_app(Box::new(init_fn));
    }
}
