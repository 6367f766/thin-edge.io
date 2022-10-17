#[cfg(test)]
mod tests {
    use futures::{
        channel::mpsc::{channel, Receiver},
        SinkExt, StreamExt,
    };
    use notify::{Config, Error, Event, RecommendedWatcher, RecursiveMode, Watcher};
    use std::path::Path;
    use tedge_test_utils::fs::TempTedgeDir;

    async fn assert_stream(mut stream: Receiver<Result<Event, Error>>) {
        while let Some(Ok(event)) = stream.next().await {
            dbg!(event);
        }
    }

    #[tokio::test]
    async fn it_works_with_tokio() -> notify::Result<()> {
        let ttd = TempTedgeDir::new();
        dbg!(ttd.path());
        let dir = ttd.dir("dir_a");
        let (mut watcher, rx) = async_watcher()?;
        watcher.watch(dir.path().as_ref(), RecursiveMode::Recursive)?;

        let server_handler = tokio::spawn(async move {
            assert_stream(rx).await;
        });

        let handle = tokio::spawn(async move {
            dbg!("this was called");
            dir.file("file_a");
            ttd.dir("new_dir");
            //let result = watcher.watch(dir_two.path(), RecursiveMode::Recursive);
            //dbg!(result);
            //    .unwrap();
            //dir_two.file("file_b");
        });

        //let add_watcher = tokio::spawn(async move {
        //    let path = Path::new("/tmp/my-dir");
        //    let result = watcher.watch(path.as_ref(), RecursiveMode::Recursive);
        //    dbg!(result);
        //});

        server_handler.await.unwrap();
        handle.await.unwrap();
        //add_watcher.await.unwrap();

        Ok(())
    }

    #[test]
    fn it_works() {
        let path = Path::new("/tmp/my-dir");
        println!("watching {}", path.display());
        futures::executor::block_on(async {
            if let Err(e) = async_watch(path).await {
                println!("error: {:?}", e)
            }
        });
    }

    fn async_watcher() -> notify::Result<(RecommendedWatcher, Receiver<notify::Result<Event>>)> {
        let (mut tx, rx) = channel(1);

        // Automatically select the best implementation for your platform.
        // You can also access each implementation directly e.g. INotifyWatcher.
        let watcher = RecommendedWatcher::new(
            move |res| {
                futures::executor::block_on(async {
                    tx.send(res).await.unwrap();
                })
            },
            Config::default(),
        )?;

        Ok((watcher, rx))
    }

    async fn async_watch<P: AsRef<Path>>(path: P) -> notify::Result<()> {
        let (mut watcher, mut rx) = async_watcher()?;

        // Add a path to be watched. All files and directories at that path and
        // below will be monitored for changes.
        watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;

        while let Some(res) = rx.next().await {
            match res {
                Ok(event) => println!("changed: {:?}", event),
                Err(e) => println!("watch error: {:?}", e),
            }
        }

        Ok(())
    }
}

