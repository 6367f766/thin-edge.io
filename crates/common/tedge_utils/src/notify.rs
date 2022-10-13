use async_stream::try_stream;
use futures::Stream;
// This crate replaces fs_notify, using the `notify` crate: https://github.com/notify-rs/notify
use notify::{
    raw_watcher, watcher, DebouncedEvent, INotifyWatcher, RawEvent, RecursiveMode, Watcher,
};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
};
use std::{path::PathBuf, time::Duration};

use crate::fs_notify::{FileEvent, NotifyStreamError};

struct NotifyStream {
    rx: Receiver<DebouncedEvent>,
    watcher: INotifyWatcher,
    metadata: HashMap<PathBuf, HashSet<FileEvent>>,
    path_track: HashSet<PathBuf>,
}

impl NotifyStream {
    pub fn add_watcher(
        &mut self,
        dir_path: &Path,
        file: Option<&str>,
        events: &[FileEvent],
    ) -> Result<(), NotifyStreamError> {
        let (path, mode) = if let Some(file) = file {
            (dir_path.join(file), RecursiveMode::NonRecursive)
        } else {
            (dir_path.to_path_buf(), RecursiveMode::Recursive)
        };
        self.watcher.watch(&path, mode).unwrap();
        let entry = self.metadata.entry(path).or_insert(HashSet::new());
        for event in events {
            entry.insert(*event);
        }
        Ok(())
    }

    fn matches(&self, path: &Path, file_event: &FileEvent) -> Option<(PathBuf, FileEvent)> {
        let path_clone = path.clone().parent().unwrap();
        if path_clone.is_file() {
            // TODO
            None
        } else {
            // is dir
            //let parent = path.parent()?;
            let hs = self.metadata.get(path_clone).unwrap();
            if hs.contains(file_event) {
                Some((path.to_path_buf(), *file_event))
            } else {
                None
            }
        }
    }

    fn stream<'a>(
        &'a mut self,
    ) -> impl Stream<Item = Result<(PathBuf, FileEvent), NotifyStreamError>> + 'a {
        try_stream! {
            loop {
                match self.rx.recv() {
                    Ok(event) => {
                        match event {
                            DebouncedEvent::NoticeWrite(path) => {
                                if let Some((path, file_event)) = self.matches(&path, &FileEvent::Modified) {
                                    yield (path, file_event)
                                }
                            }
                            DebouncedEvent::Create(path) => {
                                if self.path_track.contains(&path) {
                                    // the path was not new
                                } else {
                                    self.path_track.insert(path.clone());
                                    if let Some((path, file_event)) = self.matches(&path, &FileEvent::Created) {
                                        yield (path, file_event)
                                    }
                                }
                            }
                            DebouncedEvent::Remove(path) => {
                                if let Some((path, file_event)) = self.matches(&path, &FileEvent::Deleted) {
                                    yield (path, file_event)
                                }
                            }
                            _rest => {}
                        }
                    },
                    Err(_e) => yield Err(NotifyStreamError::FailedToCreateStream)?,
                }
            }
        }
    }
}

impl Default for NotifyStream {
    fn default() -> Self {
        let (tx, rx) = channel();
        let watcher = INotifyWatcher::new(tx, std::time::Duration::from_secs(0)).unwrap(); // TODO
        Self {
            rx,
            watcher,
            metadata: HashMap::new(),
            path_track: HashSet::new(),
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

    use crate::{fs_notify::FileEvent, notify::NotifyStream};

    //use super::stream_notify_event;

    #[tokio::test]
    async fn it_streams() {
        let ttd = TempTedgeDir::new();
        //ttd.file("file_a");
        dbg!(ttd.path());

        let mut inotify_stream = NotifyStream::default();
        inotify_stream
            .add_watcher(
                ttd.path(),
                None,
                &[FileEvent::Created, FileEvent::Modified, FileEvent::Created],
            )
            .unwrap();

        let stream = inotify_stream.stream();
        pin_mut!(stream);

        //let file_handler = tokio::spawn(async move {
        //    ttd.file("file_a").with_raw_content("this is the content");
        //});
        //while let Some(Ok((path, flag))) = stream.next().await {
        //    dbg!(path, flag);
        //}

        loop {
            tokio::select! {
                Some(Ok((path, flag))) = stream.next() => {
                    dbg!(path, flag);
                },
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
