use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::Duration;

pub struct FileWatcher {
    _watcher: RecommendedWatcher,
    rx: mpsc::Receiver<Result<Event, notify::Error>>,
    pub path: PathBuf,
}

impl FileWatcher {
    pub fn new(path: PathBuf) -> notify::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let mut watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            Config::default().with_poll_interval(Duration::from_millis(500)),
        )?;
        watcher.watch(&path, RecursiveMode::NonRecursive)?;
        Ok(Self {
            _watcher: watcher,
            rx,
            path,
        })
    }

    pub fn poll(&self) -> bool {
        let mut changed = false;
        while let Ok(Ok(event)) = self.rx.try_recv() {
            if event.kind.is_modify() || event.kind.is_create() {
                for p in &event.paths {
                    if p == &self.path {
                        changed = true;
                    }
                }
            }
        }
        changed
    }
}
