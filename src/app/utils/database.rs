// SPDX-License-Identifier: GPL-3.0-only

use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    time::Duration,
};

use futures::{SinkExt, Stream, StreamExt};
use iced::Subscription;
use notify_debouncer_full::{
    DebounceEventResult, new_debouncer,
    notify::{EventKind, RecursiveMode},
};
use tracing::{error, info};

/// How long to wait after the last raw filesystem event before emitting.
const DEBOUNCE: Duration = Duration::from_millis(100);

/// An Iced [`Subscription`] that emits `()` whenever the database file
pub fn watch_database(db_path: PathBuf) -> Subscription<()> {
    Subscription::run_with(db_path, |db_path| watch_stream(db_path.clone()))
}

fn watch_stream(db_path: PathBuf) -> impl Stream<Item = ()> {
    iced::stream::channel(16, async move |mut output| {
        let Some(dir) = db_path.parent().map(Path::to_path_buf) else {
            error!("Database path has no parent directory, not watching");
            return;
        };
        let file_name: Option<OsString> =
            db_path.file_name().map(std::ffi::OsStr::to_os_string);

        let (tx, mut rx) = futures::channel::mpsc::unbounded();

        let mut debouncer =
            match new_debouncer(DEBOUNCE, None, move |res: DebounceEventResult| {
                let _ = tx.unbounded_send(res);
            }) {
                Ok(debouncer) => debouncer,
                Err(e) => {
                    error!("Failed to create filesystem watcher: {e}");
                    return;
                }
            };

        if let Err(e) = debouncer.watch(&dir, RecursiveMode::NonRecursive) {
            error!("Failed to watch {dir:?}: {e}");
            return;
        }

        info!("Watching {dir:?} for database changes");

        // debouncer` must stay alive for as long as this task runs — dropping it stops the watch. Keeping it in scope  of this loop is what keeps it alive.
        while let Some(result) = rx.next().await {
            match result {
                Ok(events) => {
                    let relevant = events.iter().any(|ev| {
                        // only genuine mutations
                        let is_mutation = matches!(
                            ev.kind,
                            EventKind::Create(_)
                                | EventKind::Modify(_)
                                | EventKind::Remove(_)
                        );

                        // only events touching the database file itself
                        let touches_db = ev
                            .paths
                            .iter()
                            .any(|p| p.file_name() == file_name.as_deref());

                        is_mutation && touches_db
                    });

                    if relevant && output.send(()).await.is_err() {
                        // receiver gone: subscription was dropped
                        break;
                    }
                }
                Err(errors) => {
                    for e in errors {
                        error!("Filesystem watcher error: {e}");
                    }
                }
            }
        }
    })
}
