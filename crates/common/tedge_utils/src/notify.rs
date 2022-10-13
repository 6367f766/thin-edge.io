use async_stream::try_stream;
use futures::Stream;
// This crate replaces fs_notify, using the `notify` crate: https://github.com/notify-rs/notify
use notify::{raw_watcher, watcher, RecursiveMode, Watcher};
use std::{path::Path, sync::mpsc::channel};
use std::{path::PathBuf, time::Duration};

use crate::fs_notify::{FileEvent, NotifyStreamError};

fn stream_notify_event(
    path: PathBuf,
) -> impl Stream<Item = Result<(PathBuf, FileEvent), NotifyStreamError>> {
    try_stream! {
        // Create a channel to receive the events.
        let (tx, rx) = channel();

        // Create a watcher object, delivering debounced events.
        // The notification back-end is selected based on the platform.
        let mut watcher = watcher(tx, Duration::from_nanos(10)).unwrap();

        // Add a path to be watched. All files and directories at that path and
        // below will be monitored for changes.
        watcher.watch(path, RecursiveMode::Recursive).unwrap();
        loop {
            match rx.recv() {
                Ok(_event) => yield (PathBuf::default(), FileEvent::Created),
                Err(_e) => yield Err(NotifyStreamError::FailedToCreateStream)?,
            }
        }
    }
}

#[cfg(test)]
mod tests {

    pub use futures::{pin_mut, Stream, StreamExt};
    use notify::{raw_watcher, watcher, RecursiveMode, Watcher};
    use std::sync::mpsc::channel;
    use std::time::Duration;
    use tedge_test_utils::fs::TempTedgeDir;

    use super::stream_notify_event;

    #[tokio::test]
    async fn it_streams() {
        let ttd = TempTedgeDir::new();
        dbg!(ttd.path());

        let stream = stream_notify_event(ttd.path().to_path_buf());
        pin_mut!(stream);
        //while let Some(Ok((path, flag))) = stream.next().await {
        //    dbg!(path, flag);
        //}

        tokio::select! {
            Some(Ok((path, flag))) = stream.next() => {
                dbg!(path, flag);
            }
        }
    }

    #[tokio::test]
    async fn it_works() {
        let ttd = TempTedgeDir::new();
        // Create a channel to receive the events.
        let (tx, rx) = channel();

        // Create a watcher object, delivering debounced events.
        // The notification back-end is selected based on the platform.
        let mut watcher = watcher(tx, Duration::from_nanos(10)).unwrap();

        // Add a path to be watched. All files and directories at that path and
        // below will be monitored for changes.
        dbg!(ttd.path());
        watcher.watch(ttd.path(), RecursiveMode::Recursive).unwrap();

        let spawn_handle = tokio::spawn(async move {
            let _handle = tokio::spawn(async move {
                loop {
                    match rx.recv() {
                        Ok(event) => println!("{:?}", event),
                        Err(e) => println!("watch error: {:?}", e),
                    }
                }
            });
        });

        let fs_handle = tokio::spawn(async move {
            ttd.file("file_a");
            ttd.dir("dir_one");
            let ten_millis = std::time::Duration::from_millis(100);
            std::thread::sleep(ten_millis);
            ttd.dir("dir_one").dir("dir_two").file("file_c");
            let ten_millis = std::time::Duration::from_millis(100);
            std::thread::sleep(ten_millis);
        });

        spawn_handle.await.unwrap();
        fs_handle.await.unwrap();
    }
}
