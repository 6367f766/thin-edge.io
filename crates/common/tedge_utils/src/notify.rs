use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::Duration,
};

use async_stream::try_stream;
use futures::{
    //channel::mpsc::{channel, Receiver, Sender},
    SinkExt,
    Stream,
    StreamExt,
};
use notify::{
    event::{AccessKind, AccessMode, CreateKind, RemoveKind},
    Config, EventKind, INotifyWatcher, RecommendedWatcher, RecursiveMode, Watcher,
};
use tokio::sync::mpsc::{channel, Receiver};
use try_traits::default::TryDefault;

use crate::fs_notify::{FileEvent, NotifyStreamError};

type DirPath = PathBuf;
type MaybeFileName = Option<String>;
type Metadata = HashMap<DirPath, HashMap<MaybeFileName, HashSet<FileEvent>>>;

struct WatchMetadata {
    metadata: Metadata,
}

struct NotifyStream {
    watcher: INotifyWatcher,
    rx: Receiver<(PathBuf, FileEvent)>,
    metadata: WatchMetadata,
}

impl TryDefault for NotifyStream {
    type Error = NotifyStreamError;

    fn try_default() -> Result<Self, Self::Error> {
        let (tx, rx) = channel(1024);

        // Automatically select the best implementation for your platform.
        // You can also access each implementation directly e.g. INotifyWatcher.
        let watcher = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                futures::executor::block_on(async {
                    if let Ok(notify_event) = res {
                        match notify_event.kind {
                            EventKind::Access(AccessKind::Close(AccessMode::Write)) => {
                                for path in notify_event.paths {
                                    tx.send((path, FileEvent::Modified)).await.unwrap();
                                }
                            }
                            EventKind::Create(CreateKind::File) => {
                                for path in notify_event.paths {
                                    tx.send((path, FileEvent::Created)).await.unwrap();
                                }
                            }
                            EventKind::Create(CreateKind::Folder) => {
                                for path in notify_event.paths {
                                    tx.send((path, FileEvent::Created)).await.unwrap();
                                }
                            }
                            //EventKind::Modify(_) => todo!(),
                            EventKind::Remove(RemoveKind::File) => {
                                for path in notify_event.paths {
                                    tx.send((path, FileEvent::Deleted)).await.unwrap();
                                }
                            }
                            EventKind::Remove(RemoveKind::Folder) => {
                                for path in notify_event.paths {
                                    tx.send((path, FileEvent::Deleted)).await.unwrap();
                                }
                            }
                            _other => {}
                        }
                    }
                })
            },
            Config::default(),
        )
        .unwrap();
        Ok(Self {
            watcher,
            rx,
            metadata: WatchMetadata {
                metadata: HashMap::new(),
            },
        })
    }
}

impl NotifyStream {
    fn get_metadata_as_mut(&mut self) -> &mut Metadata {
        &mut self.metadata.metadata
    }

    fn add_watcher(&mut self, dir_path: &Path, file: Option<String>, events: &[FileEvent]) {
        // we make the full path for the watcher in case a `file` is given.
        // when a `file` is given we return a NonRecursive watcher
        let (full_path, recursive_mode) = if let Some(file_name) = file.clone() {
            (dir_path.join(file_name), RecursiveMode::NonRecursive)
        } else {
            (dir_path.to_path_buf(), RecursiveMode::Recursive)
        };

        self.watcher.watch(&full_path, recursive_mode).unwrap(); // TODO

        // we add the information to the metadata
        let maybe_file_name_entry = self
            .get_metadata_as_mut()
            .entry(dir_path.to_path_buf())
            .or_insert(HashMap::new());

        let file_event_entry = maybe_file_name_entry.entry(file).or_insert(HashSet::new());
        for event in events {
            file_event_entry.insert(*event);
        }
    }

    fn stream<'a>(
        &'a mut self,
    ) -> impl Stream<Item = Result<(PathBuf, FileEvent), NotifyStreamError>> + '_ {
        try_stream! {
            for (path, file_event) in &self.rx.recv().await {
                yield  (path.to_owned(), *file_event)
             }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, path::PathBuf, sync::Arc, time::Duration};

    use futures::{pin_mut, Stream, StreamExt};
    use maplit::hashmap;
    use tedge_test_utils::fs::TempTedgeDir;
    use try_traits::default::TryDefault;

    use crate::fs_notify::{FileEvent, NotifyStreamError};

    use super::NotifyStream;

    async fn assert_stream(
        mut inputs: HashMap<String, Vec<FileEvent>>,
        stream: impl Stream<Item = Result<(PathBuf, FileEvent), NotifyStreamError>>,
    ) {
        pin_mut!(stream);
        while let Some(Ok((path, flag))) = stream.next().await {
            let file_name = String::from(path.file_name().unwrap().to_str().unwrap());
            let mut values = match inputs.get_mut(&file_name) {
                Some(v) => v.to_vec(),
                None => {
                    inputs.remove(&file_name);
                    continue;
                }
            };
            match values.iter().position(|x| *x == flag) {
                Some(i) => values.remove(i),
                None => {
                    continue;
                }
            };
            if values.is_empty() {
                inputs.remove(&file_name);
            } else {
                inputs.insert(file_name, values);
            }
            if inputs.is_empty() {
                break;
            }
            dbg!(&inputs);
        }
    }

    #[tokio::test]
    async fn it_works() {
        let ttd = Arc::new(TempTedgeDir::new());
        let ttd_clone = ttd.clone();
        let mut fs_notify = NotifyStream::try_default().unwrap();
        fs_notify.add_watcher(ttd.path(), None, &[FileEvent::Created, FileEvent::Modified]);

        let file_system_handler = tokio::spawn(async move {
            ttd_clone.file("file_b"); //.with_raw_content("yo");
            ttd_clone.file("file_a");
            ttd_clone.file("file_c");
            //tokio::time::sleep(Duration::from_secs(1)).await;
            //ttd_clone.file("file_c").delete();
        });

        let fs_notify_handler = tokio::spawn(async move {
            let stream = fs_notify.stream();
            pin_mut!(stream);
            while let Some(Ok((path, event))) = stream.next().await {
                dbg!(path, event);
            }
        });

        //let fs_notify_handler = tokio::task::spawn(async move {
        //    assert_stream(expected_events, stream).await;
        //});

        //fs_notify_handler.await.unwrap();
        file_system_handler.await.unwrap();
    }
}

// TODO: explore https://github.com/notify-rs/notify/issues/380