//use async_stream::try_stream;
//use futures::Stream;
//// This crate replaces fs_notify, using the `notify` crate: https://github.com/notify-rs/notify
//use notify::{
//    raw_watcher, watcher, DebouncedEvent, INotifyWatcher, RawEvent, RecursiveMode, Watcher,
//};
//use std::{
//    collections::{HashMap, HashSet},
//    path::Path,
//    sync::{
//        mpsc::{channel, Receiver, Sender},
//        Arc,
//    },
//};
//use std::{path::PathBuf, time::Duration};
//
//use crate::fs_notify::{FileEvent, NotifyStreamError};
//
//struct NotifyStream {
//    rx: Receiver<DebouncedEvent>,
//    watcher: INotifyWatcher,
//    metadata: HashMap<PathBuf, HashSet<FileEvent>>,
//    path_track: HashSet<PathBuf>,
//}
//
//impl NotifyStream {
//    pub fn add_watcher(
//        &mut self,
//        dir_path: &Path,
//        file: Option<&str>,
//        events: &[FileEvent],
//    ) -> Result<(), NotifyStreamError> {
//        let (path, mode) = if let Some(file) = file {
//            (dir_path.join(file), RecursiveMode::NonRecursive)
//        } else {
//            (dir_path.to_path_buf(), RecursiveMode::Recursive)
//        };
//        self.watcher.watch(&path, mode).unwrap();
//        let entry = self.metadata.entry(path).or_insert(HashSet::new());
//        for event in events {
//            entry.insert(*event);
//        }
//        Ok(())
//    }
//
//    fn matches(&self, path: &Path, file_event: &FileEvent) -> Option<(PathBuf, FileEvent)> {
//        let path_clone = path.clone().parent().unwrap();
//        if path_clone.is_file() {
//            // TODO
//            None
//        } else {
//            // is dir
//            //let parent = path.parent()?;
//            let hs = self.metadata.get(path_clone).unwrap();
//            if hs.contains(file_event) {
//                Some((path.to_path_buf(), *file_event))
//            } else {
//                None
//            }
//        }
//    }
//
//    fn stream<'a>(
//        &'a mut self,
//    ) -> impl Stream<Item = Result<(PathBuf, FileEvent), NotifyStreamError>> + 'a {
//        try_stream! {
//            loop {
//                match self.rx.recv() {
//                    Ok(event) => {
//                        match event {
//                            DebouncedEvent::NoticeWrite(path) => {
//                                if let Some((path, file_event)) = self.matches(&path, &FileEvent::Modified) {
//                                    yield (path, file_event)
//                                }
//                            }
//                            DebouncedEvent::Create(path) => {
//                                if self.path_track.contains(&path) {
//                                    // the path was not new
//                                } else {
//                                    self.path_track.insert(path.clone());
//                                    if let Some((path, file_event)) = self.matches(&path, &FileEvent::Created) {
//                                        yield (path, file_event)
//                                    }
//                                }
//                            }
//                            DebouncedEvent::Remove(path) => {
//                                if let Some((path, file_event)) = self.matches(&path, &FileEvent::Deleted) {
//                                    yield (path, file_event)
//                                }
//                            }
//                            _rest => {}
//                        }
//                    },
//                    Err(_e) => yield Err(NotifyStreamError::FailedToCreateStream)?,
//                }
//            }
//        }
//    }
//}
//
//impl Default for NotifyStream {
//    fn default() -> Self {
//        let (tx, rx) = channel();
//        let watcher = INotifyWatcher::new(tx, std::time::Duration::from_secs(0)).unwrap(); // TODO
//        Self {
//            rx,
//            watcher,
//            metadata: HashMap::new(),
//            path_track: HashSet::new(),
//        }
//    }
//}
//
//#[cfg(test)]
//mod tests {
//
//    pub use futures::{pin_mut, Stream, StreamExt};
//    use notify::{raw_watcher, watcher, RecursiveMode, Watcher};
//    use std::sync::mpsc::channel;
//    use std::time::Duration;
//    use tedge_test_utils::fs::TempTedgeDir;
//
//    use crate::{fs_notify::FileEvent, notify::NotifyStream};
//
//    //use super::stream_notify_event;
//
//    #[tokio::test]
//    async fn it_streams() {
//        let ttd = TempTedgeDir::new();
//        //ttd.file("file_a");
//        dbg!(ttd.path());
//
//        let mut inotify_stream = NotifyStream::default();
//        inotify_stream
//            .add_watcher(
//                ttd.path(),
//                None,
//                &[FileEvent::Created, FileEvent::Modified, FileEvent::Created],
//            )
//            .unwrap();
//
//        let stream = inotify_stream.stream();
//        pin_mut!(stream);
//
//        //let file_handler = tokio::spawn(async move {
//        //    ttd.file("file_a").with_raw_content("this is the content");
//        //});
//        //while let Some(Ok((path, flag))) = stream.next().await {
//        //    dbg!(path, flag);
//        //}
//
//        loop {
//            tokio::select! {
//                Some(Ok((path, flag))) = stream.next() => {
//                    dbg!(path, flag);
//                },
//            }
//        }
//    }
//
//    #[tokio::test]
//    async fn it_works() {
//        let ttd = TempTedgeDir::new();
//        // Create a channel to receive the events.
//        let (tx, rx) = channel();
//
//        // Create a watcher object, delivering debounced events.
//        // The notification back-end is selected based on the platform.
//        let mut watcher = watcher(tx, Duration::from_nanos(10)).unwrap();
//
//        // Add a path to be watched. All files and directories at that path and
//        // below will be monitored for changes.
//        dbg!(ttd.path());
//        watcher.watch(ttd.path(), RecursiveMode::Recursive).unwrap();
//
//        let spawn_handle = tokio::spawn(async move {
//            let _handle = tokio::spawn(async move {
//                loop {
//                    match rx.recv() {
//                        Ok(event) => println!("{:?}", event),
//                        Err(e) => println!("watch error: {:?}", e),
//                    }
//                }
//            });
//        });
//
//        let fs_handle = tokio::spawn(async move {
//            ttd.file("file_a");
//            ttd.dir("dir_one");
//            let ten_millis = std::time::Duration::from_millis(100);
//            std::thread::sleep(ten_millis);
//            ttd.dir("dir_one").dir("dir_two").file("file_c");
//            let ten_millis = std::time::Duration::from_millis(100);
//            std::thread::sleep(ten_millis);
//        });
//
//        spawn_handle.await.unwrap();
//        fs_handle.await.unwrap();
//    }
//}
